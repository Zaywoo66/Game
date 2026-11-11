use std::{
    collections::BTreeMap,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use strategy_app::{load_embedded_scenario, tick_host_with_log, AiDecisionLog};
use strategy_sim::{
    Command, CommandActor, Controller, CountryId, Date, FiscalPolicy, Fixed, GameState, PlayerId,
    Speed, TimeCommand,
};
use tauri::{App, AppHandle, Emitter, Manager, State};

const HOST: PlayerId = PlayerId::new(1);
const UPDATE_EVENT: &str = "strategy:update";

pub(crate) struct Session {
    game: GameState,
    sequence: u64,
    visible_countries: BTreeMap<u32, CountryView>,
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

#[derive(Serialize)]
struct OwnerView {
    province_id: u32,
    country_id: u32,
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
}

#[derive(Clone, Serialize)]
struct Update {
    sequence: u64,
    clock: ClockView,
    countries: Vec<CountryView>,
}

#[derive(Serialize)]
pub(crate) struct AiLogEntry {
    tick: u64,
    country_id: u32,
    reason: String,
    command: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum UiCommand {
    #[serde(rename = "set_paused")]
    Paused { paused: bool },
    #[serde(rename = "set_speed")]
    Speed { speed: u8 },
    #[serde(rename = "set_fiscal_policy")]
    FiscalPolicy {
        country_id: u32,
        tax_rate: i64,
        monthly_spending: String,
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
        })
    }
}

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let game = load_embedded_scenario().map_err(std::io::Error::other)?;
    let visible_countries = country_views(&game);
    app.manage(Mutex::new(Session {
        game,
        sequence: 0,
        visible_countries,
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
    let owners = polity
        .countries
        .iter()
        .flat_map(|(id, country)| {
            country.provinces.iter().map(|province| OwnerView {
                province_id: province.get(),
                country_id: id.get(),
            })
        })
        .collect();
    Ok(Snapshot {
        sequence: session.sequence,
        clock: clock_view(&session.game),
        local_country_id,
        countries: session.visible_countries.values().cloned().collect(),
        owners,
        max_tax_rate: polity.rules.max_tax_rate.raw(),
        max_monthly_spending: money(polity.rules.max_monthly_spending),
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
            reason: format!("{:?}", decision.reason),
            command: format!("{:?}", decision.command),
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
    let changed = countries
        .iter()
        .filter(|(id, country)| session.visible_countries.get(id) != Some(*country))
        .map(|(_, country)| country.clone())
        .collect();
    session.visible_countries = countries;
    session.sequence += 1;
    if let Err(error) = app.emit(
        UPDATE_EVENT,
        Update {
            sequence: session.sequence,
            clock: clock_view(&session.game),
            countries: changed,
        },
    ) {
        eprintln!("Could not publish the game update: {error}");
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
                    death: character.death.map(format_date),
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
