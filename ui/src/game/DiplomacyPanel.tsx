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
  const marriageOffers = game.marriage_offers ?? [];
  const incomingMarriages = marriageOffers.filter((offer) => offer.toward_country_id === local);
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
  const character = (id: number) => game.countries.flatMap((country) => [country.ruler, country.heir])
    .find((person) => person?.id === id) ?? null;
  const characterName = (id: number) => character(id)?.name ?? `Character ${id}`;
  const characterCountry = (id: number) => game.countries.find(
    (country) => country.ruler?.id === id || country.heir?.id === id,
  );
  const marriagePair = (targetId: number) => {
    const ours = game.countries.find((country) => country.id === local);
    const theirs = game.countries.find((country) => country.id === targetId);
    const candidates = (country: typeof ours) => country ? [country.ruler, country.heir]
      .filter((person) => person !== null && person !== undefined) : [];
    for (const proposer of candidates(ours)) {
      for (const partner of candidates(theirs)) {
        const legal = proposer && partner
          && proposer.sex && partner.sex
          && proposer.sex !== 'unknown' && partner.sex !== 'unknown'
          && proposer.sex !== partner.sex
          && proposer.age >= (game.marriage_adulthood_years ?? 18)
          && partner.age >= (game.marriage_adulthood_years ?? 18)
          && proposer.death === null && partner.death === null
          && (proposer.spouse_id === null || proposer.spouse_id === undefined)
          && (partner.spouse_id === null || partner.spouse_id === undefined);
        if (legal) return { proposer, partner };
      }
    }
    return null;
  };

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

    {incomingMarriages.length > 0 && <section aria-label="Incoming royal-marriage proposals">
      <h3>Incoming royal-marriage proposals</h3>
      <ul>{incomingMarriages.map((offer) => <li key={`${offer.from_character_id}:${offer.toward_character_id}`}>
        <span>{characterName(offer.from_character_id)} of {nameOf(game, offer.from_country_id)} proposes to
          {' '}{characterName(offer.toward_character_id)} · expires {offer.expires_at}</span>
        <button type="button" disabled={pending} onClick={() => void send({
          type: 'accept_royal_marriage', country_id: local, from_id: offer.from_country_id,
          character_id: offer.toward_character_id, partner_id: offer.from_character_id,
        })}>Accept</button>
        <button type="button" disabled={pending} onClick={() => void send({
          type: 'reject_royal_marriage', country_id: local, from_id: offer.from_country_id,
          character_id: offer.toward_character_id, partner_id: offer.from_character_id,
        })}>Reject</button>
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
        const pair = marriagePair(country.id);
        const marriagePending = marriageOffers.some((offer) => pairMatches(
          offer.from_country_id, offer.toward_country_id, local, country.id,
        ));
        const marriagePartner = [game.countries.find((row) => row.id === local)?.ruler,
          game.countries.find((row) => row.id === local)?.heir]
          .filter((person) => person?.spouse_id !== null && person?.spouse_id !== undefined)
          .map((person) => characterCountry(person!.spouse_id!))
          .find((other) => other?.id === country.id);
        const relation = [
          allied && 'Ally',
          guaranteed && 'Guaranteed by you',
          protects && 'Guarantees you',
          overlord === country.id && 'Your overlord',
          subject && 'Your subject',
          marriagePartner && 'Royal marriage',
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
            <button type="button" disabled={pending || !pair || marriagePending}
              title={marriagePending ? 'A royal-marriage proposal is already pending' : !pair
                ? 'Requires a living, unmarried adult ruler or heir of the opposite sex' : undefined}
              onClick={() => pair && void send({
                type: 'offer_royal_marriage', country_id: local, character_id: pair.proposer.id,
                toward_id: country.id, partner_id: pair.partner.id,
              })}>{marriagePending ? 'Marriage proposal pending' : 'Offer royal marriage'}</button>
          </div>
        </article>;
      })}
    </div>
  </section>;
}
