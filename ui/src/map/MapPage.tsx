/// <reference types="vite/client" />

import { useEffect, useMemo, useRef, useState } from 'react';
import { CountryPanel, HostControls } from '../game/CountryPanel';
import { EventPopup } from '../game/EventPopup';
import { IdeaPanel } from '../game/IdeaPanel';
import { ReformPanel } from '../game/ReformPanel';
import { CountryView, GameSnapshot, useGameHost } from '../game/host';
import { LanLobby } from '../game/LanLobby';
import { LanController, lanStatusText } from '../game/lan';
import { ArmyOrder, MilitaryPanel } from '../game/MilitaryPanel';
import { TechnologyPanel } from '../game/TechnologyPanel';
import { loadMapAssets, MapAssets, Province, Rgba } from './mapData';
import { MapMode } from './mapPresentation';
import { BenchmarkSample, createMapViewer, MapTestControls } from './mapViewer';
import { FleetMarker } from './fleetLayer';

declare global {
  interface Window {
    strategyMapTest?: MapTestControls;
  }
}

const options = new URLSearchParams(window.location.search);
const mapRegion = options.get('map') === 'western_europe' ? 'western_europe' : 'europe';
const base = import.meta.env.BASE_URL + `map/${mapRegion}/`;
const verify = options.get('verify') === '1';
const benchmark = options.get('benchmark') === '1';
const debugUi = options.get('debug') === '1';
const manualFrames = import.meta.env.DEV && !benchmark && (verify || options.get('testFrames') === '1');

function nameOf(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, (character) => character.toUpperCase());
}

function benchmarkText(sample: BenchmarkSample): string {
  const { frameMs, drawingBufferWidth: width, drawingBufferHeight: height } = sample;
  if (!frameMs.length) return 'No frames measured';
  const mean = frameMs.reduce((total, duration) => total + duration, 0) / frameMs.length;
  const sorted = frameMs.slice().sort((left, right) => left - right);
  const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1];
  return (1000 / mean).toFixed(1) + ' FPS at ' + width + ' × ' + height + ' · p95 ' + p95.toFixed(1) + ' ms';
}

function fixedStatus(raw: bigint, signed = false): string {
  const negative = raw < 0n;
  const absolute = negative ? -raw : raw;
  const whole = new Intl.NumberFormat().format(absolute / 1000n);
  const fraction = String(absolute % 1000n).padStart(3, '0').replace(/0+$/, '');
  const sign = negative ? '−' : signed ? '+' : '';
  return `${sign}${whole}${fraction ? `.${fraction}` : ''}`;
}

function CampaignResources({ game }: { game: GameSnapshot }) {
  const country = game.countries.find((row) => row.id === game.local_country_id);
  if (!country) return null;
  const owned = game.owners.filter((row) => row.country_id === country.id).length;
  const armies = game.armies?.filter((army) => army.country_id === country.id && !army.embarked_on) ?? [];
  const fleets = game.fleets?.filter((fleet) => fleet.country_id === country.id) ?? [];
  const soldiers = armies.reduce((total, army) => total + army.soldiers, 0);
  const ships = fleets.reduce((total, fleet) => total + fleet.ships, 0);
  const monthlyNet = BigInt(country.income) - BigInt(country.expenses);
  const entries = [
    { icon: '¤', label: 'Treasury', value: fixedStatus(BigInt(country.treasury)) },
    { icon: '↗', label: 'Monthly net', value: fixedStatus(monthlyNet, true) },
    { icon: '⚔', label: 'Army soldiers', value: new Intl.NumberFormat().format(soldiers) },
    { icon: '⚓', label: 'Fleet ships', value: new Intl.NumberFormat().format(ships) },
    { icon: '⌂', label: 'Owned provinces', value: new Intl.NumberFormat().format(owned) },
  ];
  return (
    <nav className="campaign-resources" aria-label={`${country.name} resources`}>
      {entries.map(({ icon, label, value }) => (
        <span className="campaign-resource" key={label} aria-label={`${label}: ${value}`} title={label}>
          <span className="campaign-resource-icon" aria-hidden="true">{icon}</span>
          <span>{value}</span>
        </span>
      ))}
    </nav>
  );
}

