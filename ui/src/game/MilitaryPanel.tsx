import { FormEvent, useState } from 'react';
import { AiLogEntry, ArmyView, FleetView, GameCommand, GameSnapshot, WarView } from './host';

export type ArmyOrder = 'move_army' | 'retreat_army';

interface MilitaryProps {
  game: GameSnapshot;
  pending: boolean;
  provinceId: number | null;
  provinceIsLand: boolean;
  army: ArmyView | undefined;
  order: ArmyOrder | null;
  selectArmy(id: number): void;
  chooseOrder(order: ArmyOrder): void;
  cancelOrder(): void;
  provinceName(id: number): string;
  send(command: GameCommand): Promise<void>;
  readAiLog(): Promise<AiLogEntry[]>;
  canReadAiLog?: boolean;
}

function ArmyOrders({
  army, armies, pending, chooseOrder, send,
}: Pick<MilitaryProps, 'pending' | 'chooseOrder' | 'send'> & { army: ArmyView; armies: ArmyView[] }) {
  const [split, setSplit] = useState('1');
  const [mergeFrom, setMergeFrom] = useState('');
  const country_id = army.country_id;
  const mergeCandidates = armies.filter((other) => (
    other.id !== army.id && other.country_id === country_id && other.province_id === army.province_id
    && other.to_province_id === null && army.to_province_id === null
  ));

  function splitArmy(event: FormEvent) {
    event.preventDefault();
    const soldiers = Number(split);
    if (Number.isInteger(soldiers) && soldiers > 0 && soldiers < army.soldiers) {
      void send({ type: 'split_army', country_id, army_id: army.id, soldiers });
    }
  }

  function mergeArmies(event: FormEvent) {
    event.preventDefault();
    const from = Number(mergeFrom);
    if (mergeCandidates.some((candidate) => candidate.id === from)) {
      void send({ type: 'merge_armies', country_id, into: army.id, from });
    }
  }

  return (
    <>
      <div className="military-actions">
        <button type="button" disabled={pending} onClick={() => chooseOrder('move_army')}>Choose move destination</button>
        <button type="button" disabled={pending} onClick={() => chooseOrder('retreat_army')}>Choose retreat destination</button>
      </div>
      <form className="military-form" aria-label="Split army" onSubmit={splitArmy}>
        <label>Split soldiers
          <input type="number" min={1} max={army.soldiers - 1} step={1} value={split}
            onChange={(event) => setSplit(event.target.value)} disabled={pending || army.soldiers < 2} required />
        </label>
        <button type="submit" disabled={pending || army.soldiers < 2}>Split</button>
      </form>
      {mergeCandidates.length > 0 && (
        <form className="military-form" aria-label="Merge armies" onSubmit={mergeArmies}>
          <label>Merge from
            <select value={mergeFrom} onChange={(event) => setMergeFrom(event.target.value)} disabled={pending} required>
              <option value="">Choose army</option>
              {mergeCandidates.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>Army {candidate.id}</option>
              ))}
            </select>
          </label>
          <button type="submit" disabled={pending || !mergeFrom}>Merge</button>
        </form>
      )}
    </>
  );
}

function RaiseArmy({
  country, capital, min, max, pending, send,
}: Pick<MilitaryProps, 'pending' | 'send'> & { country: number; capital: number; min: number; max: number }) {
  const [quantity, setQuantity] = useState(String(min));
  function submit(event: FormEvent) {
    event.preventDefault();
    const soldiers = Number(quantity);
    if (Number.isInteger(soldiers) && soldiers >= min && soldiers <= max) {
      void send({ type: 'raise_army', country_id: country, province_id: capital, soldiers });
    }
  }
  return (
    <form className="military-form" aria-label="Raise army" onSubmit={submit}>
      <label>Soldiers at capital
        <input type="number" min={min} max={max} step={1} value={quantity}
          onChange={(event) => setQuantity(event.target.value)} disabled={pending} required />
      </label>
      <button type="submit" disabled={pending}>Raise army</button>
    </form>
  );
}

