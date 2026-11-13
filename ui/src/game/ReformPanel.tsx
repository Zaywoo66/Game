import { CountryView, GameCommand, GameSnapshot } from './host';

interface Props {
  game: GameSnapshot;
  country: CountryView | undefined;
  pending: boolean;
  send(command: GameCommand): Promise<void>;
}

function label(key: string): string {
  return key.replace(/_/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function fixed(raw: string): string {
  const value = BigInt(raw);
  const sign = value < 0n ? '−' : '+';
  const absolute = value < 0n ? -value : value;
  const fraction = String(absolute % 1000n).padStart(3, '0').replace(/0+$/, '');
  return `${sign}${absolute / 1000n}${fraction ? `.${fraction}` : ''}`;
}

export function ReformPanel({ game, country, pending, send }: Props) {
  const reforms = (game.reforms ?? []).filter((reform) => reform.government === country?.government);
  const known = new Set(country?.known_reforms ?? []);
  const chosen = new Set(country?.chosen_reform_tiers ?? []);
  const local = country?.id === game.local_country_id;
  const points = country?.research_points ?? 0;
  return (
    <section className="reform-panel" aria-label="Government reforms">
      <h3>Government reforms</h3>
      <p>{country?.name ?? 'No country selected'} · {label(country?.government ?? 'government unavailable')}
        {' · '}{points} research points · 150 per reform</p>
      {!reforms.length && <p>No reforms are defined for this government.</p>}
      {reforms.map((reform) => {
        const alreadyKnown = known.has(reform.key);
        const tierTaken = chosen.has(reform.tier);
        const reason = alreadyKnown ? 'Already selected'
          : tierTaken ? 'This tier already has a reform'
            : points < 150 ? 'Needs 150 research points' : null;
        return (
          <article key={reform.key} className="idea-card" data-reform-key={reform.key}
            data-reform-status={alreadyKnown ? 'known' : reason ? 'locked' : 'ready'}>
            <h4>Tier {reform.tier} · {label(reform.key)}</h4>
            <ul>{reform.effects.map((effect, index) => (
              <li key={`${effect.key}:${index}`}>
                {label(effect.key)} · {fixed(effect.value)} ({effect.kind})
              </li>
            ))}</ul>
            <button type="button" disabled={pending || !local || reason !== null}
              title={reason ?? 'Permanently adopt this reform'}
              onClick={() => country && void send({
                type: 'unlock_reform', country_id: country.id, key: reform.key,
              })}>
              {alreadyKnown ? 'Selected' : 'Adopt · 150'}
            </button>
            {reason && !alreadyKnown && <p className="idea-reason">{reason}</p>}
          </article>
        );
      })}
    </section>
  );
}
