import { useState } from 'react';
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
  const [targetGovernment, setTargetGovernment] = useState('monarchy');
  const [monarchName, setMonarchName] = useState('');
  const [dynasty, setDynasty] = useState('');
  const [monarchSex, setMonarchSex] = useState<'male' | 'female'>('male');
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
      {country && <section className="government-transition" aria-label="Government and stability">
        <p>Stability {country.stability}/3 · Administrative {country.admin_points} · Diplomatic {country.diplomatic_points} · Military {country.military_points}</p>
        <p>War exhaustion {country.war_exhaustion}</p>
        <button type="button" disabled={pending || !local || country.stability >= 3 || country.admin_points < 100}
          onClick={() => void send({ type: 'increase_stability', country_id: country.id })}>
          Increase stability · 100 ADM
        </button>
        <h4>Change government · 500 research points · 90 days</h4>
        {country.government_change_pending ? <p>Government transition in progress.</p> :
          <>
            <label>New government <select value={targetGovernment} onChange={(event) => setTargetGovernment(event.target.value)}>
              {country.government_options.filter((item) => item !== country.government).map((item) =>
                <option key={item} value={item}>{label(item)}</option>)}
            </select></label>
            {targetGovernment === 'monarchy' && <div className="new-monarch-fields">
              <label>Monarch name <input value={monarchName} maxLength={64} onChange={(event) => setMonarchName(event.target.value)} /></label>
              <label>Dynasty <input value={dynasty} maxLength={64} onChange={(event) => setDynasty(event.target.value)} /></label>
              <label>Sex <select value={monarchSex} onChange={(event) => setMonarchSex(event.target.value as 'male' | 'female')}>
                <option value="male">Male</option><option value="female">Female</option>
              </select></label>
            </div>}
            <button type="button" disabled={pending || !local || !targetGovernment
              || (targetGovernment === 'monarchy' && (!monarchName.trim() || !dynasty.trim()))
              || points < 500}
              onClick={() => country && void send({
                type: 'change_government', country_id: country.id, target: targetGovernment,
                ...(targetGovernment === 'monarchy' ? {
                  monarch_name: monarchName.trim(), dynasty: dynasty.trim(), monarch_sex: monarchSex,
                } : {}),
              })}>Begin transition</button>
          </>}
      </section>}
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
