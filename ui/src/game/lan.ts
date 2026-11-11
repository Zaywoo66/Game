import { useEffect, useRef, useState } from 'react';

export type AiPreset = 'balanced' | 'cautious' | 'aggressive';

export interface LanConfig {
  room: string;
  host_country_id: number;
  guest_country_id: number;
  ai_preset: AiPreset;
  grace_minutes: number;
  port: number;
}

export interface LanStatus {
  mode: 'solo' | 'host' | 'guest';
  state: 'idle' | 'hosting' | 'connected' | 'reconnecting' | 'resyncing';
  room: string | null;
  address: string | null;
  guest_connected: boolean;
  guest_country_id: number | null;
  can_takeover: boolean;
  temporary_ai: boolean;
  verified_checksums: number;
  checksum_mismatches: number;
  detail: string | null;
  generation: number;
}

export interface DiscoveredRoom {
  name: string;
  address: string;
  player_count: number;
  version: number;
}

type LanAction = 'lan_start' | 'lan_join' | 'lan_leave' | 'lan_takeover';

function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return 'The LAN request failed.';
}

export function lanStatusText(status: LanStatus): string {
  if (status.state === 'reconnecting') return 'Reconnecting';
  if (status.state === 'resyncing') return 'Resynchronizing';
  if (status.state === 'connected') return 'Connected';
  if (status.mode === 'host') return status.guest_connected ? 'Connected' : 'Waiting for guest';
  return 'Solo';
}

export function useLan(enabled: boolean) {
  const bridge = enabled ? window.__TAURI__ : undefined;
  const [status, setStatus] = useState<LanStatus | null>(null);
  const [checked, setChecked] = useState(false);
  const [rooms, setRooms] = useState<DiscoveredRoom[]>([]);
  const [scanned, setScanned] = useState(false);
  const [pendingAction, setPendingAction] = useState<LanAction | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState('');
  const observedEvents = useRef(0);
  const actionInFlight = useRef(false);
  const scanInFlight = useRef(false);

  useEffect(() => {
    if (!bridge) return;
    let active = true;
    let unlisten: (() => void) | undefined;

    async function subscribe(ipc: NonNullable<typeof bridge>) {
      try {
        unlisten = await ipc.event.listen<LanStatus>('strategy:lan', ({ payload }) => {
          if (!active) return;
          observedEvents.current++;
          setStatus((current) => !current || payload.generation >= current.generation ? payload : current);
        });
        if (!active) {
          unlisten();
          return;
        }
        const revision = observedEvents.current;
        const initial = await ipc.core.invoke<LanStatus>('lan_status');
        if (!active) return;
        setStatus((current) => (
          !current || initial.generation > current.generation
          || (initial.generation === current.generation && observedEvents.current === revision)
        ) ? initial : current);
      } catch (reason) {
        if (active) {
          unlisten?.();
          unlisten = undefined;
          setError(errorText(reason));
        }
      } finally {
        if (active) setChecked(true);
      }
    }

    void subscribe(bridge);
    return () => {
      active = false;
      unlisten?.();
    };
  }, [bridge]);

  async function action(command: LanAction, args?: Record<string, unknown>) {
    if (!bridge || actionInFlight.current) return;
    actionInFlight.current = true;
    const revision = observedEvents.current;
    setPendingAction(command);
    setError('');
    try {
      const next = await bridge.core.invoke<LanStatus>(command, args);
      setStatus((current) => (
        !current || next.generation > current.generation
        || (next.generation === current.generation && observedEvents.current === revision)
      ) ? next : current);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      actionInFlight.current = false;
      setPendingAction(null);
    }
  }

  async function discover() {
    if (!bridge || scanInFlight.current) return;
    scanInFlight.current = true;
    setScanning(true);
    setError('');
    try {
      setRooms(await bridge.core.invoke<DiscoveredRoom[]>('lan_discover'));
      setScanned(true);
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      scanInFlight.current = false;
      setScanning(false);
    }
  }

  return {
    status, rooms, scanned, pendingAction, busy: pendingAction !== null,
    checking: Boolean(bridge) && !checked, scanning, error, discover,
    start: (config: LanConfig) => action('lan_start', { config }),
    join: (address: string, name: string) => action('lan_join', { address, name }),
    leave: () => action('lan_leave'),
    takeover: () => action('lan_takeover'),
  };
}

export type LanController = ReturnType<typeof useLan>;
