import { FormEvent, useState } from 'react';
import { CountryView, GameSnapshot } from './host';
import { AiPreset, LanController, LanStatus } from './lan';

interface Props {
  game: GameSnapshot | null;
  gameError: string;
  lan: LanController;
  onSinglePlayer(): void;
}

type CountryOption = Pick<CountryView, 'id' | 'tag' | 'name' | 'color'>;

type MenuPage = 'home' | 'network' | 'create' | 'join';

function RoomLobby({ game, lan, status }: { game: GameSnapshot | null; lan: LanController; status: LanStatus }) {
  const host = status.mode === 'host';
  const ownCountryId = host ? status.host_country_id : status.guest_country_id;
  const otherCountryId = host ? status.guest_country_id : status.host_country_id;
  const ownCountry = game?.countries.find((country) => country.id === ownCountryId);
  const otherCountry = game?.countries.find((country) => country.id === otherCountryId);
  const countries = game?.countries ?? [];
  const selectable = countries.filter((country) => country.id !== otherCountryId);
  const canStart = host && status.guest_connected && Boolean(status.host_country_id && status.guest_country_id);

  return (
    <section className="room-screen" aria-label="Room country selection">
      <header className="room-screen-heading">
        <div>
          <p className="menu-eyebrow">Private LAN room</p>
          <h1>{status.room ?? 'Campaign room'}</h1>
          <p className="room-address">{status.address ?? 'Preparing room address…'}</p>
        </div>
        <span className={'room-connection' + (status.guest_connected ? ' is-connected' : '')} role="status">
          {status.guest_connected ? 'Two players connected' : 'Waiting for the other player'}
        </span>
      </header>

      {!game ? <p className="room-waiting" role="status">Loading the country roster…</p> : (
        <div className="room-seats">
          <article className="room-seat room-seat--local">
            <span className="room-seat-number">01 · YOU</span>
            <h2>{host ? 'Room host' : 'Guest player'}</h2>
            {host ? <label className="menu-field">Choose your country
              <select value={ownCountryId ?? ''} disabled={lan.busy || !countries.length}
                onChange={(event) => void lan.claimCountry(Number(event.target.value))}>
                <option value="">Select a country</option>
                {selectable.map((country) => <option key={country.id} value={country.id}>{country.name}</option>)}
              </select>
            </label> : <>
              <p className="room-selected-country">{ownCountry?.name ?? 'Choose your country'}</p>
              <label className="menu-field">Choose your country
                <select value={ownCountryId ?? ''} disabled={lan.busy || !countries.length}
                  onChange={(event) => void lan.claimCountry(Number(event.target.value))}>
                  <option value="">Select a country</option>
                  {selectable.map((country) => <option key={country.id} value={country.id}>{country.name}</option>)}
                </select>
              </label>
            </>}
            {host && ownCountry && <p className="room-selected-country">{ownCountry.name}</p>}
          </article>

          <div className="room-versus" aria-hidden="true">VS</div>

          <article className="room-seat">
            <span className="room-seat-number">02 · {host ? 'GUEST' : 'HOST'}</span>
            <h2>{host ? 'Guest player' : 'Room host'}</h2>
            <p className="room-selected-country">{otherCountry?.name ?? 'Choosing a country…'}</p>
            <p className="room-seat-note">
              {status.guest_connected ? 'Country selection is reserved by that player.' : 'The country will appear when the guest joins.'}
            </p>
          </article>
        </div>
      )}

      {lan.error && <p className="menu-error" role="alert">{lan.error}</p>}
      {status.detail && <p className="menu-hint" role="status">{status.detail}</p>}
      <footer className="room-screen-footer">
        <button className="menu-button menu-button--quiet" type="button" disabled={lan.busy}
          onClick={() => void lan.leave()}>Leave room</button>
        {host ? <button className="menu-button menu-button--primary" type="button"
          disabled={!canStart || lan.busy} onClick={() => void lan.startCampaign()}>
          {lan.busy ? 'Preparing campaign…' : 'Start campaign'}
        </button> : <p className="menu-hint">When both players have chosen, the host can start the campaign.</p>}
      </footer>
    </section>
  );
}