function ActiveWar({
  war, local, pending, countryName, provinceName, send,
}: Pick<MilitaryProps, 'pending' | 'provinceName' | 'send'> & {
  war: WarView;
  local: number | null;
  countryName(id: number): string;
}) {
  const involved = local !== null && (local === war.attacker_id || local === war.defender_id);
  function offer(cede_goal: boolean) {
    if (local !== null) void send({ type: 'offer_peace', country_id: local, war_id: war.id, cede_goal });
  }
  return (
    <section className="active-war" aria-label={'War ' + war.id}>
      <h4>{countryName(war.attacker_id)} vs {countryName(war.defender_id)}</h4>
      <p>Goal · {provinceName(war.goal_province_id)}</p>
      <p>Attacker war score · {war.score}</p>
      {war.occupations.length > 0 && <p>Occupied provinces · {war.occupations.length}</p>}
      {war.offer && (
        <p>{countryName(war.offer.offered_by)} offered {war.offer.cede_goal ? 'cession of the war goal' : 'white peace'}.</p>
      )}
      {involved && (
        <div className="military-actions">
          <button type="button" disabled={pending} onClick={() => offer(false)}>Offer white peace</button>
          <button type="button" disabled={pending} onClick={() => offer(true)}>
            {local === war.attacker_id ? 'Demand war goal' : 'Offer war goal'}
          </button>
          {war.offer && war.offer.offered_by !== local && local !== null && (
            <button type="button" disabled={pending}
              onClick={() => void send({ type: 'accept_peace', country_id: local, war_id: war.id })}>
              Accept peace
            </button>
          )}
        </div>
      )}
    </section>
  );
}

function AiDebug({
  readAiLog, countryName,
}: Pick<MilitaryProps, 'readAiLog'> & { countryName(id: number): string }) {
  const [entries, setEntries] = useState<AiLogEntry[] | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  async function refresh() {
    if (pending) return;
    setPending(true);
    setError('');
    try {
      setEntries(await readAiLog());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setPending(false);
    }
  }
  return (
    <details className="military-debug" onToggle={(event) => {
      if (event.currentTarget.open && entries === null && !pending) void refresh();
    }}>
      <summary>AI decisions</summary>
      <button type="button" disabled={pending} onClick={() => void refresh()}>Refresh decisions</button>
      {pending && <p role="status">Reading decisions…</p>}
      {error && <p className="game-error" role="alert">{error}</p>}
      {entries && (entries.length ? (
        <ul>
          {entries.slice().reverse().map((entry, index) => (
            <li key={entry.tick + ':' + index}>
              <p>{countryName(entry.country_id)} · Tick {entry.tick}</p>
              <p>{entry.reason} · {entry.command}</p>
              {entry.score !== null && <p>Score · {entry.score}</p>}
            </li>
          ))}
        </ul>
      ) : <p>No AI decisions recorded yet</p>)}
    </details>
  );
}

