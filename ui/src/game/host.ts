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
  sex?: 'male' | 'female' | 'unknown';
  spouse_id?: number | null;
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
  tribute_paid: string;
  tribute_received: string;
  expenses: string;
  tax_rate: number;
  monthly_spending: string;
  bankrupt: boolean;
  regency: boolean;
  ruler: CharacterView | null;
  heir: CharacterView | null;
  research_points: number | null;
  known_technologies: string[];
  known_ideas: string[];
  government: string;
  known_reforms: string[];
  chosen_reform_tiers: number[];
}

export interface TechnologyView {
  key: string;
  branch: string;
  cost: number;
  requires: string[];
  pos_x: number;
  pos_y: number;
  effects: { key: string; value: string; kind: 'additive' | 'multiplicative'; source_key: string }[];
}

export interface IdeaView {
  key: string;
  country: string;
  name_key: string;
  available_countries: number[];
  effects: { key: string; value: string; kind: 'additive' | 'multiplicative' }[];
}

export interface ReformView {
  key: string;
  government: string;
  tier: number;
  ai_weight: number;
  effects: { key: string; value: string; kind: 'additive' | 'multiplicative' }[];
}

export interface ArmyView {
  id: number;
  country_id: number;
  province_id: number;
  soldiers: number;
  morale: number;
  to_province_id: number | null;
  progress: number;
  embarked_on?: number | null;
}

export interface FleetView {
  id: number;
  country_id: number;
  sea_province_id: number;
  ships: number;
  cargo_army_id: number | null;
  to_sea_province_id: number | null;
  progress: number;
}

export interface ShipyardView {
  port_province_id: number;
  sea_province_id: number;
  country_id: number;
  days_remaining: number;
}

export interface CallToArmsOfferView {
  war_id: number;
  from_id: number;
  toward_id: number;
  side: 'attacker' | 'defender';
  expires_at: string;
}

export interface WarView {
  id: number;
  attacker_id: number;
  defender_id: number;
  attacker_supporters: number[];
  defender_supporters: number[];
  goal_province_id: number;
  independence: boolean;
  score: number;
  occupations: { province_id: number; country_id: number }[];
  offer: { offered_by: number; cede_goal: boolean } | null;
}

