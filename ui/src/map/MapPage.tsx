/// <reference types="vite/client" />

import { useEffect, useMemo, useRef, useState } from 'react';
import { CountryPanel, HostControls } from '../game/CountryPanel';
import { CountryView, GameSnapshot, useGameHost } from '../game/host';
import { loadMapAssets, Province, Rgba } from './mapData';
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
  const [renderer, setRenderer] = useState('');
  const [benchmarkResult, setBenchmarkResult] = useState('Loading the map…');
  const hostGame = useGameHost(!verify && !benchmark);
  const game = hostGame.snapshot;
  const hasGame = Boolean(game);
  const colors = useMemo(
    () => provinceColors(game?.owners, game?.countries),
    [game?.owners, game?.countries],
  );

  useEffect(() => {
    if (hasGame && status === 'ready') viewerRef.current?.setOwnerColors(colors);
  }, [hasGame, status, colors]);

  const ownerId = selected && game?.owners.find((owner) => owner.province_id === selected.id)?.country_id;
  const shownCountryId = inspectedCountry ?? (selected ? ownerId : game?.local_country_id ?? game?.countries[0]?.id);
  const shownCountry = game?.countries.find((country) => country.id === shownCountryId);
  const provinceOwner = game?.countries.find((country) => country.id === ownerId);

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
        const active = createMapViewer(host, assets, (id) => {
          setSelected(provinces.get(id) ?? null);
          setInspectedCountry(null);
        });
        viewer = active;
        viewerRef.current = active;
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
    <main className="map-page" data-map-status={status} data-map-renderer={renderer}>
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
          </>
        )}
      </header>
      {game && <HostControls game={game} pending={hostGame.pending} error={hostGame.commandError} send={hostGame.send} />}
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
      <aside className={'map-inspector map-panel' + (game ? ' map-inspector--host' : '')}
        data-map-selected={selected?.id ?? 0}
        hidden={benchmark || status !== 'ready'}>
        <div aria-live="polite">
          {selected ? (
            <>
              <h2>{nameOf(selected.key)}</h2>
              <p>Province {selected.id} · {nameOf(selected.kind)} · {nameOf(selected.terrain)}</p>
              {game && <p>Owner · {provinceOwner?.name ?? 'Unowned'}</p>}
            </>
          ) : <p>Click a province to inspect it</p>}
        </div>
        {game && (
          <CountryPanel game={game} country={shownCountry} pending={hostGame.pending}
            onSelect={setInspectedCountry} send={hostGame.send} />
        )}
      </aside>
    </main>
  );
}
