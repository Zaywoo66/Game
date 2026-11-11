use std::{
    collections::BTreeMap,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use strategy_app::{load_embedded_scenario, tick_host_with_log, AiDecisionLog};
use strategy_sim::{
    ArmyId, Command, CommandActor, Controller, CountryId, Date, FiscalPolicy, Fixed, GameState,
    PlayerId, ProvinceId, Speed, TimeCommand, WarId,
};
use tauri::{App, AppHandle, Emitter, Manager, State};

const HOST: PlayerId = PlayerId::new(1);
const UPDATE_EVENT: &str = "strategy:update";

pub(crate) struct Session {
    game: GameState,
    sequence: u64,
    visible_countries: BTreeMap<u32, CountryView>,
    visible_owners: BTreeMap<u32, u32>,
    military: MilitaryView,
    ai_log: AiDecisionLog,
}

#[derive(Clone, Serialize)]
struct ClockView {
    date: String,
    tick: u64,
    paused: bool,
    speed: u8,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct CharacterView {
    id: u32,
    name: String,
    dynasty: String,
    age: u32,
    adm: u8,
    dip: u8,
    mil: u8,
    traits: Vec<String>,
    death: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct CountryView {
    id: u32,
    tag: String,
    name: String,
    color: String,
    capital_id: u32,
    capital: String,
    treasury: String,
    income: String,
    expenses: String,
    tax_rate: i64,
    monthly_spending: String,
    bankrupt: bool,
    regency: bool,
    ruler: Option<CharacterView>,
    heir: Option<CharacterView>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct OwnerView {
    province_id: u32,
    country_id: Option<u32>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct ArmyView {
    id: u32,
    country_id: u32,
    province_id: u32,
    soldiers: u32,
    morale: i64,
    to_province_id: Option<u32>,
    progress: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct PeaceView {
    offered_by: u32,
    cede_goal: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct WarView {
    id: u32,
    attacker_id: u32,
    defender_id: u32,
    goal_province_id: u32,
    score: i32,
    occupations: Vec<OwnerView>,
    offer: Option<PeaceView>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct SiegeView {
    province_id: u32,
    war_id: u32,
    country_id: u32,
    progress_days: u32,
    required_days: u32,
}

#[derive(Serialize)]
struct FortView {
    province_id: u32,
    level: u8,
}

#[derive(Default)]
struct MilitaryView {
    armies: BTreeMap<u32, ArmyView>,
    wars: BTreeMap<u32, WarView>,
    sieges: Vec<SiegeView>,
}

#[derive(Serialize)]
pub(crate) struct Snapshot {
    sequence: u64,
    clock: ClockView,
    local_country_id: Option<u32>,
    countries: Vec<CountryView>,
    owners: Vec<OwnerView>,
    max_tax_rate: i64,
    max_monthly_spending: String,
    armies: Vec<ArmyView>,
    wars: Vec<WarView>,
    sieges: Vec<SiegeView>,
    forts: Vec<FortView>,
    min_raise_soldiers: u32,
    max_raise_soldiers: u32,
}

#[derive(Clone, Serialize)]
struct Update {
    sequence: u64,
    clock: ClockView,
    countries: Vec<CountryView>,
    owners: Vec<OwnerView>,
    armies: Vec<ArmyView>,
    removed_armies: Vec<u32>,
    wars: Vec<WarView>,
    removed_wars: Vec<u32>,
    sieges: Option<Vec<SiegeView>>,
}

#[derive(Serialize)]
pub(crate) struct AiLogEntry {
    tick: u64,
    country_id: u32,
    reason: String,
    command: String,
    score: Option<i32>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum UiCommand {
    #[serde(rename = "set_paused")]
    Paused {
        paused: bool,
    },
    #[serde(rename = "set_speed")]
    Speed {
        speed: u8,
    },
    #[serde(rename = "set_fiscal_policy")]
    FiscalPolicy {
        country_id: u32,
        tax_rate: i64,
        monthly_spending: String,
    },
    RaiseArmy {
        country_id: u32,
        province_id: u32,
        soldiers: u32,
    },
    MergeArmies {
        country_id: u32,
        into: u32,
        from: u32,
    },
    SplitArmy {
        country_id: u32,
        army_id: u32,
        soldiers: u32,
    },
    MoveArmy {
        country_id: u32,
        army_id: u32,
        destination: u32,
    },
    RetreatArmy {
        country_id: u32,
        army_id: u32,
        destination: u32,
    },
    DeclareWar {
        country_id: u32,
        defender_id: u32,
        goal_province_id: u32,
    },
    OfferPeace {
        country_id: u32,
        war_id: u32,
        cede_goal: bool,
    },
    AcceptPeace {
        country_id: u32,
        war_id: u32,
    },
}

impl UiCommand {
    fn into_sim(self) -> Result<Command, String> {
        Ok(match self {
            Self::Paused { paused } => Command::Time(TimeCommand::SetPaused(paused)),
            Self::Speed { speed } => Command::Time(TimeCommand::SetSpeed(
                Speed::try_from(speed).map_err(|_| "Speed must be between 1 and 5.")?,
            )),
            Self::FiscalPolicy {
                country_id,
                tax_rate,
                monthly_spending,
            } => Command::SetFiscalPolicy {
                country: CountryId::new(country_id),
                policy: FiscalPolicy {
                    tax_rate: Fixed::from_raw(tax_rate),
                    monthly_spending: Fixed::from_raw(
                        monthly_spending
                            .parse()
                            .map_err(|_| "Spending is outside the allowed numeric range.")?,
                    ),
                },
            },
            Self::RaiseArmy {
                country_id,
                province_id,
                soldiers,
            } => Command::RaiseArmy {
                country: CountryId::new(country_id),
                province: ProvinceId::new(province_id),
                soldiers,
            },
            Self::MergeArmies {
                country_id,
                into,
                from,
            } => Command::MergeArmies {
                country: CountryId::new(country_id),
                into: ArmyId::new(into),
                from: ArmyId::new(from),
            },
            Self::SplitArmy {
                country_id,
                army_id,
                soldiers,
            } => Command::SplitArmy {
                country: CountryId::new(country_id),
                army: ArmyId::new(army_id),
                soldiers,
            },
            Self::MoveArmy {
                country_id,
                army_id,
                destination,
            } => Command::MoveArmy {
                country: CountryId::new(country_id),
                army: ArmyId::new(army_id),
                destination: ProvinceId::new(destination),
            },
            Self::RetreatArmy {
                country_id,
                army_id,
                destination,
            } => Command::RetreatArmy {
                country: CountryId::new(country_id),
                army: ArmyId::new(army_id),
                destination: ProvinceId::new(destination),
            },
            Self::DeclareWar {
                country_id,
                defender_id,
                goal_province_id,
            } => Command::DeclareWar {
                country: CountryId::new(country_id),
                defender: CountryId::new(defender_id),
                goal: ProvinceId::new(goal_province_id),
            },
            Self::OfferPeace {
                country_id,
                war_id,
                cede_goal,
            } => Command::OfferPeace {
                country: CountryId::new(country_id),
                war: WarId::new(war_id),
                cede_goal,
            },
            Self::AcceptPeace { country_id, war_id } => Command::AcceptPeace {
                country: CountryId::new(country_id),
                war: WarId::new(war_id),
            },
        })
    }
}

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let game = load_embedded_scenario().map_err(std::io::Error::other)?;
    let visible_countries = country_views(&game);
    let visible_owners = owner_views(&game);
    let military = military_view(&game);
    app.manage(Mutex::new(Session {
        game,
        sequence: 0,
        visible_countries,
        visible_owners,
        military,
        ai_log: AiDecisionLog::default(),
    }));

    let handle = app.handle().clone();
    thread::Builder::new()
        .name("strategy-host-clock".into())
        .spawn(move || run_clock(handle))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn game_snapshot(state: State<'_, Mutex<Session>>) -> Result<Snapshot, String> {
    let session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    let polity = session
        .game
        .polity()
        .ok_or("The scenario is unavailable.")?;
    let local_country_id = session.game.countries().find_map(|(id, country)| {
        (country.controller() == &Controller::Human(HOST)).then_some(id.get())
    });
    let war = session.game.war_state();
    Ok(Snapshot {
        sequence: session.sequence,
        clock: clock_view(&session.game),
        local_country_id,
        countries: session.visible_countries.values().cloned().collect(),
        owners: session
            .visible_owners
            .iter()
            .map(|(&province_id, &country_id)| OwnerView {
                province_id,
                country_id: Some(country_id),
            })
            .collect(),
        max_tax_rate: polity.rules.max_tax_rate.raw(),
        max_monthly_spending: money(polity.rules.max_monthly_spending),
        armies: session.military.armies.values().cloned().collect(),
        wars: session.military.wars.values().cloned().collect(),
        sieges: session.military.sieges.clone(),
        forts: war
            .into_iter()
            .flat_map(|war| &war.forts)
            .map(|(&province, &level)| FortView {
                province_id: province.get(),
                level,
            })
            .collect(),
        min_raise_soldiers: 1,
        max_raise_soldiers: war.map_or(0, |war| war.rules.max_army_size),
    })
}

#[tauri::command]
pub(crate) fn game_command(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
    command: UiCommand,
) -> Result<(), String> {
    let command = command.into_sim()?;
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    let delta = session
        .game
        .apply(CommandActor::Host(HOST), &command)
        .map_err(|error| format!("Command rejected ({error:?})."))?;
    if !delta.changes().is_empty() {
        publish(&app, &mut session);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn game_ai_log(state: State<'_, Mutex<Session>>) -> Result<Vec<AiLogEntry>, String> {
    let session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    Ok(session
        .ai_log
        .entries()
        .map(|decision| AiLogEntry {
            tick: decision.tick.get(),
            country_id: decision.country.get(),
            reason: decision.reason.label().into(),
            command: format!("{:?}", decision.command),
            score: decision.reason.score(),
        })
        .collect())
}

fn run_clock(app: AppHandle) {
    let state = app.state::<Mutex<Session>>();
    let mut previous = None;
    let mut deadline = Instant::now();
    loop {
        thread::sleep(Duration::from_millis(16));
        let Ok(mut session) = state.lock() else {
            return;
        };
        let time = session.game.time();
        let speed = time.speed();
        let paused = time.is_paused();
        let now = Instant::now();
        let period = Duration::from_millis(1_000 >> (speed.get() - 1));
        if previous != Some((speed, paused)) {
            previous = Some((speed, paused));
            deadline = now + period;
        }
        if paused || now < deadline {
            continue;
        }
        let Session { game, ai_log, .. } = &mut *session;
        match tick_host_with_log(game, ai_log) {
            Ok(_) => publish(&app, &mut session),
            Err(error) => {
                eprintln!("Host clock stopped: {error:?}");
                return;
            }
        }
        deadline = now + period;
    }
}

fn publish(app: &AppHandle, session: &mut Session) {
    let countries = country_views(&session.game);
    let owners = owner_views(&session.game);
    let military = military_view(&session.game);
    session.sequence += 1;
    let update = Update {
        sequence: session.sequence,
        clock: clock_view(&session.game),
        countries: changed_rows(&session.visible_countries, &countries),
        owners: owners
            .iter()
            .filter(|(id, owner)| session.visible_owners.get(id) != Some(*owner))
            .map(|(&province_id, &country_id)| OwnerView {
                province_id,
                country_id: Some(country_id),
            })
            .chain(
                removed_rows(&session.visible_owners, &owners)
                    .into_iter()
                    .map(|province_id| OwnerView {
                        province_id,
                        country_id: None,
                    }),
            )
            .collect(),
        armies: changed_rows(&session.military.armies, &military.armies),
        removed_armies: removed_rows(&session.military.armies, &military.armies),
        wars: changed_rows(&session.military.wars, &military.wars),
        removed_wars: removed_rows(&session.military.wars, &military.wars),
        sieges: (session.military.sieges != military.sieges).then(|| military.sieges.clone()),
    };
    session.visible_countries = countries;
    session.visible_owners = owners;
    session.military = military;
    if let Err(error) = app.emit(UPDATE_EVENT, update) {
        eprintln!("Could not publish the game update: {error}");
    }
}

fn changed_rows<T: Clone + PartialEq>(old: &BTreeMap<u32, T>, new: &BTreeMap<u32, T>) -> Vec<T> {
    new.iter()
        .filter(|(id, value)| old.get(id) != Some(*value))
        .map(|(_, value)| value.clone())
        .collect()
}

fn removed_rows<T>(old: &BTreeMap<u32, T>, new: &BTreeMap<u32, T>) -> Vec<u32> {
    old.keys()
        .copied()
        .filter(|id| !new.contains_key(id))
        .collect()
}

fn owner_views(game: &GameState) -> BTreeMap<u32, u32> {
    game.polity()
        .into_iter()
        .flat_map(|polity| &polity.countries)
        .flat_map(|(id, country)| {
            country
                .provinces
                .iter()
                .map(|province| (province.get(), id.get()))
        })
        .collect()
}

fn military_view(game: &GameState) -> MilitaryView {
    let Some(state) = game.war_state() else {
        return MilitaryView::default();
    };
    MilitaryView {
        armies: state
            .armies
            .iter()
            .map(|(&id, army)| {
                let movement = army.movement.as_ref();
                (
                    id.get(),
                    ArmyView {
                        id: id.get(),
                        country_id: army.country.get(),
                        province_id: army.province.get(),
                        soldiers: army.soldiers,
                        morale: army.morale.raw(),
                        to_province_id: movement
                            .and_then(|movement| movement.route.first())
                            .map(|leg| leg.to.get()),
                        progress: movement.map_or(0, |movement| movement.progress().raw()),
                    },
                )
            })
            .collect(),
        wars: state
            .wars
            .iter()
            .map(|(&id, war)| {
                (
                    id.get(),
                    WarView {
                        id: id.get(),
                        attacker_id: war.attacker.get(),
                        defender_id: war.defender.get(),
                        goal_province_id: war.goal.get(),
                        score: war.score,
                        occupations: war
                            .occupations
                            .iter()
                            .map(|(&province, &country)| OwnerView {
                                province_id: province.get(),
                                country_id: Some(country.get()),
                            })
                            .collect(),
                        offer: war.offer.map(|offer| PeaceView {
                            offered_by: offer.offered_by.get(),
                            cede_goal: offer.cede_goal,
                        }),
                    },
                )
            })
            .collect(),
        sieges: state
            .sieges
            .iter()
            .map(|(&province, siege)| SiegeView {
                province_id: province.get(),
                war_id: siege.war.get(),
                country_id: siege.besieger.get(),
                progress_days: siege.progress_days,
                required_days: state.siege_required(province),
            })
            .collect(),
    }
}

fn country_views(game: &GameState) -> BTreeMap<u32, CountryView> {
    let Some(polity) = game.polity() else {
        return BTreeMap::new();
    };
    polity
        .countries
        .iter()
        .map(|(&id, country)| {
            let character_view = |id: strategy_sim::CharacterId| {
                let character = game.character(id)?;
                Some(CharacterView {
                    id: id.get(),
                    name: character.name.clone(),
                    dynasty: character.dynasty.clone(),
                    age: character.age_years(game.time().date())?,
                    adm: character.stats.adm,
                    dip: character.stats.dip,
                    mil: character.stats.mil,
                    traits: character.traits.iter().cloned().collect(),
                    death: character
                        .death
                        .filter(|date| *date <= game.time().date())
                        .map(format_date),
                })
            };
            let [red, green, blue] = country.color;
            let economy = &country.economy;
            (
                id.get(),
                CountryView {
                    id: id.get(),
                    tag: country.tag.clone(),
                    name: country.name.clone(),
                    color: format!("#{red:02x}{green:02x}{blue:02x}"),
                    capital_id: country.capital.get(),
                    capital: game
                        .province_graph()
                        .get(country.capital)
                        .map_or_else(String::new, |province| province.key.replace('_', " ")),
                    treasury: money(economy.treasury),
                    income: money(
                        economy
                            .last_month
                            .map_or(Fixed::ZERO, |budget| budget.income),
                    ),
                    expenses: money(
                        economy
                            .last_month
                            .map_or(Fixed::ZERO, |budget| budget.expenses),
                    ),
                    tax_rate: economy.policy.tax_rate.raw(),
                    monthly_spending: money(economy.policy.monthly_spending),
                    bankrupt: economy.bankrupt,
                    regency: game.is_in_regency(id).unwrap_or_default(),
                    ruler: country.succession.ruler.and_then(character_view),
                    heir: country.succession.heir.and_then(character_view),
                },
            )
        })
        .collect()
}

fn clock_view(game: &GameState) -> ClockView {
    let time = game.time();
    ClockView {
        date: format_date(time.date()),
        tick: time.tick().get(),
        paused: time.is_paused(),
        speed: time.speed().get(),
    }
}

fn format_date(date: Date) -> String {
    let (year, month, day) = date.components();
    format!("{year:04}-{month:02}-{day:02}")
}

fn money(value: Fixed) -> String {
    value.raw().to_string()
}