function MapSources() {
  return (
    <details className="map-sources">
      <summary>Map sources</summary>
      <ul>
        <li>
          <a href="https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.2.1"
            target="_blank" rel="noreferrer">Cliopatria v0.2.1</a> by Bennett et al. (Seshat),
          {' '}<a href="https://creativecommons.org/licenses/by/4.0/"
            target="_blank" rel="noreferrer">CC BY 4.0</a>. Clipped and used to seed test provinces.
        </li>
        <li>
          Land, lakes, and rivers from{' '}
          <a href="https://www.naturalearthdata.com/downloads/"
            target="_blank" rel="noreferrer">Natural Earth</a>.
        </li>
        <li>Political coloring uses the cited historical source polygons, not extracted Europa Universalis IV game data.</li>
        <li>
          Elevation and bathymetry from{' '}
          <a href="https://www.ncei.noaa.gov/products/etopo-global-relief-model"
            target="_blank" rel="noreferrer">NOAA ETOPO1</a>.
        </li>
      </ul>
    </details>
  );
}

function provinceColors(
  owners: GameSnapshot['owners'] | undefined,
  countries: CountryView[] | undefined,
): Map<number, Rgba> {
  const colors = new Map<number, Rgba>();
  if (!owners || !countries) return colors;
  const byCountry = new Map<number, Rgba>();
  for (const country of countries) {
    if (!/^#[a-f\d]{6}$/i.test(country.color)) continue;
    const rgb = Number.parseInt(country.color.slice(1), 16);
    byCountry.set(country.id, [rgb >> 16, (rgb >> 8) & 0xff, rgb & 0xff, 255]);
  }
  for (const owner of owners) {
    const color = byCountry.get(owner.country_id);
    if (color) colors.set(owner.province_id, color);
  }
  return colors;
}

function changedProvinceOwners(
  owners: GameSnapshot['owners'] | undefined,
  countries: CountryView[] | undefined,
  initialOwnerTags: ReadonlyMap<number, string> | undefined,
): Set<number> {
  const changed = new Set<number>();
  if (!owners || !countries || !initialOwnerTags) return changed;
  const countryIds = new Map(countries.map((country) => [country.tag, country.id]));
  const liveOwners = new Map(owners.map((owner) => [owner.province_id, owner.country_id]));
  for (const [provinceId, initialTag] of initialOwnerTags) {
    if (liveOwners.get(provinceId) !== countryIds.get(initialTag)) changed.add(provinceId);
  }
  return changed;
}

export function MapPage({ lan, hostGame }: { lan: LanController; hostGame: ReturnType<typeof useGameHost> }) {
  const viewport = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<ReturnType<typeof createMapViewer>>();
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');
  const [selected, setSelected] = useState<Province | null>(null);
  const [hoveredProvince, setHoveredProvince] = useState<{ id: number; x: number; y: number } | null>(null);
  const [inspectedCountry, setInspectedCountry] = useState<number | null>(null);
  const [provinceNames, setProvinceNames] = useState<ReadonlyMap<number, Province>>(new Map());
  const [mapAssets, setMapAssets] = useState<MapAssets | null>(null);
  const [mapMode, setMapMode] = useState<MapMode>('historical');
  const modeChosenByUser = useRef(false);
  const [section, setSection] = useState<'country' | 'military' | 'technology' | 'ideas' | 'reforms'>('country');
  const [inspectedArmy, setInspectedArmy] = useState(0);
  const [inspectedArmies, setInspectedArmies] = useState<number[]>([]);
  const [inspectedFleet, setInspectedFleet] = useState(0);
  const [order, setOrder] = useState<ArmyOrder | null>(null);
  const [renderer, setRenderer] = useState('');
  const [benchmarkResult, setBenchmarkResult] = useState('Loading the map…');
  const [lobbyOpen, setLobbyOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const guest = lan.status?.mode === 'guest';
  const game = hostGame.snapshot;
  const hasGame = Boolean(game);
  const localSuccessionWar = game?.wars?.find((war) => war.succession_country_id === game.local_country_id);
  const localAristocratId = localSuccessionWar?.factions.find((faction) => !faction.country_backed)?.id;
  const isPlayerArmy = (army: NonNullable<GameSnapshot['armies']>[number]) => (
    army.country_id === game?.local_country_id
    && (!localSuccessionWar || army.faction_override === localAristocratId)
  );
  const isPlayerArmyRef = useRef(isPlayerArmy);
  isPlayerArmyRef.current = isPlayerArmy;
  const gameRef = useRef(game);
  gameRef.current = game;
  const campaignLost = game?.countries.some((country) =>
    country.id === game.local_country_id && country.eliminated) ?? false;
  useEffect(() => {
    if (hasGame && !modeChosenByUser.current) setMapMode('political');
  }, [hasGame]);
  const militaryEnabled = game?.armies !== undefined;
  const technologyEnabled = Boolean(game?.technologies?.length);
  const ideasEnabled = Boolean(game?.ideas?.length);
  const reformsEnabled = Boolean(game?.reforms?.length);
  const colors = useMemo(
    () => provinceColors(game?.owners, game?.countries),
    [game?.owners, game?.countries],
  );
  const countryLabels = useMemo(() => {
    if (!game || !mapAssets) return [];
    const ownedByCountry = new Map<number, Set<number>>();
    for (const owner of game.owners) {
      const provinces = ownedByCountry.get(owner.country_id) ?? new Set<number>();
      provinces.add(owner.province_id);
      ownedByCountry.set(owner.country_id, provinces);
    }

    return game.countries.flatMap((country) => {
      const owned = ownedByCountry.get(country.id);
      if (!owned?.size) return [];
      const start = owned.has(country.capital_id) ? country.capital_id : Math.min(...owned);
      const component = new Set<number>([start]);
      const queue = [start];
      for (let cursor = 0; cursor < queue.length; cursor++) {
        const province = mapAssets.provinces.get(queue[cursor]);
        for (const neighbor of province?.neighbors ?? []) {
          if (owned.has(neighbor.id) && !component.has(neighbor.id)) {
            component.add(neighbor.id);
            queue.push(neighbor.id);
          }
        }
      }

      const territory = [...component]
        .map((id) => mapAssets.provinces.get(id))
        .filter((province): province is Province => province !== undefined && province.kind === 'land');
      if (!territory.length) return [];
      const centerX = territory.reduce((sum, province) => sum + province.centroid[0], 0) / territory.length;
      const centerY = territory.reduce((sum, province) => sum + province.centroid[1], 0) / territory.length;
      const anchor = territory.reduce((closest, province) =>
        (province.centroid[0] - centerX) ** 2 + (province.centroid[1] - centerY) ** 2
          < (closest.centroid[0] - centerX) ** 2 + (closest.centroid[1] - centerY) ** 2
          ? province : closest);
      const padding = Math.max(12, Math.min(80, Math.sqrt(territory.length) * 14));
      return [{
        id: anchor.id,
        text: country.name,
        capital: true,
        extent: [
          Math.min(...territory.map((province) => province.centroid[0])) - padding,
          Math.min(...territory.map((province) => province.centroid[1])) - padding,
          Math.max(...territory.map((province) => province.centroid[0])) + padding,
          Math.max(...territory.map((province) => province.centroid[1])) + padding,
        ] as [number, number, number, number],
      }];
    });
  }, [game?.countries, game?.owners, mapAssets]);
  const changedOwners = useMemo(
    () => changedProvinceOwners(game?.owners, game?.countries, mapAssets?.initialOwnerTags),
    [game?.owners, game?.countries, mapAssets?.initialOwnerTags],
  );
  const occupationColors = useMemo(
    () => provinceColors(game?.wars?.flatMap((war) => war.occupations), game?.countries),
    [game?.wars, game?.countries],
  );
  const armies = useMemo(() => {
    const countries = new Map(game?.countries.map((country) => [country.id, country.color]));
    return game?.armies?.filter((army) => !army.embarked_on).map((army) => ({
      id: army.id,
      countryId: army.country_id,
      province: army.province_id,
      destination: army.to_province_id,
      progress: army.progress,
      color: army.faction_override === null ? countries.get(army.country_id) ?? '#ffffff' : '#ba654f',
      soldiers: army.soldiers,
      recruitingDays: army.recruitment_days_remaining,
    })) ?? [];
  }, [game?.armies, game?.countries]);
  const fleets: FleetMarker[] = useMemo(() => {
    const countries = new Map(game?.countries.map((country) => [country.id, country.color]));
    return game?.fleets?.map((fleet) => ({
      id: fleet.id,
      province: fleet.sea_province_id,
      destination: fleet.to_sea_province_id,
      progress: fleet.progress,
      color: countries.get(fleet.country_id) ?? '#ffffff',
      ships: fleet.ships,
    })) ?? [];
  }, [game?.fleets, game?.countries]);

  const shownArmies = game?.armies?.filter((army) => inspectedArmies.includes(army.id)) ?? [];
  const shownArmy = shownArmies.find((army) => army.id === inspectedArmy) ?? shownArmies[0];
  const mayTarget = Boolean(order && shownArmies.length > 0 && game?.local_country_id !== null
    && shownArmies.every(isPlayerArmy));
  const onProvinceClick = useRef<(province: Province | null) => void>(() => undefined);
  const hoverTimer = useRef<number | undefined>(undefined);
  const onProvinceHover = useRef<(id: number, x: number, y: number) => void>(() => undefined);
  useEffect(() => {
    onProvinceHover.current = (id, x, y) => {
      window.clearTimeout(hoverTimer.current);
      if (id === 0) {
        setHoveredProvince(null);
        return;
      }
      hoverTimer.current = window.setTimeout(() => setHoveredProvince({ id, x, y }), 300);
    };
  }, []);
  useEffect(() => {
    onProvinceClick.current = (province) => {
      setSelected(province);
      setInspectedCountry(null);
      if (order && mayTarget && shownArmies.length > 0 && province?.kind === 'land' && !hostGame.pending) {
        setOrder(null);
        void hostGame.sendMany(shownArmies.map((army) => ({
          type: order, country_id: army.country_id, army_id: army.id, destination: province.id,
        })));
      }
    };
  }, [order, mayTarget, shownArmies, hostGame.pending, hostGame.sendMany]);

  useEffect(() => {
    if (status === 'ready') viewerRef.current?.setOwnerColors(colors, changedOwners);
  }, [status, colors, changedOwners]);
  useEffect(() => {
    if (status === 'ready') viewerRef.current?.setMapMode(mapMode);
  }, [status, mapMode]);
  useEffect(() => {
    if (status !== 'ready') return;
    viewerRef.current?.setCountryLabels(countryLabels);
  }, [status, countryLabels]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setOccupationColors(occupationColors);
  }, [militaryEnabled, status, occupationColors]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setArmies(armies);
  }, [militaryEnabled, status, armies]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setFleets(fleets);
  }, [militaryEnabled, status, fleets]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setArmyGroupSelection(inspectedArmies, mayTarget);
  }, [militaryEnabled, status, inspectedArmies, mayTarget]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setFleetSelection(inspectedFleet);
  }, [militaryEnabled, status, inspectedFleet]);

  const ownerId = selected && game?.owners.find((owner) => owner.province_id === selected.id)?.country_id;
  const shownCountryId = inspectedCountry ?? (selected ? ownerId : game?.local_country_id ?? game?.countries[0]?.id);
  const shownCountry = game?.countries.find((country) => country.id === shownCountryId);
  const provinceOwner = game?.countries.find((country) => country.id === ownerId);
  const occupierId = game?.wars?.flatMap((war) => war.occupations)
    .find((occupation) => occupation.province_id === selected?.id)?.country_id;
  const occupier = game?.countries.find((country) => country.id === occupierId);
  const provinceName = (id: number) => {
    const province = provinceNames.get(id);
    return province ? mapAssets?.names.get(province.name_key ?? '') ?? 'Province ' + id : 'Province ' + id;
  };
  const attribution = selected && mapAssets?.polityAttributions.get(selected.id);

  useEffect(() => {
    const host = viewport.current;
    if (!host) return;

    const abort = new AbortController();
    let viewer: ReturnType<typeof createMapViewer> | undefined;
    let warmupTimer: number | undefined;
    let measurementTimer: number | undefined;
    loadMapAssets(base, abort.signal)
      .then((assets) => {
        if (abort.signal.aborted) return;
        const { provinces } = assets;
        const active = createMapViewer(
          host,
          assets,
          (id) => onProvinceClick.current(provinces.get(id) ?? null),
          (id, x, y) => onProvinceHover.current(id, x, y),
          (id) => {
            setSection('military');
            setInspectedArmy(id);
            setInspectedArmies([id]);
            setOrder(null);
          },
          (id) => {
            setSection('military');
            setInspectedFleet(id);
            setOrder(null);
          },
          manualFrames,
          (ids) => {
            const currentGame = gameRef.current;
            const controlled = ids.filter((id) => currentGame?.armies?.some((army) => (
              army.id === id && isPlayerArmyRef.current(army)
            )));
            setInspectedArmies(controlled);
            setInspectedArmy(controlled[0] ?? 0);
            if (controlled.length) setSection('military');
            setOrder(null);
          },
        );
        viewer = active;
        viewerRef.current = active;
        setProvinceNames(provinces);
        setMapAssets(assets);
        setRenderer(active.renderer);
        if (verify) window.strategyMapTest = active.testControls;
        setStatus('ready');

        if (benchmark) {
          setBenchmarkResult('Warming up…');
          warmupTimer = window.setTimeout(() => {
            active.startBenchmark();
            setBenchmarkResult('Measuring for 10 seconds…');
            measurementTimer = window.setTimeout(() => {
              setBenchmarkResult(benchmarkText(active.readBenchmark()));
            }, 10_000);
          }, 2_000);
        }
      })
      .catch((reason: unknown) => {
        if (abort.signal.aborted) return;
        const message = reason instanceof Error ? reason.message : String(reason);
        console.error('Map startup failed:', reason);
        setError(message);
        setStatus('error');
      });

    return () => {
      abort.abort();
      window.clearTimeout(warmupTimer);
      window.clearTimeout(measurementTimer);
      window.clearTimeout(hoverTimer.current);
      if (viewer && window.strategyMapTest === viewer.testControls) delete window.strategyMapTest;
      if (viewerRef.current === viewer) viewerRef.current = undefined;
      viewer?.dispose();
    };
  }, []);

  return (
    <main className="map-page" data-map-status={status} data-map-renderer={renderer} data-map-mode={mapMode}
      data-map-selected-fleet={inspectedFleet}>
      <div className="map-viewport" data-map-viewport ref={viewport} />
      {campaignLost && <section className="campaign-loss-overlay" role="alertdialog" aria-modal="true"
        aria-labelledby="campaign-loss-title">
        <p className="campaign-loss-kicker">Campaign concluded</p>
        <h2 id="campaign-loss-title">Your country has been fully annexed</h2>
        <p>This campaign is over. A partial peace settlement would not end the game.</p>
      </section>}
      <header className="map-header map-panel">
        <h1>{game?.countries.find((country) => country.id === game.local_country_id)?.name
          ?? (mapRegion === 'europe' ? 'Strategy · Europe' : 'Strategy · Western Europe')}</h1>
        {benchmark ? (
          <>
            <p data-map-benchmark role="status">{status === 'error' ? error : benchmarkResult}</p>
            {renderer && <p>{renderer}</p>}
          </>
        ) : (
          <>
            <details className="map-controls-help">
              <summary>Map controls</summary>
              <p>Drag to pan · WASD/arrows · Scroll to zoom · Double-click to focus · Home for region</p>
            </details>
            {debugUi && <>
              <a href="?smoke=1">WebGL smoke test</a>
              <MapSources />
            </>}
            <nav className="map-modes" aria-label="Map mode">
              {(['political', 'terrain', 'historical'] as const).map((mode) => (
                <button key={mode} type="button" aria-pressed={mapMode === mode}
                  onClick={() => { modeChosenByUser.current = true; setMapMode(mode); }}>
                  {mode === 'political' ? 'Game political' : mode === 'terrain' ? 'Terrain' : 'Historical 1440'}
                </button>
              ))}
            </nav>
            {mapMode === 'historical' && <p className="map-mode-note">
              1440 reference colors · Solid tint: named polity · Muted tint: unresolved placeholder.
              Scandinavian realm labels are visual references; placeholder colors do not assert game ownership.
            </p>}
            {game && mapMode === 'political' && <p className="map-mode-note">
              Solid fill: campaign owner · Unowned frontier: physical atlas terrain.
            </p>}
            {lan.status && lan.status.mode !== 'solo' && (
              <div className="map-lan">
                <button type="button" onClick={() => setLobbyOpen(true)}>Room details</button>
                <span data-lan-state={lan.status.state} role="status">{lanStatusText(lan.status)}</span>
              </div>
            )}
          </>
        )}
      </header>
      {game && <HostControls game={game} pending={hostGame.pending} error={hostGame.commandError}
        canSetSpeed={!guest} send={hostGame.send} />}
      {game && <CampaignResources game={game} />}
      {game && <EventPopup events={game.pending_events ?? []} localCountryId={game.local_country_id}
        pending={hostGame.pending} error={hostGame.commandError} send={hostGame.send} />}
      {hoveredProvince && !benchmark && (() => {
        const province = provinceNames.get(hoveredProvince.id);
        const name = province ? mapAssets?.names.get(province.name_key ?? '') ?? `Province ${hoveredProvince.id}` : '';
        return name ? <div className="map-hover-tooltip" role="tooltip"
          style={{ left: Math.min(hoveredProvince.x + 14, window.innerWidth - 250),
            top: Math.min(hoveredProvince.y + 14, window.innerHeight - 56) }}>
          {name}
        </div> : null;
      })()}
      {hostGame.connected && !game && (
        <p className="game-clock map-panel" role={hostGame.connectionError ? 'alert' : 'status'}>
          {hostGame.connectionError || 'Connecting to the game host…'}
        </p>
      )}
      {!benchmark && status !== 'ready' && (
        <p className="map-status map-panel" role="status">
          {status === 'loading' ? 'Loading terrain and provinces…' : error}
        </p>
      )}
      {!inspectorOpen && !benchmark && status === 'ready' && (
        <button className="inspector-toggle inspector-toggle--closed" type="button"
          aria-controls="map-inspector" aria-expanded={false}
          onClick={() => setInspectorOpen(true)}>Open panel</button>
      )}
      <aside id="map-inspector" className={'map-inspector map-panel' + (game ? ' map-inspector--host' : '')
        + (section === 'technology' || section === 'ideas' || section === 'reforms' ? ' map-inspector--technology' : '')}
        data-map-selected={selected?.id ?? 0}
        hidden={benchmark || status !== 'ready' || !inspectorOpen}>
        <div className="inspector-heading">
          <span className="inspector-kicker">Inspector</span>
          <button className="inspector-toggle" type="button" aria-controls="map-inspector"
            aria-expanded={inspectorOpen} onClick={() => setInspectorOpen(false)}>Hide</button>
        </div>
        <div aria-live="polite">
          {selected ? (
            <>
              <h2>{provinceName(selected.id)}</h2>
              <p>Province {selected.id} · {nameOf(selected.kind)} · {nameOf(selected.terrain)}</p>
              {game && <p>Owner · {provinceOwner?.name ?? 'Unowned'}</p>}
              {game?.blockaded_ports?.includes(selected.id) && <p>Blockaded · port tax income closed</p>}
              {mapMode === 'historical' && selected.kind === 'land' && (
                <p data-historical-resolution={attribution?.resolution ?? 'unmatched'}>
                  1440 source · {attribution?.polity_name === 'Kalmar Union'
                    ? 'Scandinavian kingdoms · separate reference tints'
                    : attribution?.polity_name ?? 'Unknown attribution'}
                  {attribution?.resolution === 'coarse_group' && ' · Low precision group'}
                </p>
              )}
              {occupier && <p>Occupied by · {occupier.name}</p>}
            </>
          ) : <p>Click a province to inspect it</p>}
        </div>
        {(militaryEnabled || technologyEnabled || ideasEnabled || reformsEnabled || (game && game.local_country_id !== null)) && (
          <div className="inspector-tabs" role="group" aria-label="Inspector panels">
            <button type="button" aria-pressed={section === 'country'} onClick={() => {
              setSection('country');
              setOrder(null);
            }}><span className="inspector-tab-icon" aria-hidden="true">⌂</span>Country</button>
            {militaryEnabled && <button type="button" aria-pressed={section === 'military'}
              onClick={() => setSection('military')}>
              <span className="inspector-tab-icon" aria-hidden="true">⚔</span>Military</button>}
            {technologyEnabled && <button type="button" aria-pressed={section === 'technology'}
              onClick={() => setSection('technology')}>
              <span className="inspector-tab-icon" aria-hidden="true">⌘</span>Technology</button>}
            {ideasEnabled && <button type="button" aria-pressed={section === 'ideas'}
              onClick={() => setSection('ideas')}>
              <span className="inspector-tab-icon" aria-hidden="true">✦</span>Ideas</button>}
            {reformsEnabled && <button type="button" aria-pressed={section === 'reforms'}
              onClick={() => setSection('reforms')}>
              <span className="inspector-tab-icon" aria-hidden="true">⚖</span>Reforms</button>}
          </div>
        )}
        {game && section === 'country' && (
          <CountryPanel game={game} country={shownCountry} pending={hostGame.pending}
            onSelect={setInspectedCountry} send={hostGame.send} />
        )}
        {game && militaryEnabled && section === 'military' && (
          <MilitaryPanel game={game} pending={hostGame.pending} provinceId={selected?.id ?? null}
            provinceIsLand={selected?.kind === 'land'}
            portSeaIds={selected?.neighbors.filter((edge) => edge.edge === 'port').map((edge) => edge.id)}
            army={shownArmy} selectedArmyCount={shownArmies.length} order={order}
            selectArmy={(id) => { setInspectedArmy(id); setInspectedArmies(id > 0 ? [id] : []); setOrder(null); }}
            selectedFleetId={inspectedFleet}
            chooseOrder={setOrder} cancelOrder={() => setOrder(null)} provinceName={provinceName}
            send={hostGame.send} readAiLog={hostGame.readAiLog} canReadAiLog={!guest} />
        )}
        {game && technologyEnabled && section === 'technology' && (
          <TechnologyPanel game={game} country={shownCountry} pending={hostGame.pending}
            send={hostGame.send} />
        )}
        {game && ideasEnabled && section === 'ideas' && (
          <IdeaPanel game={game} country={shownCountry} pending={hostGame.pending}
            send={hostGame.send} />
        )}
        {game && reformsEnabled && section === 'reforms' && (
          <ReformPanel game={game} country={shownCountry} pending={hostGame.pending}
            send={hostGame.send} />
        )}
      </aside>
      {lobbyOpen && lan.status && <LanLobby game={game} lan={lan} onClose={() => setLobbyOpen(false)} />}
    </main>
  );
}
