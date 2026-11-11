import { FormEvent, useEffect, useRef, useState } from 'react';
import { GameSnapshot } from './host';
import { AiPreset, LanController, lanStatusText } from './lan';

interface Props {
  game: GameSnapshot | null;
  lan: LanController;
  onClose(): void;
}

function NewRoom({ game, lan }: Pick<Props, 'game' | 'lan'>) {
  const [room, setRoom] = useState('Strategy LAN');
  const [hostPick, setHostPick] = useState<number>();
  const [guestPick, setGuestPick] = useState<number>();
  const [preset, setPreset] = useState<AiPreset>('balanced');
  const [grace, setGrace] = useState('2');
  const [port, setPort] = useState('47620');
  const countries = game?.countries ?? [];
  const hostCountry = countries.find(({ id }) => id === hostPick)
    ?? countries.find(({ id }) => id === game?.local_country_id) ?? countries[0];
  const guestCountries = countries.filter(({ id }) => id !== hostCountry?.id);
  const guestCountry = guestCountries.find(({ id }) => id === guestPick) ?? guestCountries[0];

  function start(event: FormEvent) {
    event.preventDefault();
    const portNumber = Number(port);
    const graceMinutes = Number(grace);
    if (!hostCountry || !guestCountry || !room.trim() || !Number.isInteger(portNumber)
      || portNumber < 1 || portNumber > 65_535 || !Number.isInteger(graceMinutes)
      || graceMinutes < 0 || graceMinutes > 60) return;
    void lan.start({
      room: room.trim(),
      host_country_id: hostCountry.id,
      guest_country_id: guestCountry.id,
      ai_preset: preset,
      grace_minutes: graceMinutes,
      port: portNumber,
    });
  }

  return (
    <form aria-label="Host a LAN game" onSubmit={start} className="lan-form">
      <h3>Host a game</h3>
      <label>Room name
        <input value={room} onChange={(event) => setRoom(event.target.value)} required />
      </label>
      <div className="lan-fields">
        <label>Host country
          <select value={hostCountry?.id ?? ''} onChange={(event) => setHostPick(Number(event.target.value))}>
            {countries.map(({ id, name }) => <option key={id} value={id}>{name}</option>)}
          </select>
        </label>
        <label>Guest country
          <select value={guestCountry?.id ?? ''} onChange={(event) => setGuestPick(Number(event.target.value))}>
            {guestCountries.map(({ id, name }) => <option key={id} value={id}>{name}</option>)}
          </select>
        </label>
        <label>AI preset
          <select value={preset} onChange={(event) => setPreset(event.target.value as AiPreset)}>
            <option value="balanced">Balanced</option>
            <option value="cautious">Cautious</option>
            <option value="aggressive">Aggressive</option>
          </select>
        </label>
        <label>Disconnect grace (minutes)
          <input type="number" min={0} max={60} step={1} value={grace}
            onChange={(event) => setGrace(event.target.value)} required />
        </label>
      </div>
      <label>Port
        <input type="number" min={1} max={65_535} step={1} value={port}
          onChange={(event) => setPort(event.target.value)} required />
      </label>
      <button type="submit" disabled={lan.busy || !guestCountry}>Host LAN game</button>
    </form>
  );
}

function JoinRoom({ lan }: Pick<Props, 'lan'>) {
  const [address, setAddress] = useState('');
  const [name, setName] = useState('Guest');

  function join(target: string) {
    if (target.trim() && name.trim()) void lan.join(target.trim(), name.trim());
  }

  return (
    <section className="lan-form" aria-label="Join a LAN game">
      <h3>Join a game</h3>
      <label>Player name
        <input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      <button type="button" disabled={lan.busy || lan.scanning} onClick={() => void lan.discover()}>
        {lan.scanning ? 'Scanning…' : 'Find LAN rooms'}
      </button>
      {lan.rooms.length > 0 && (
        <ul className="lan-rooms">
          {lan.rooms.map((room) => (
            <li key={room.address}>
              <div><strong>{room.name}</strong><p>{room.address} · {room.player_count}/2 players · v{room.version}</p></div>
              <button type="button" disabled={lan.busy || !name.trim()} onClick={() => join(room.address)}>
                Join {room.name}
              </button>
            </li>
          ))}
        </ul>
      )}
      {lan.scanned && !lan.scanning && lan.rooms.length === 0 && <p>No LAN rooms found</p>}
      <form className="lan-form lan-manual" aria-label="Manual LAN address" onSubmit={(event) => {
        event.preventDefault();
        join(address);
      }}>
        <label>IP address and port
          <input value={address} onChange={(event) => setAddress(event.target.value)}
            placeholder="192.168.1.10:47620" required />
        </label>
        <button type="submit" disabled={lan.busy || !name.trim()}>Join by address</button>
      </form>
    </section>
  );
}

function ActiveRoom({ game, lan }: Pick<Props, 'game' | 'lan'>) {
  const status = lan.status;
  if (!status) return null;
  const guest = game?.countries.find(({ id }) => id === status.guest_country_id);
  const localPort = status.mode === 'host' && status.address?.match(/^(?:0\.0\.0\.0|127\.0\.0\.1):(\d+)$/)?.[1];
  return (
    <section className="lan-active" aria-label="Active LAN room">
      <h3>{status.room ?? 'LAN game'}</h3>
      <p role="status">{status.mode === 'host' ? 'Host' : 'Guest'} · {lanStatusText(status)}</p>
      {status.address && <p>Address · {status.address}</p>}
      {localPort && <p>On the second PC, enter this host’s LAN IP with port {localPort}.</p>}
      {guest && <p>Guest country · {guest.name}</p>}
      {status.detail && <p>{status.detail}</p>}
      {status.temporary_ai && <p>Temporary AI controls the guest country</p>}
      <dl className="lan-checks">
        <div><dt>Verified checksums</dt><dd>{status.verified_checksums}</dd></div>
        <div><dt>Checksum mismatches</dt><dd>{status.checksum_mismatches}</dd></div>
      </dl>
      <div className="lan-actions">
        {status.mode === 'host' && status.can_takeover && !status.temporary_ai && (
          <button type="button" disabled={lan.busy} onClick={() => void lan.takeover()}>
            Give guest country to AI
          </button>
        )}
        <button type="button" disabled={lan.busy} onClick={() => void lan.leave()}>
          {status.mode === 'host' ? 'Close room' : 'Leave room'}
        </button>
      </div>
    </section>
  );
}

export function LanLobby({ game, lan, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (dialog.current && !dialog.current.open) dialog.current.showModal();
  }, []);

  return (
    <dialog className="lan-dialog" ref={dialog} aria-labelledby="lan-title" onCancel={(event) => {
      event.preventDefault();
      onClose();
    }}>
      <header>
        <h2 id="lan-title">LAN multiplayer</h2>
        <button type="button" onClick={onClose} aria-label="Close LAN lobby">Close</button>
      </header>
      {lan.error && <p className="lan-error" role="alert">{lan.error}</p>}
      {lan.pendingAction && <p role="status">{{
        lan_start: 'Starting room…',
        lan_join: 'Joining room…',
        lan_leave: 'Leaving room…',
        lan_takeover: 'Updating guest control…',
      }[lan.pendingAction]}</p>}
      {lan.status?.mode === 'solo' ? (
        <div className="lan-layout"><NewRoom game={game} lan={lan} /><JoinRoom lan={lan} /></div>
      ) : <ActiveRoom game={game} lan={lan} />}
    </dialog>
  );
}
