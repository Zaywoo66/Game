/// <reference types="vite/client" />

import { useEffect, useMemo, useRef, useState } from 'react';
import { CountryPanel, HostControls } from '../game/CountryPanel';
import { EventPopup } from '../game/EventPopup';
import { CountryView, GameSnapshot, useGameHost } from '../game/host';
import { LanLobby } from '../game/LanLobby';
import { lanStatusText, useLan } from '../game/lan';
import { ArmyOrder, MilitaryPanel } from '../game/MilitaryPanel';
import { TechnologyPanel } from '../game/TechnologyPanel';
import { loadMapAssets, MapAssets, Province, Rgba } from './mapData';
import { MapMode } from './mapPresentation';
import { BenchmarkSample, createMapViewer, MapTestControls } from './mapViewer';

declare global {
  interface Window {
    strategyMapTest?: MapTestControls;
  }
}

const base = import.meta.env.BASE_URL + 'map/western_europe/';
const options = new URLSearchParams(window.location.search);
const verify = options.get('verify') === '1';
const benchmark = options.get('benchmark') === '1';
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

function MapSources() {
  return (
    <details className="map-sources">
      <summary>Map sources</summary>
      <ul>
        <li>
          <a href="https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.1.3"
            target="_blank" rel="noreferrer">Cliopatria v0.1.3</a> by Bennett et al. (Seshat),
          {' '}<a href="https://creativecommons.org/licenses/by/4.0/"
            target="_blank" rel="noreferrer">CC BY 4.0</a>. Clipped and used to seed test provinces.
        </li>
        <li>
          Land, lakes, and rivers from{' '}
          <a href="https://www.naturalearthdata.com/downloads/"
            target="_blank" rel="noreferrer">Natural Earth</a>.
        </li>
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

export function MapPage() {
  const viewport = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<ReturnType<typeof createMapViewer>>();
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState('');
  const [selected, setSelected] = useState<Province | null>(null);
  const [inspectedCountry, setInspectedCountry] = useState<number | null>(null);
  const [provinceNames, setProvinceNames] = useState<ReadonlyMap<number, Province>>(new Map());
  const [mapAssets, setMapAssets] = useState<MapAssets | null>(null);
  const [mapMode, setMapMode] = useState<MapMode>('historical');
  const modeChosenByUser = useRef(false);
  const [section, setSection] = useState<'country' | 'military' | 'technology'>('country');
  const [inspectedArmy, setInspectedArmy] = useState(0);
  const [order, setOrder] = useState<ArmyOrder | null>(null);
  const [renderer, setRenderer] = useState('');
  const [benchmarkResult, setBenchmarkResult] = useState('Loading the map…');
  const [lobbyOpen, setLobbyOpen] = useState(false);
  const verifyHost = import.meta.env.DEV && options.get('verifyHost') === '1';
  const lan = useLan(!benchmark && !verify);
  const guest = lan.status?.mode === 'guest';
  const guestSuspended = guest && lan.status?.state !== 'connected';
  const hostGame = useGameHost(
    !benchmark && (!verify || verifyHost) && !guestSuspended,
    lan.status?.generation,
    !guestSuspended && !lan.busy && !lan.checking,
    !guest,
  );
  const game = hostGame.snapshot;
  const hasGame = Boolean(game);
  useEffect(() => {
    if (hasGame && !modeChosenByUser.current) setMapMode('political');
  }, [hasGame]);
  const militaryEnabled = game?.armies !== undefined;
  const technologyEnabled = Boolean(game?.technologies?.length);
  const colors = useMemo(
    () => provinceColors(game?.owners, game?.countries),
    [game?.owners, game?.countries],
  );
  const occupationColors = useMemo(
    () => provinceColors(game?.wars?.flatMap((war) => war.occupations), game?.countries),
    [game?.wars, game?.countries],
  );
  const armies = useMemo(() => {
    const countries = new Map(game?.countries.map((country) => [country.id, country.color]));
    return game?.armies?.filter((army) => !army.embarked_on).map((army) => ({
      id: army.id,
      province: army.province_id,
      destination: army.to_province_id,
      progress: army.progress,
      color: countries.get(army.country_id) ?? '#ffffff',
      soldiers: army.soldiers,
    })) ?? [];
  }, [game?.armies, game?.countries]);

  const shownArmy = game?.armies?.find((army) => army.id === inspectedArmy);
  const mayTarget = Boolean(order && shownArmy && shownArmy.country_id === game?.local_country_id);
  const onProvinceClick = useRef<(province: Province | null) => void>(() => undefined);
  useEffect(() => {
    onProvinceClick.current = (province) => {
      setSelected(province);
      setInspectedCountry(null);
      if (order && mayTarget && shownArmy && province?.kind === 'land' && !hostGame.pending) {
        setOrder(null);
        void hostGame.send({
          type: order, country_id: shownArmy.country_id, army_id: shownArmy.id, destination: province.id,
        });
      }
    };
  }, [order, mayTarget, shownArmy, hostGame.pending, hostGame.send]);

  useEffect(() => {
    if (hasGame && status === 'ready') viewerRef.current?.setOwnerColors(colors);
  }, [hasGame, status, colors]);
  useEffect(() => {
    if (status === 'ready') viewerRef.current?.setMapMode(mapMode);
  }, [status, mapMode]);
  useEffect(() => {
    if (status !== 'ready') return;
    viewerRef.current?.setCountryLabels(game?.countries.map((country) => ({
      id: country.capital_id, text: country.name, capital: true,
    })) ?? []);
  }, [status, game?.countries]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setOccupationColors(occupationColors);
  }, [militaryEnabled, status, occupationColors]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setArmies(armies);
  }, [militaryEnabled, status, armies]);
  useEffect(() => {
    if (militaryEnabled && status === 'ready') viewerRef.current?.setArmySelection(shownArmy?.id ?? 0, mayTarget);
  }, [militaryEnabled, status, shownArmy?.id, mayTarget]);

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
          (id) => {
            setSection('military');
            setInspectedArmy(id);
            setOrder(null);
          },
          manualFrames,
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
      if (viewer && window.strategyMapTest === viewer.testControls) delete window.strategyMapTest;
      if (viewerRef.current === viewer) viewerRef.current = undefined;
      viewer?.dispose();
    };
  }, []);

  return (
    <main className="map-page" data-map-status={status} data-map-renderer={renderer} data-map-mode={mapMode}>
      <div className="map-viewport" data-map-viewport ref={viewport} />
      <header className="map-header map-panel">
        <h1>Western Europe</h1>
        {benchmark ? (
          <>
            <p data-map-benchmark role="status">{status === 'error' ? error : benchmarkResult}</p>
            {renderer && <p>{renderer}</p>}
          </>
        ) : (
          <>
            <p>Drag to pan · Scroll to zoom · Right-drag to tilt</p>
            <a href="?smoke=1">WebGL smoke test</a>
            <MapSources />
            <nav className="map-modes" aria-label="Map mode">
              {(['political', 'terrain', 'historical'] as const).map((mode) => (
                <button key={mode} type="button" aria-pressed={mapMode === mode}
                  onClick={() => { modeChosenByUser.current = true; setMapMode(mode); }}>
                  {mode === 'political' ? 'Game political' : mode === 'terrain' ? 'Terrain' : 'Historical 1440'}
                </button>
              ))}
            </nav>
            {mapMode === 'historical' && <p className="map-mode-note">Reference borders (1440), not game ownership.</p>}
            {lan.status && (
              <div className="map-lan">
                <button type="button" onClick={() => setLobbyOpen(true)}>LAN multiplayer</button>
                {lan.status.mode !== 'solo' && (
                  <span data-lan-state={lan.status.state} role="status">{lanStatusText(lan.status)}</span>
                )}
              </div>
            )}
          </>
        )}
      </header>
      {game && <HostControls game={game} pending={hostGame.pending} error={hostGame.commandError}
        canSetSpeed={!guest} send={hostGame.send} />}
      {game && <EventPopup events={game.pending_events ?? []} localCountryId={game.local_country_id}
        pending={hostGame.pending} error={hostGame.commandError} send={hostGame.send} />}
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
      <aside className={'map-inspector map-panel' + (game ? ' map-inspector--host' : '')
        + (section === 'technology' ? ' map-inspector--technology' : '')}
        data-map-selected={selected?.id ?? 0}
        hidden={benchmark || status !== 'ready'}>
        <div aria-live="polite">
          {selected ? (
            <>
              <h2>{provinceName(selected.id)}</h2>
              <p>Province {selected.id} · {nameOf(selected.kind)} · {nameOf(selected.terrain)}</p>
              {game && <p>Owner · {provinceOwner?.name ?? 'Unowned'}</p>}
              {mapMode === 'historical' && selected.kind === 'land' && (
                <p data-historical-resolution={attribution?.resolution ?? 'unmatched'}>
                  1440 source · {attribution?.polity_name ?? 'Unknown attribution'}
                  {attribution?.resolution === 'coarse_group' && ' · Low precision group'}
                </p>
              )}
              {occupier && <p>Occupied by · {occupier.name}</p>}
            </>
          ) : <p>Click a province to inspect it</p>}
        </div>
        {(militaryEnabled || technologyEnabled) && (
          <div className="inspector-tabs" role="group" aria-label="Inspector panels">
            <button type="button" aria-pressed={section === 'country'} onClick={() => {
              setSection('country');
              setOrder(null);
            }}>Country</button>
            {militaryEnabled && <button type="button" aria-pressed={section === 'military'}
              onClick={() => setSection('military')}>Military</button>}
            {technologyEnabled && <button type="button" aria-pressed={section === 'technology'}
              onClick={() => setSection('technology')}>Technology</button>}
          </div>
        )}
        {game && section === 'country' && (
          <CountryPanel game={game} country={shownCountry} pending={hostGame.pending}
            onSelect={setInspectedCountry} send={hostGame.send} />
        )}
        {game && militaryEnabled && section === 'military' && (
          <MilitaryPanel game={game} pending={hostGame.pending} provinceId={selected?.id ?? null}
            provinceIsLand={selected?.kind === 'land'} army={shownArmy} order={order}
            selectArmy={(id) => { setInspectedArmy(id); setOrder(null); }}
            chooseOrder={setOrder} cancelOrder={() => setOrder(null)} provinceName={provinceName}
            send={hostGame.send} readAiLog={hostGame.readAiLog} canReadAiLog={!guest} />
        )}
        {game && technologyEnabled && section === 'technology' && (
          <TechnologyPanel game={game} country={shownCountry} pending={hostGame.pending}
            send={hostGame.send} />
        )}
      </aside>
      {lobbyOpen && lan.status && <LanLobby game={game} lan={lan} onClose={() => setLobbyOpen(false)} />}
    </main>
  );
}
