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

export interface ArmyView {
  id: number;
  country_id: number;
  province_id: number;
  soldiers: number;
  morale: number;
  to_province_id: number | null;
  progress: number;
}

export interface WarView {
  id: number;
  attacker_id: number;
  defender_id: number;
  attacker_supporters: number[];
  defender_supporters: number[];
  goal_province_id: number;
  score: number;
  occupations: { province_id: number; country_id: number }[];
  offer: { offered_by: number; cede_goal: boolean } | null;
}

export interface SiegeView {
  province_id: number;
  war_id: number;
  country_id: number;
  progress_days: number;
  required_days: number;
}

export interface AiLogEntry {
  tick: number;
  country_id: number;
  reason: string;
  command: string;
  score: number | null;
}

export interface EventView {
  country_id: number;
  key: string;
  title_key: string;
  desc_key: string;
  options: { text_key: string }[];
}

export interface GameSnapshot {
  generation?: number;
  sequence: number;
  clock: ClockView;
  local_country_id: number | null;
  countries: CountryView[];
  pending_events?: EventView[];
  guarantees: { guarantor_id: number; protected_id: number }[];
  owners: { province_id: number; country_id: number }[];
  max_tax_rate: number;
  max_monthly_spending: string;
  armies?: ArmyView[];
  wars?: WarView[];
  sieges?: SiegeView[];
  forts?: { province_id: number; level: number }[];
  min_raise_soldiers?: number;
  max_raise_soldiers?: number;
}

interface GameUpdate {
  generation?: number;
  sequence: number;
  clock: ClockView;
  countries: CountryView[];
  pending_events?: EventView[] | null;
  guarantees: { guarantor_id: number; protected_id: number }[];
  armies?: ArmyView[];
  removed_armies?: number[];
  wars?: WarView[];
  removed_wars?: number[];
  sieges?: SiegeView[] | null;
  owners?: { province_id: number; country_id: number | null }[];
}

export type GameCommand =
  | { type: 'set_paused'; paused: boolean }
  | { type: 'set_speed'; speed: number }
  | { type: 'set_fiscal_policy'; country_id: number; tax_rate: number; monthly_spending: string }
  | { type: 'raise_army'; country_id: number; province_id: number; soldiers: number }
  | { type: 'merge_armies'; country_id: number; into: number; from: number }
  | { type: 'split_army'; country_id: number; army_id: number; soldiers: number }
  | { type: 'move_army' | 'retreat_army'; country_id: number; army_id: number; destination: number }
  | { type: 'declare_war'; country_id: number; defender_id: number; goal_province_id: number }
  | { type: 'offer_peace'; country_id: number; war_id: number; cede_goal: boolean }
  | { type: 'accept_peace'; country_id: number; war_id: number }
  | { type: 'choose_event'; country_id: number; key: string; option: number }
  | { type: 'guarantee' | 'revoke_guarantee'; country_id: number; protected_id: number };

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

function updateRows<T extends { id: number }>(
  current: T[] | undefined,
  changed: T[] = [],
  removed: number[] = [],
): T[] | undefined {
  if (!changed.length && !removed.length) return current;
  const rows = new Map(current?.map((row) => [row.id, row]));
  for (const row of changed) rows.set(row.id, row);
  for (const id of removed) rows.delete(id);
  return [...rows.values()];
}

function applyUpdate(snapshot: GameSnapshot, update: GameUpdate): GameSnapshot {
  if (update.sequence <= snapshot.sequence) return snapshot;
  let owners = snapshot.owners;
  if (update.owners?.length) {
    const byProvince = new Map(owners.map(({ province_id, country_id }) => [province_id, country_id]));
    for (const { province_id, country_id } of update.owners) {
      if (country_id === null) byProvince.delete(province_id);
      else byProvince.set(province_id, country_id);
    }
    owners = [...byProvince].map(([province_id, country_id]) => ({ province_id, country_id }));
  }
  return {
    ...snapshot,
    sequence: update.sequence,
    clock: update.clock,
    countries: updateRows(snapshot.countries, update.countries) ?? snapshot.countries,
    pending_events: update.pending_events ?? snapshot.pending_events,
    guarantees: update.guarantees,
    owners,
    armies: updateRows(snapshot.armies, update.armies, update.removed_armies),
    wars: updateRows(snapshot.wars, update.wars, update.removed_wars),
    sieges: update.sieges ?? snapshot.sieges,
  };
}

function errorText(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === 'string') return reason;
  return 'The host returned an unknown error.';
}

export function useGameHost(enabled: boolean, generation = 0, canCommand = true, canSetSpeed = true) {
  const bridge = enabled ? window.__TAURI__ : undefined;
  const [model, setModel] = useState<{ generation: number; snapshot: GameSnapshot } | null>(null);
  const snapshot = model?.generation === generation ? model.snapshot : null;
  const [connectionError, setConnectionError] = useState('');
  const [commandError, setCommandError] = useState('');
  const [pending, setPending] = useState(false);
  const inFlight = useRef<symbol>();

  useEffect(() => {
    inFlight.current = undefined;
    setPending(false);
    setConnectionError('');
    setCommandError('');
    if (!bridge) return;
    let active = true;
    let ready = false;
    let unlisten: (() => void) | undefined;
    const queued: GameUpdate[] = [];

    async function connect(ipc: TauriBridge) {
      try {
        // Subscribe first so a tick during snapshot loading is not lost.
        unlisten = await ipc.event.listen<GameUpdate>('strategy:update', ({ payload }) => {
          if (!active || (payload.generation !== undefined && payload.generation !== generation)) return;
          if (ready) {
            setModel((current) => current?.generation === generation
              ? { generation, snapshot: applyUpdate(current.snapshot, payload) }
              : current);
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
        if (initial.generation !== undefined && initial.generation !== generation) return;
        const latest = queued
          .sort((left, right) => left.sequence - right.sequence)
          .reduce(applyUpdate, initial);
        queued.length = 0;
        ready = true;
        setModel({ generation, snapshot: latest });
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
  }, [bridge, generation]);

  async function send(command: GameCommand): Promise<void> {
    if (!bridge || !snapshot || !canCommand || inFlight.current
      || (command.type === 'set_speed' && !canSetSpeed)) return;
    const request = Symbol();
    inFlight.current = request;
    setPending(true);
    setCommandError('');
    try {
      await bridge.core.invoke<null>('game_command', { command });
      // Read models change only when the host emits an update.
    } catch (reason) {
      if (inFlight.current === request) setCommandError(errorText(reason));
    } finally {
      if (inFlight.current === request) {
        inFlight.current = undefined;
        setPending(false);
      }
    }
  }

  function readAiLog(): Promise<AiLogEntry[]> {
    return bridge ? bridge.core.invoke<AiLogEntry[]>('game_ai_log') : Promise.resolve([]);
  }

  return { connected: Boolean(bridge), snapshot, connectionError, commandError,
    pending: pending || !canCommand, send, readAiLog };
}