export function MilitaryPanel(props: MilitaryProps) {
  const {
    game, pending, provinceId, provinceIsLand, army, order, selectArmy, chooseOrder, cancelOrder,
    provinceName, send, readAiLog, canReadAiLog = true,
  } = props;
  const localId = game.local_country_id;
  const local = game.countries.find((country) => country.id === localId);
  const armies = game.armies ?? [];
  const fleets = game.fleets ?? [];
  const wars = game.wars ?? [];
  const countryName = (id: number) => game.countries.find((country) => country.id === id)?.name ?? 'Country ' + id;
  const owner = game.owners.find((row) => row.province_id === provinceId)?.country_id;
  const enemy = owner === undefined || owner === localId ? undefined : owner;
  const fort = game.forts?.find((row) => row.province_id === provinceId);
  const sieges = game.sieges?.filter((row) => row.province_id === provinceId) ?? [];
  const existingWar = wars.some((war) => (
    (war.attacker_id === localId && war.defender_id === enemy)
    || (war.defender_id === localId && war.attacker_id === enemy)
  ));
  const controlsCapital = local
    && game.owners.some((row) => row.province_id === local.capital_id && row.country_id === local.id)
    && !wars.some((war) => war.occupations.some((row) => (
      row.province_id === local.capital_id && row.country_id !== local.id
    )));

  return (
    <section className="military-panel" aria-label="Military">
      <h3>Armies</h3>
      <label className="army-picker">Army
        <select value={army?.id ?? ''} onChange={(event) => selectArmy(Number(event.target.value))}>
          <option value="">Choose an army</option>
          {armies.map((row) => (
            <option key={row.id} value={row.id}>{countryName(row.country_id)} · Army {row.id}</option>
          ))}
        </select>
      </label>
      {army && (
        <section aria-label="Selected army">
          <h4>{countryName(army.country_id)} · Army {army.id}</h4>
          <p>{army.soldiers.toLocaleString()} soldiers · Morale {army.morale / 10}%</p>
          <p>{provinceName(army.province_id)}</p>
          {army.embarked_on && <p>Aboard fleet {army.embarked_on}</p>}
          {army.to_province_id !== null && (
            <p>Moving to {provinceName(army.to_province_id)} · {army.progress / 10}%</p>
          )}
          {army.country_id === localId && !army.embarked_on && (
            <ArmyOrders key={army.id} army={army} armies={armies} pending={pending} chooseOrder={chooseOrder} send={send} />
          )}
          {order && army.country_id === localId && (
            <div className="military-target">
              <p role="status">Click a land province to {order === 'move_army' ? 'move' : 'retreat'}.</p>
              <button type="button" onClick={cancelOrder}>Cancel destination</button>
            </div>
          )}
        </section>
      )}
      {local && game.min_raise_soldiers !== undefined && game.max_raise_soldiers !== undefined && (
        controlsCapital ? (
          <RaiseArmy key={local.id} country={local.id} capital={local.capital_id}
            min={game.min_raise_soldiers} max={game.max_raise_soldiers} pending={pending} send={send} />
        ) : <p>Capital is not controlled. Raising is unavailable.</p>
      )}
      <h3>Fleets</h3>
      {fleets.length ? fleets.map((fleet: FleetView) => (
        <section key={fleet.id} aria-label={`Fleet ${fleet.id}`}>
          <h4>{countryName(fleet.country_id)} · Fleet {fleet.id}</h4>
          <p>{fleet.ships} ships · Capacity {(fleet.ships * 1000).toLocaleString()} soldiers</p>
          <p>{provinceName(fleet.sea_province_id)}</p>
          {fleet.cargo_army_id && <p>Carrying army {fleet.cargo_army_id}</p>}
          {fleet.to_sea_province_id && <p>Sailing to {provinceName(fleet.to_sea_province_id)} · {fleet.progress / 10}%</p>}
          {fleet.country_id === localId && localId !== null && (
            <div className="military-actions">
              <button type="button" disabled={pending || provinceId === null || provinceIsLand || fleet.to_sea_province_id !== null}
                onClick={() => provinceId !== null && void send({ type: 'move_fleet', country_id: localId, fleet_id: fleet.id, destination: provinceId })}>
                Sail to selected sea zone
              </button>
              <button type="button" disabled={pending || !army || army.country_id !== localId || Boolean(fleet.cargo_army_id) || fleet.to_sea_province_id !== null}
                onClick={() => army && void send({ type: 'embark_army', country_id: localId, army_id: army.id, fleet_id: fleet.id })}>
                Embark selected army
              </button>
              <button type="button" disabled={pending || !fleet.cargo_army_id || provinceId === null || !provinceIsLand || fleet.to_sea_province_id !== null}
                onClick={() => provinceId !== null && void send({ type: 'disembark_army', country_id: localId, fleet_id: fleet.id, destination: provinceId })}>
                Land at selected port
              </button>
            </div>
          )}
        </section>
      )) : <p>No fleets</p>}
      <h3>Wars</h3>
      {fort && <p>Fort level {fort.level}</p>}
      {sieges.map((siege) => (
        <p key={siege.war_id}>{countryName(siege.country_id)} siege · {siege.progress_days}/{siege.required_days} days</p>
      ))}
      {local && enemy !== undefined && provinceId !== null && provinceIsLand && !existingWar && (
        <button type="button" disabled={pending}
          onClick={() => void send({
            type: 'declare_war', country_id: local.id, defender_id: enemy, goal_province_id: provinceId,
          })}>
          Declare war for {provinceName(provinceId)}
        </button>
      )}
      {wars.length ? wars.map((war) => (
        <ActiveWar key={war.id} war={war} local={localId} pending={pending}
          countryName={countryName} provinceName={provinceName} send={send} />
      )) : <p>No active wars</p>}
      {canReadAiLog ? <AiDebug readAiLog={readAiLog} countryName={countryName} />
        : <p className="military-debug">Decision log is available on the host</p>}
    </section>
  );
}