function CreateRoom({ lan, game, gameError, onBack }: {
  lan: LanController; game: GameSnapshot | null; gameError: string; onBack(): void;
}) {
  const [room, setRoom] = useState('Strategy room');
  const [preset, setPreset] = useState<AiPreset>('balanced');
  const [grace, setGrace] = useState('2');
  const [port, setPort] = useState('47620');
  const [countryQuery, setCountryQuery] = useState('');
  const [countryId, setCountryId] = useState('');
  const countries: CountryOption[] = game?.countries ?? [];
  const rosterError = gameError || (game && countries.length === 0
    ? 'The loaded scenario contains no playable countries.' : '');
  const visibleCountries = countries
    .filter((country) => `${country.name} ${country.tag}`.toLocaleLowerCase().includes(countryQuery.trim().toLocaleLowerCase()))
    .sort((left, right) => left.name.localeCompare(right.name));

  function submit(event: FormEvent) {
    event.preventDefault();
    const graceMinutes = Number(grace);
    const portNumber = Number(port);
    if (!room.trim() || !countryId || !Number.isInteger(graceMinutes) || graceMinutes < 0 || graceMinutes > 60
      || !Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65_535) return;
    void lan.start({
      room: room.trim(), host_country_id: Number(countryId), guest_country_id: 0,
      ai_preset: preset, grace_minutes: graceMinutes, port: portNumber,
    });
  }

  return (
    <section className="menu-card" aria-label="Create LAN room">
      <button className="menu-back" type="button" onClick={onBack}>← Network game</button>
      <p className="menu-eyebrow">Host a new room</p>
      <h1>Create room</h1>
      <p className="menu-copy">Choose your country. Your guest will choose theirs after joining.</p>
      <form className="menu-form" onSubmit={submit}>
        <label className="menu-field">Room name
          <input value={room} maxLength={48} onChange={(event) => setRoom(event.target.value)} required />
        </label>
        <label className="menu-field">Filter countries
          <input value={countryQuery} maxLength={48} onChange={(event) => setCountryQuery(event.target.value)}
            placeholder="Search by country name or tag" />
        </label>
        <label className="menu-field">Your country
          <select className="menu-country-list" size={Math.min(5, Math.max(3, visibleCountries.length + 1))}
            aria-label="Your country" value={countryId} onChange={(event) => setCountryId(event.target.value)}
            disabled={!countries.length || Boolean(rosterError)} required>
            <option value="" disabled>
              {rosterError ? 'Country data unavailable' : game ? 'Select your country' : 'Loading countries…'}
            </option>
            {visibleCountries.map((country) => <option key={country.id} value={country.id}>{country.name}</option>)}
          </select>
        </label>
        {rosterError && <p className="menu-error" role="alert">
          Could not load the country list: {rosterError}. Restart the game after updating it.
        </p>}
        <div className="menu-form-row">
          <label className="menu-field">AI style
            <select value={preset} onChange={(event) => setPreset(event.target.value as AiPreset)}>
              <option value="balanced">Balanced</option>
              <option value="cautious">Cautious</option>
              <option value="aggressive">Aggressive</option>
            </select>
          </label>
          <label className="menu-field">Reconnect grace · minutes
            <input type="number" min={0} max={60} step={1} value={grace}
              onChange={(event) => setGrace(event.target.value)} required />
          </label>
        </div>
        <label className="menu-field">Room port
          <input type="number" min={1} max={65_535} step={1} value={port}
            onChange={(event) => setPort(event.target.value)} required />
        </label>
        {lan.error && <p className="menu-error" role="alert">{lan.error}</p>}
        <button className="menu-button menu-button--primary" type="submit" disabled={lan.busy || !countryId}>
          {lan.busy ? 'Opening room…' : 'Open room'}
        </button>
      </form>
    </section>
  );
}

