import { FormEvent, useState } from 'react';
import { CharacterView, CountryView, GameCommand, GameSnapshot } from './host';

/** Money is an i64 in thousandths. Keep all three digits without Number conversion. */
function fixedText(raw: string | bigint): string {
  const integer = BigInt(raw);
  const absolute = integer < 0n ? -integer : integer;
  const whole = absolute / 1000n;
  const fraction = String(absolute % 1000n).padStart(3, '0').replace(/0+$/, '');
  return (integer < 0n ? '-' : '') + whole + (fraction ? '.' + fraction : '');
}

function decimalThousandths(text: string): bigint | null {
  const trimmed = text.trim();
  if (!/^\d{1,19}(?:\.\d{1,3})?$/.test(trimmed)) return null;
  const [whole, fraction = ''] = trimmed.split('.');
  return BigInt(whole) * 1000n + BigInt(fraction.padEnd(3, '0'));
}

const percent = (rate: number) => fixedText(BigInt(rate) * 100n);
const readable = (key: string) => key.replace(/_/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());

interface HostControlsProps {
  game: GameSnapshot;
  pending: boolean;
  error: string;
  canSetSpeed?: boolean;
  send(command: GameCommand): Promise<void>;
}

export function HostControls({ game, pending, error, canSetSpeed = true, send }: HostControlsProps) {
  const clock = game.clock;
  return (
    <section className="game-clock map-panel" aria-label="Game clock">
      <time dateTime={clock.date}>{clock.date}</time>
      <div className="game-clock-actions">
        <button type="button" disabled={pending}
          onClick={() => void send({ type: 'set_paused', paused: !clock.paused })}>
          {clock.paused ? 'Resume' : 'Pause'}
        </button>
        <label>
          Speed
          <select value={clock.speed} disabled={pending || !canSetSpeed}
            title={canSetSpeed ? undefined : 'The host sets the game speed'}
            onChange={(event) => void send({ type: 'set_speed', speed: Number(event.target.value) })}>
            {[1, 2, 3, 4, 5].map((speed) => <option key={speed} value={speed}>{speed}</option>)}
          </select>
        </label>
      </div>
      {error && <p className="game-error" role="alert">{error}</p>}
    </section>
  );
}

interface FiscalPolicyProps {
  country: CountryView;
  game: GameSnapshot;
  pending: boolean;
  send(command: GameCommand): Promise<void>;
}

function FiscalPolicyForm({ country, game, pending, send }: FiscalPolicyProps) {
  const [tax, setTax] = useState(percent(country.tax_rate));
  const [spending, setSpending] = useState(fixedText(country.monthly_spending));
  const [error, setError] = useState('');

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const taxInput = decimalThousandths(tax);
    const spendInput = decimalThousandths(spending);
    if (taxInput === null || taxInput % 100n !== 0n || taxInput / 100n > BigInt(game.max_tax_rate)) {
      setError('Tax must be between 0% and ' + percent(game.max_tax_rate) + '% in steps of 0.1%.');
      return;
    }
    if (spendInput === null || spendInput > BigInt(game.max_monthly_spending)) {
      setError('Spending must be between 0 and ' + fixedText(game.max_monthly_spending) + '.');
      return;
    }
    setError('');
    void send({
      type: 'set_fiscal_policy',
      country_id: country.id,
      tax_rate: Number(taxInput / 100n),
      monthly_spending: spendInput.toString(),
    });
  }

  return (
    <form className="fiscal-policy" onSubmit={submit} aria-label="Fiscal policy">
      <div className="fiscal-fields">
        <label>Tax rate (%)
          <input value={tax} onChange={(event) => setTax(event.target.value)}
            inputMode="decimal" disabled={pending} required />
        </label>
        <label>Monthly spending
          <input value={spending} onChange={(event) => setSpending(event.target.value)}
            inputMode="decimal" disabled={pending} required />
        </label>
      </div>
      <button type="submit" disabled={pending}>Apply fiscal policy</button>
      {error && <p className="game-error" role="alert">{error}</p>}
    </form>
  );
}

function CharacterPanel({ title, character, date }: { title: string; character: CharacterView | null; date: string }) {
  return (
    <section className="character-panel" aria-label={title}>
      <h3>{title}</h3>
      {character ? (
        <>
          <p className="character-name">{character.name}</p>
          <p>{character.dynasty} · Age {character.age}</p>
          {character.death && character.death <= date && <p>Died {character.death}</p>}
          <dl className="character-stats">
            <div><dt><abbr title="Administration">ADM</abbr></dt><dd>{character.adm}</dd></div>
            <div><dt><abbr title="Diplomacy">DIP</abbr></dt><dd>{character.dip}</dd></div>
            <div><dt><abbr title="Military">MIL</abbr></dt><dd>{character.mil}</dd></div>
          </dl>
          {character.traits.length > 0 && (
            <p className="character-traits">Traits · {character.traits.map(readable).join(', ')}</p>
          )}
        </>
      ) : <p>No {title.toLowerCase()} recorded</p>}
    </section>
  );
}

