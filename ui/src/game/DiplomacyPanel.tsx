import { GameCommand, GameSnapshot } from './host';

interface DiplomacyPanelProps {
  game: GameSnapshot;
  pending: boolean;
  send(command: GameCommand): Promise<void>;
}

function nameOf(game: GameSnapshot, id: number): string {
  return game.countries.find((country) => country.id === id)?.name ?? `Country ${id}`;
}

function pairMatches(a: number, b: number, x: number, y: number): boolean {
  return (a === x && b === y) || (a === y && b === x);
}

export function DiplomacyPanel({ game, pending, send }: DiplomacyPanelProps) {
  const local = game.local_country_id;
  if (local === null) {
    return <section className="diplomacy-panel" aria-label="Diplomacy">
      <h2>Diplomacy</h2><p>Choose a country in the LAN lobby to issue diplomatic commands.</p>
    </section>;
  }

  const incoming = (game.alliance_offers ?? []).filter((offer) => offer.toward_id === local);
  const outgoing = (game.alliance_offers ?? []).filter((offer) => offer.from_id === local);
  const ownCountry = nameOf(game, local);
  const wars = game.wars ?? [];
  const atWarWith = (other: number) => wars.some((war) => {
    const attackers = [war.attacker_id, ...war.attacker_supporters];
    const defenders = [war.defender_id, ...war.defender_supporters];
    return (attackers.includes(local) && defenders.includes(other))
      || (defenders.includes(local) && attackers.includes(other));
  });
  const opinionToward = (other: number) => (game.opinions ?? []).find(
    (opinion) => opinion.from_id === local && opinion.toward_id === other,
  )?.effective_points ?? 0;

  return <section className="diplomacy-panel" aria-label="Diplomacy">
    <h2>Diplomacy</h2>
    <p>Playing as {ownCountry}. Treaty decisions are validated by the host and shared with both players.</p>

    {incoming.length > 0 && <section aria-label="Incoming alliance proposals">
      <h3>Incoming alliance proposals</h3>
      <ul>{incoming.map((offer) => <li key={`${offer.from_id}:${offer.toward_id}`}>
        <span>{nameOf(game, offer.from_id)} · expires {offer.expires_at}</span>
        <button type="button" disabled={pending} onClick={() => void send({
          type: 'accept_alliance', country_id: local, from_id: offer.from_id,
        })}>Accept</button>
        <button type="button" disabled={pending} onClick={() => void send({
          type: 'reject_alliance', country_id: local, from_id: offer.from_id,
        })}>Reject</button>
      </li>)}</ul>
    </section>}

    {outgoing.length > 0 && <section aria-label="Outgoing alliance proposals">
      <h3>Outgoing proposals</h3>
      <ul>{outgoing.map((offer) => <li key={`${offer.from_id}:${offer.toward_id}`}>
        {nameOf(game, offer.toward_id)} · awaiting response until {offer.expires_at}
      </li>)}</ul>
    </section>}

    <h3>Countries</h3>
    <div className="diplomacy-country-list">
      {game.countries.filter((country) => country.id !== local).map((country) => {
        const allied = (game.alliances ?? []).some((row) => pairMatches(row.a_id, row.b_id, local, country.id));
        const offer = (game.alliance_offers ?? []).find((row) => pairMatches(
          row.from_id, row.toward_id, local, country.id,
        ));
        const guaranteed = game.guarantees.some(
          (row) => row.guarantor_id === local && row.protected_id === country.id,
        );
        const protects = game.guarantees.some(
          (row) => row.guarantor_id === country.id && row.protected_id === local,
        );
        const overlord = game.vassals.find((row) => row.subject_id === local)?.overlord_id;
        const subject = game.vassals.find((row) => row.subject_id === country.id)?.overlord_id === local;
        const vassalOffer = game.vassal_offers.find(
          (row) => row.overlord_id === local && row.subject_id === country.id,
        );
        const relation = [
          allied && 'Ally',
          guaranteed && 'Guaranteed by you',
          protects && 'Guarantees you',
          overlord === country.id && 'Your overlord',
          subject && 'Your subject',
          atWarWith(country.id) && 'At war',
        ].filter(Boolean).join(' · ') || 'No active treaty';
        const pendingWithCountry = Boolean(offer);
        const canOffer = !allied && !pendingWithCountry && !atWarWith(country.id);
        const cannotOfferReason = allied ? 'Already allied'
          : pendingWithCountry ? 'An alliance proposal is already pending'
            : atWarWith(country.id) ? 'Cannot offer an alliance while at war'
              : undefined;
        return <article className="diplomacy-country" key={country.id}>
          <div className="diplomacy-country-heading">
            <span className="diplomacy-swatch" style={{ backgroundColor: country.color }} aria-hidden="true" />
            <strong>{country.name}</strong>
            <span className="diplomacy-opinion" title="Effective opinion of this country">Opinion {opinionToward(country.id)}/100</span>
          </div>
          <p>{relation}</p>
          <div className="diplomacy-actions">
            {allied ? <button type="button" disabled={pending} onClick={() => void send({
              type: 'break_alliance', country_id: local, other_id: country.id,
            })}>Break alliance</button> : (
              <button type="button" disabled={pending || !canOffer}
                title={cannotOfferReason} onClick={() => void send({
                  type: 'offer_alliance', country_id: local, toward_id: country.id,
                })}>{offer?.from_id === local ? 'Proposal sent' : 'Offer alliance'}</button>
            )}
            <button type="button" disabled={pending || guaranteed} title={guaranteed ? 'Already guaranteed' : undefined}
              onClick={() => void send({ type: 'guarantee', country_id: local, protected_id: country.id })}>
              {guaranteed ? 'Guarantee active' : 'Guarantee'}
            </button>
            {guaranteed && <button type="button" disabled={pending} onClick={() => void send({
              type: 'revoke_guarantee', country_id: local, protected_id: country.id,
            })}>Revoke guarantee</button>}
            {!overlord && !game.vassals.some((row) => row.subject_id === local) && !subject && !vassalOffer && (
              <button type="button" disabled={pending || atWarWith(country.id)}
                title={atWarWith(country.id) ? 'Both countries must be at peace' : undefined}
                onClick={() => void send({ type: 'offer_vassalage', country_id: local, toward_id: country.id })}>
                Offer vassalage
              </button>
            )}
            {vassalOffer && <span>Vassalage offer pending · expires {vassalOffer.expires_at}</span>}
          </div>
        </article>;
      })}
    </div>
  </section>;
}