function JoinRoom({ lan, onBack }: { lan: LanController; onBack(): void }) {
  const [name, setName] = useState('Player 2');
  const [address, setAddress] = useState('');
  const join = (target: string) => {
    if (target.trim() && name.trim()) void lan.join(target.trim(), name.trim());
  };

  return (
    <section className="menu-card" aria-label="Join LAN room">
      <button className="menu-back" type="button" onClick={onBack}>← Network game</button>
      <p className="menu-eyebrow">Join an existing room</p>
      <h1>Join room</h1>
      <label className="menu-field">Player name
        <input value={name} maxLength={48} onChange={(event) => setName(event.target.value)} />
      </label>
      <div className="room-finder">
        <div><h2>Nearby rooms</h2><p>Find a host on your local network.</p></div>
        <button className="menu-button menu-button--secondary" type="button"
          disabled={lan.busy || lan.scanning} onClick={() => void lan.discover()}>
          {lan.scanning ? 'Searching…' : 'Search LAN'}
        </button>
      </div>
      {lan.rooms.length > 0 && <ul className="menu-room-list">
        {lan.rooms.map((room) => <li key={room.address}>
          <div><strong>{room.name}</strong><span>{room.address} · {room.player_count}/2</span></div>
          <button type="button" disabled={lan.busy || !name.trim()} onClick={() => join(room.address)}>Join</button>
        </li>)}
      </ul>}
      {lan.scanned && !lan.scanning && lan.rooms.length === 0 && <p className="menu-hint">No rooms found. You can enter the host address below.</p>}
      <form className="menu-form menu-manual-join" onSubmit={(event) => { event.preventDefault(); join(address); }}>
        <label className="menu-field">Host address
          <input value={address} onChange={(event) => setAddress(event.target.value)}
            placeholder="192.168.1.10:47620" required />
        </label>
        <button className="menu-button menu-button--primary" type="submit" disabled={lan.busy || !name.trim()}>
          {lan.busy ? 'Joining…' : 'Join by address'}
        </button>
      </form>
      {lan.error && <p className="menu-error" role="alert">{lan.error}</p>}
    </section>
  );
}

export function MainMenu({ game, gameError, lan, onSinglePlayer }: Props) {
  const [page, setPage] = useState<MenuPage>('home');
  const room = lan.status && lan.status.mode !== 'solo' && !lan.status.started ? lan.status : null;
  if (room) return <main className="main-menu-page"><RoomLobby game={game} lan={lan} status={room} /></main>;

  return (
    <main className="main-menu-page">
      <div className="menu-atlas-mark" aria-hidden="true"><span>✳</span></div>
      <section className="main-menu-layout">
        <div className="main-menu-brand">
          <p className="menu-eyebrow">A grand strategy campaign</p>
          <h1>Strategy</h1>
          <p className="menu-brand-copy">Shape the continent. Build your state. Decide the course of history.</p>
          <div className="menu-campaign-tag"><span /> 2022 · Europe, North Africa &amp; Middle East</div>
        </div>

        {page === 'home' && <nav className="menu-card menu-navigation" aria-label="Main menu">
          <p className="menu-eyebrow">Main menu</p>
          <h2>Choose how to play</h2>
          <button className="menu-button menu-button--primary" type="button" onClick={onSinglePlayer}>
            Single player <span aria-hidden="true">→</span>
          </button>
          <button className="menu-button menu-button--secondary" type="button" onClick={() => setPage('network')}>
            Network game <span aria-hidden="true">↗</span>
          </button>
          <p className="menu-footnote">One campaign start · 1 January 2022</p>
        </nav>}

        {page === 'network' && <section className="menu-card menu-navigation" aria-label="Network game options">
          <button className="menu-back" type="button" onClick={() => setPage('home')}>← Main menu</button>
          <p className="menu-eyebrow">Two players · local network</p>
          <h2>Network game</h2>
          <button className="menu-button menu-button--primary" type="button" onClick={() => setPage('create')}>
            Create a room <span aria-hidden="true">＋</span>
          </button>
          <button className="menu-button menu-button--secondary" type="button" onClick={() => setPage('join')}>
            Join a room <span aria-hidden="true">⇢</span>
          </button>
          <p className="menu-footnote">The host starts the campaign after both players choose a country.</p>
        </section>}

        {page === 'create' && <CreateRoom lan={lan} game={game} gameError={gameError}
          onBack={() => setPage('network')} />}
        {page === 'join' && <JoinRoom lan={lan} onBack={() => setPage('network')} />}
      </section>
      <footer className="main-menu-footer"><span>STRATEGY</span><span>LOCAL CAMPAIGN · 2022</span></footer>
    </main>
  );
}
