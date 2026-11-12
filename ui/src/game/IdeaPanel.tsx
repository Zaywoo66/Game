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

export function IdeaPanel({ game, country, pending, send }: Props) {
  const ideas = (game.ideas ?? []).filter((idea) => idea.country === country?.tag);
  const known = new Set(country?.known_ideas ?? []);
  const local = country?.id === game.local_country_id;
  const points = country?.research_points ?? 0;
  return (
    <section className="idea-panel" aria-label="National ideas">
      <h3>National ideas</h3>
      <p>{country?.name ?? 'No country selected'} · {points} research points · 100 per idea</p>
      {!ideas.length && <p>No national ideas are defined for this country yet.</p>}
      {ideas.map((idea) => {
        const unlocked = country ? idea.available_countries.includes(country.id) : false;
        const alreadyKnown = known.has(idea.key);
        const reason = alreadyKnown ? 'Already unlocked'
          : !unlocked ? 'Unlock condition is not met'
            : points < 100 ? 'Needs 100 research points' : null;
        return (
          <article key={idea.key} className="idea-card" data-idea-key={idea.key}
            data-idea-status={alreadyKnown ? 'known' : reason ? 'locked' : 'ready'}>
            <h4>{label(idea.name_key)}</h4>
            <ul>{idea.effects.map((effect, index) => (
              <li key={`${effect.key}:${index}`}>
                {label(effect.key)} · {fixed(effect.value)} ({effect.kind})
              </li>
            ))}</ul>
            <button type="button" disabled={pending || !local || reason !== null}
              title={reason ?? 'Permanently unlock this national idea'}
              onClick={() => country && void send({
                type: 'unlock_idea', country_id: country.id, key: idea.key,
              })}>
              {alreadyKnown ? 'Unlocked' : 'Unlock · 100'}
            </button>
            {reason && !alreadyKnown && <p className="idea-reason">{reason}</p>}
          </article>
        );
      })}
    </section>
  );
}