export interface TruceView {
  a_id: number;
  b_id: number;
  remaining_days: number;
  ends_on: string | null;
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

export interface ProposalAcceptancePreview {
  outcome: string;
  factors: { key: string; value: number }[];
}

export interface CountryAcceptancePreview {
  country_id: number;
  alliance: ProposalAcceptancePreview;
  vassalage: ProposalAcceptancePreview;
  marriage: ProposalAcceptancePreview | null;
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
  acceptance_previews?: CountryAcceptancePreview[];
  technologies?: TechnologyView[];
  ideas?: IdeaView[];
  reforms?: ReformView[];
  pending_events?: EventView[];
  guarantees: { guarantor_id: number; protected_id: number }[];
  alliances?: { a_id: number; b_id: number }[];
  alliance_offers?: { from_id: number; toward_id: number; expires_at: string }[];
  marriage_offers?: { from_country_id: number; toward_country_id: number; from_character_id: number;
    toward_character_id: number; expires_at: string }[];
  marriage_adulthood_years?: number;
  opinions?: { from_id: number; toward_id: number; points: number; effective_points: number }[];
  truces: TruceView[];
  blockaded_ports?: number[];
  vassals: { subject_id: number; overlord_id: number }[];
  vassal_offers: { overlord_id: number; subject_id: number; expires_at: string }[];
  call_to_arms_offers?: CallToArmsOfferView[];
  owners: { province_id: number; country_id: number }[];
  max_tax_rate: number;
  max_monthly_spending: string;
  armies?: ArmyView[];
  fleets?: FleetView[];
  shipyards?: ShipyardView[];
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
  acceptance_previews?: CountryAcceptancePreview[];
  pending_events?: EventView[] | null;
  guarantees: { guarantor_id: number; protected_id: number }[];
  alliances?: { a_id: number; b_id: number }[];
  alliance_offers?: { from_id: number; toward_id: number; expires_at: string }[];
  marriage_offers?: { from_country_id: number; toward_country_id: number; from_character_id: number;
    toward_character_id: number; expires_at: string }[];
  marriage_adulthood_years?: number;
  opinions?: { from_id: number; toward_id: number; points: number; effective_points: number }[];
  truces: TruceView[];
  blockaded_ports?: number[];
  vassals: { subject_id: number; overlord_id: number }[];
  vassal_offers: { overlord_id: number; subject_id: number; expires_at: string }[];
  call_to_arms_offers?: CallToArmsOfferView[];
  armies?: ArmyView[];
  removed_armies?: number[];
  fleets?: FleetView[];
  shipyards?: ShipyardView[];
  removed_fleets?: number[];
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
  | { type: 'move_fleet'; country_id: number; fleet_id: number; destination: number }
  | { type: 'build_ship'; country_id: number; port_id: number; sea_id: number }
  | { type: 'embark_army'; country_id: number; army_id: number; fleet_id: number }
  | { type: 'disembark_army'; country_id: number; fleet_id: number; destination: number }
  | { type: 'declare_war'; country_id: number; defender_id: number; goal_province_id: number }
  | { type: 'accept_call_to_arms' | 'reject_call_to_arms'; country_id: number; war_id: number; from_id: number }
  | { type: 'declare_independence'; country_id: number }
  | { type: 'offer_peace'; country_id: number; war_id: number; cede_goal: boolean }
  | { type: 'accept_peace'; country_id: number; war_id: number }
  | { type: 'offer_alliance'; country_id: number; toward_id: number }
  | { type: 'accept_alliance' | 'reject_alliance'; country_id: number; from_id: number }
  | { type: 'break_alliance'; country_id: number; other_id: number }
  | { type: 'offer_royal_marriage'; country_id: number; character_id: number; toward_id: number; partner_id: number }
  | { type: 'accept_royal_marriage' | 'reject_royal_marriage'; country_id: number; from_id: number;
    character_id: number; partner_id: number }
  | { type: 'choose_event'; country_id: number; key: string; option: number }
  | { type: 'guarantee' | 'revoke_guarantee'; country_id: number; protected_id: number }
  | { type: 'offer_vassalage'; country_id: number; toward_id: number }
  | { type: 'accept_vassalage' | 'reject_vassalage'; country_id: number; from_id: number }
  | { type: 'release_vassal'; country_id: number; subject_id: number }
  | { type: 'unlock_technology' | 'unlock_idea' | 'unlock_reform'; country_id: number; key: string };

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

function normalizeSnapshot(snapshot: GameSnapshot): GameSnapshot {
  return {
    ...snapshot,
    countries: snapshot.countries.map((country) => ({
      ...country,
      tribute_paid: country.tribute_paid ?? '0',
      tribute_received: country.tribute_received ?? '0',
      research_points: country.research_points ?? null,
      known_technologies: country.known_technologies ?? [],
      known_ideas: country.known_ideas ?? [],
      government: country.government ?? '',
      known_reforms: country.known_reforms ?? [],
      chosen_reform_tiers: country.chosen_reform_tiers ?? [],
    })),
    guarantees: snapshot.guarantees ?? [],
    alliances: snapshot.alliances ?? [],
    alliance_offers: snapshot.alliance_offers ?? [],
    marriage_offers: snapshot.marriage_offers ?? [],
    marriage_adulthood_years: snapshot.marriage_adulthood_years ?? 18,
    opinions: snapshot.opinions ?? [],
    truces: snapshot.truces ?? [],
    vassals: snapshot.vassals ?? [],
    vassal_offers: snapshot.vassal_offers ?? [],
  };
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
    countries: updateRows(snapshot.countries, update.countries)?.map((country) => ({
      ...country,
      tribute_paid: country.tribute_paid ?? '0',
      tribute_received: country.tribute_received ?? '0',
      research_points: country.research_points ?? null,
      known_technologies: country.known_technologies ?? [],
      known_ideas: country.known_ideas ?? [],
      government: country.government ?? '',
      known_reforms: country.known_reforms ?? [],
      chosen_reform_tiers: country.chosen_reform_tiers ?? [],
    })) ?? snapshot.countries,
    acceptance_previews: update.acceptance_previews ?? snapshot.acceptance_previews ?? [],
    pending_events: update.pending_events ?? snapshot.pending_events,
    guarantees: update.guarantees ?? snapshot.guarantees,
    alliances: update.alliances ?? snapshot.alliances ?? [],
    alliance_offers: update.alliance_offers ?? snapshot.alliance_offers ?? [],
    marriage_offers: update.marriage_offers ?? snapshot.marriage_offers ?? [],
    marriage_adulthood_years: update.marriage_adulthood_years ?? snapshot.marriage_adulthood_years ?? 18,
    opinions: update.opinions ?? snapshot.opinions ?? [],
    truces: update.truces ?? snapshot.truces,
    blockaded_ports: update.blockaded_ports ?? snapshot.blockaded_ports,
    vassals: update.vassals ?? snapshot.vassals,
    vassal_offers: update.vassal_offers ?? snapshot.vassal_offers,
    call_to_arms_offers: update.call_to_arms_offers ?? snapshot.call_to_arms_offers ?? [],
    owners,
    armies: updateRows(snapshot.armies, update.armies, update.removed_armies),
    fleets: updateRows(snapshot.fleets, update.fleets, update.removed_fleets),
    shipyards: update.shipyards ?? snapshot.shipyards ?? [],
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
          .reduce(applyUpdate, normalizeSnapshot(initial));
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
