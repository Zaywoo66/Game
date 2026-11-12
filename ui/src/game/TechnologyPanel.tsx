import { useEffect, useState } from 'react';
import { CountryView, GameCommand, GameSnapshot, TechnologyView } from './host';

const COLUMN_WIDTH = 190;
const ROW_HEIGHT = 190;
const NODE_WIDTH = 168;
const NODE_HEIGHT = 166;

type Locale = Record<string, string>;

function useTechnologyLocale(): Locale {
  const [locale, setLocale] = useState<Locale>({});
  useEffect(() => {
    const controller = new AbortController();
    void fetch(import.meta.env.BASE_URL + 'locales/techs.en.json', { signal: controller.signal })
      .then((response) => {
        if (!response.ok) throw new Error(`Technology locale: HTTP ${response.status}`);
        return response.json() as Promise<unknown>;
      })
      .then((catalog) => {
        if (catalog && typeof catalog === 'object' && !Array.isArray(catalog)) {
          setLocale(Object.fromEntries(Object.entries(catalog).filter((entry): entry is [string, string] =>
            typeof entry[1] === 'string')));
        }
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted) console.error('Could not load technology locale:', error);
      });
    return () => controller.abort();
  }, []);
  return locale;
}

function fixed(raw: string): string {
  const value = BigInt(raw);
  const absolute = value < 0n ? -value : value;
  const whole = absolute / 1000n;
  const fraction = String(absolute % 1000n).padStart(3, '0').replace(/0+$/, '');
  return `${value < 0n ? '-' : '+'}${whole}${fraction ? `.${fraction}` : ''}`;
}

function statusReason(
  node: TechnologyView, country: CountryView | undefined, localCountryId: number | null,
  known: ReadonlySet<string>, localize: (kind: string, key: string) => string,
): string | null {
  if (!country) return 'Select a country';
  if (country.id !== localCountryId) return 'Only your country can research';
  if (known.has(node.key)) return 'Already researched';
  const missing = node.requires.filter((required) => !known.has(required));
  if (missing.length) return `Requires ${missing.map((key) => localize('tech', key)).join(', ')}`;
  if (country.research_points === null || country.research_points === undefined) {
    return 'Research is unavailable';
  }
  if (country.research_points < node.cost) {
    return `Needs ${node.cost - country.research_points} more research points`;
  }
  return null;
}

interface Props {
  game: GameSnapshot;
  country: CountryView | undefined;
  pending: boolean;
  send(command: GameCommand): Promise<void>;
}

export function TechnologyPanel({ game, country, pending, send }: Props) {
  const locale = useTechnologyLocale();
  const nodes = game.technologies ?? [];
  const localize = (kind: string, key: string) => locale[`${kind}.${key}`] ?? key;
  const known = new Set(country?.known_technologies ?? []);
  if (!nodes.length) return <section className="technology-panel">No technology catalog is loaded.</section>;

  const minX = Math.min(...nodes.map((node) => node.pos_x));
  const minY = Math.min(...nodes.map((node) => node.pos_y));
  const maxX = Math.max(...nodes.map((node) => node.pos_x));
  const maxY = Math.max(...nodes.map((node) => node.pos_y));
  const width = (maxX - minX) * COLUMN_WIDTH + NODE_WIDTH;
  const height = (maxY - minY) * ROW_HEIGHT + NODE_HEIGHT;
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const position = (node: TechnologyView) => ({
    x: (node.pos_x - minX) * COLUMN_WIDTH,
    y: (node.pos_y - minY) * ROW_HEIGHT,
  });

  return (
    <section className="technology-panel" aria-label="Technology tree">
      <h3>Technology tree</h3>
      <p>{country?.name ?? 'No country selected'} · {country?.research_points ?? 0} research points</p>
      <div className="technology-scroll">
        <div className="technology-canvas" style={{ width, height }}>
          <svg className="technology-links" width={width} height={height} aria-hidden="true">
            {nodes.flatMap((node) => node.requires.flatMap((required) => {
              const parent = byKey.get(required);
              if (!parent) return [];
              const from = position(parent);
              const to = position(node);
              return [<line key={`${required}:${node.key}`} x1={from.x + NODE_WIDTH / 2}
                y1={from.y + NODE_HEIGHT / 2} x2={to.x + NODE_WIDTH / 2}
                y2={to.y + NODE_HEIGHT / 2} />];
            }))}
          </svg>
          {nodes.map((node) => {
            const reason = statusReason(node, country, game.local_country_id, known, localize);
            const name = localize('tech', node.key);
            const tooltip = [
              `${name} · ${node.cost} research points`,
              ...node.effects.map((effect) => (
                `${localize('modifier', effect.key)} ${fixed(effect.value)} `
                + `${effect.kind} · Source: ${localize('tech', effect.source_key)}`
              )),
              reason ?? 'Ready to research',
            ].join('\n');
            const { x, y } = position(node);
            return (
              <article key={node.key} className="technology-node" data-tech-key={node.key}
                data-tech-x={node.pos_x} data-tech-y={node.pos_y}
                data-tech-status={known.has(node.key) ? 'known' : reason ? 'locked' : 'ready'}
                style={{ left: x, top: y }} title={tooltip}>
                <small>{localize('branch', node.branch)}</small>
                <h4>{name}</h4>
                <p>Cost: {node.cost}</p>
                {node.requires.length > 0 && <p>Requires: {node.requires.map((key) =>
                  localize('tech', key)).join(', ')}</p>}
                <button type="button" disabled={pending || reason !== null} title={reason ?? tooltip}
                  onClick={() => country && void send({
                    type: 'unlock_technology', country_id: country.id, key: node.key,
                  })}>
                  {known.has(node.key) ? 'Researched' : 'Research'}
                </button>
                {reason && !known.has(node.key) && <p className="technology-reason">{reason}</p>}
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}