interface CountryPanelProps {
  game: GameSnapshot;
  country: CountryView | undefined;
  pending: boolean;
  onSelect(id: number): void;
  send(command: GameCommand): Promise<void>;
}

export function CountryPanel({ game, country, pending, onSelect, send }: CountryPanelProps) {
  const local = game.local_country_id;
  const rulerOf = (id: number) => game.countries.find((row) => row.id === id)?.name ?? 'Country ' + id;
  const subjectBond = country && game.vassals.find((bond) => bond.subject_id === country.id);
  const incoming = game.vassal_offers.filter((offer) => offer.subject_id === local);
  return (
    <section className="country-panel" aria-label="Country and rulers">
      <label className="country-picker">
        Country
        <select value={country?.id ?? ''} onChange={(event) => onSelect(Number(event.target.value))}>
          <option value="" disabled>Choose a country</option>
          {game.countries.map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}
        </select>
      </label>
      {country && (
        <>
          <h2 className="country-name">
            <span aria-hidden="true" style={{ backgroundColor: country.color }} />
            {country.name}
          </h2>
          <p>Capital · {country.capital}</p>
          <dl className="country-economy">
            <div><dt>Treasury</dt><dd>{fixedText(country.treasury)}</dd></div>
            <div><dt>Tax rate</dt><dd>{percent(country.tax_rate)}%</dd></div>
            <div><dt>Monthly income</dt><dd>{fixedText(country.income)}</dd></div>
            <div><dt>Tribute paid</dt><dd>{fixedText(country.tribute_paid)}</dd></div>
            <div><dt>Tribute received</dt><dd>{fixedText(country.tribute_received)}</dd></div>
            <div><dt>Monthly expenses</dt><dd>{fixedText(country.expenses)}</dd></div>
          </dl>
          {country.bankrupt && <p className="game-error">Bankrupt</p>}
          {country.id === game.local_country_id && (
            <FiscalPolicyForm key={country.id + ':' + country.tax_rate + ':' + country.monthly_spending}
              country={country} game={game} pending={pending} send={send} />
          )}
          <section aria-label="Vassalage">
            <h3>Vassalage</h3>
            {subjectBond ? <p>Overlord · {rulerOf(subjectBond.overlord_id)}</p> : <p>Independent</p>}
            {game.vassals.filter((bond) => bond.overlord_id === country.id).map((bond) => (
              <p key={bond.subject_id}>Subject · {rulerOf(bond.subject_id)}</p>
            ))}
            {local === country.id && subjectBond && (
              <button type="button" disabled={pending || game.wars?.some((war) => war.attacker_id === local || war.defender_id === local)}
                onClick={() => void send({ type: 'declare_independence', country_id: local })}>Declare independence</button>
            )}
            {local !== null && local !== country.id && !subjectBond && !game.vassals.some((bond) => bond.subject_id === local) && (
              <button type="button" disabled={pending || game.vassal_offers.some((offer) => offer.overlord_id === local && offer.subject_id === country.id)}
                onClick={() => void send({ type: 'offer_vassalage', country_id: local, toward_id: country.id })}>Offer vassalage</button>
            )}
            {local !== null && subjectBond?.overlord_id === local && (
              <button type="button" disabled={pending || game.wars?.some((war) => [war.attacker_id, war.defender_id, ...war.attacker_supporters, ...war.defender_supporters].includes(local) || [war.attacker_id, war.defender_id, ...war.attacker_supporters, ...war.defender_supporters].includes(country.id))}
                onClick={() => void send({ type: 'release_vassal', country_id: local, subject_id: country.id })}>Release subject</button>
            )}
            {local === country.id && incoming.map((offer) => (
              <div key={offer.overlord_id}>
                <p>{rulerOf(offer.overlord_id)} offers vassalage until {offer.expires_at}</p>
                <button type="button" disabled={pending} onClick={() => void send({ type: 'accept_vassalage', country_id: local, from_id: offer.overlord_id })}>Accept</button>
                <button type="button" disabled={pending} onClick={() => void send({ type: 'reject_vassalage', country_id: local, from_id: offer.overlord_id })}>Reject</button>
              </div>
            ))}
          </section>
          {country.regency && <p className="game-regency">Regency in effect</p>}
          <CharacterPanel title="Ruler" character={country.ruler} date={game.clock.date} />
          <CharacterPanel title="Heir" character={country.heir} date={game.clock.date} />
        </>
      )}
    </section>
  );
}
