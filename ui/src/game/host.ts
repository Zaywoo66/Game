import { useEffect, useRef, useState } from 'react';

export interface ClockView {
  date: string;
  tick: number;
  paused: boolean;
  speed: number;
}

export interface CharacterView {
  id: number;
  name: string;
  dynasty: string;
  age: number;
  adm: number;
  dip: number;
  mil: number;
  traits: string[];
  death: string | null;
}

export interface CountryView {
  id: number;
  tag: string;
  name: string;
  color: string;
  capital_id: number;
  capital: string;
  treasury: string;
  income: string;
  expenses: string;
  tax_rate: number;
  monthly_spending: string;
  bankrupt: boolean;
  regency: boolean;
  ruler: CharacterView | null;
  heir: CharacterView | null;
}

export interface GameSnapshot {
  sequence: number;
  clock: ClockView;
  local_country_id: number | null;
  countries: CountryView[];
  owners: { province_id: number; country_id: number }[];
  max_tax_rate: number;
  max_monthly_spending: string;
}

interface GameUpdate {
  sequence: number;
  clock: ClockView;
  countries: CountryView[];
}

export type GameCommand =
  | { type: 'set_paused'; paused: boolean }
  | { type: 'set_speed'; speed: number }
  | { type: 'set_fiscal_policy'; country_id: number; tax_rate: number; monthly_spending: string };

interface TauriBridge {
  core: { invoke<T>(name: string, args?: Record<string, unknown>): Promise<T> };
  event: {
    listen<T>(name: string, callback: (event: { payload: T }) => void): Promise<() => void>;
  };
}

declare global {
  interface Window {
    __TAURI__?: TauriBridge;
  }
}

function applyUpdate(snapshot: GameSnapshot, update: GameUpdate): GameSnapshot {
  if (update.sequence <= snapshot.sequence) return snapshot;
  let countries = snapshot.countries;
  if (update.countries.length > 0) {
    const rows = new Map(countries.map((country) => [country.id, country]));
    for (const country of update.countries) rows.set(country.id, country);
    countries = [...rows.values()];
  }
  return { ...snapshot, sequence: update.sequence, clock: update.clock, countries };
}

function errorText(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === 'string') return reason;
  return 'The host returned an unknown error.';
}

export function useGameHost(enabled: boolean) {
  const bridge = enabled ? window.__TAURI__ : undefined;
  const [snapshot, setSnapshot] = useState<GameSnapshot | null>(null);
  const [connectionError, setConnectionError] = useState('');
  const [commandError, setCommandError] = useState('');
  const [pending, setPending] = useState(false);
  const inFlight = useRef(false);

  useEffect(() => {
    if (!bridge) return;
    let active = true;
    let ready = false;
    let unlisten: (() => void) | undefined;
    const queued: GameUpdate[] = [];

    async function connect(ipc: TauriBridge) {
      try {
        // Subscribe first so a tick during snapshot loading is not lost.
        unlisten = await ipc.event.listen<GameUpdate>('strategy:update', ({ payload }) => {
          if (!active) return;
          if (ready) {
            setSnapshot((current) => current && applyUpdate(current, payload));
          } else {
            queued.push(payload);
          }
        });
        if (!active) {
          unlisten();
          return;
        }
        const initial = await ipc.core.invoke<GameSnapshot>('game_snapshot');
        if (!active) return;
        const latest = queued
          .sort((left, right) => left.sequence - right.sequence)
          .reduce(applyUpdate, initial);
        queued.length = 0;
        ready = true;
        setSnapshot(latest);
      } catch (reason) {
        if (active) {
          unlisten?.();
          unlisten = undefined;
          setConnectionError(errorText(reason));
        }
      }
    }

    void connect(bridge);
    return () => {
      active = false;
      unlisten?.();
    };
  }, [bridge]);

  async function send(command: GameCommand): Promise<void> {
    if (!bridge || !snapshot || inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    setCommandError('');
    try {
      await bridge.core.invoke<null>('game_command', { command });
      // Read models change only when the host emits an update.
    } catch (reason) {
      setCommandError(errorText(reason));
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  }

  return { connected: Boolean(bridge), snapshot, connectionError, commandError, pending, send };
}
