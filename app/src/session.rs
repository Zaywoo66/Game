use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use strategy_ai::{
    alliance_acceptance_factors, marriage_acceptance_factors, vassal_acceptance_factors,
};
use strategy_app::{
    embedded_technologies,
    lan::{ClientPhase, HostConfig, LanClient, LanHost, LanMetrics},
    load_embedded_scenario, tick_host_with_log, AiDecisionLog,
};
use strategy_net::discover_rooms;
use strategy_sim::{
    ArmyId, Command, CommandActor, Controller, CountryId, Date, Effect, FiscalPolicy, Fixed,
    FleetId, GameState, ModifierKind, PlayerId, ProvinceId, Speed, Tick, TimeCommand, WarId,
    WarState,
};
use tauri::{App, AppHandle, Emitter, Manager, State};

const HOST: PlayerId = PlayerId::new(1);
const GUEST: PlayerId = PlayerId::new(2);
const UPDATE_EVENT: &str = "strategy:update";
const LAN_EVENT: &str = "strategy:lan";

mod lan_log;

use lan_log::LanLog;

pub(crate) struct Session {
    game: GameState,
    sequence: u64,
    visible_countries: BTreeMap<u32, CountryView>,
    visible_events: Vec<EventView>,
    visible_owners: BTreeMap<u32, u32>,
    military: MilitaryView,
    ai_log: AiDecisionLog,
    mode: Mode,
    generation: u64,
    operation: u64,
    network_sequence: u64,
    client_revision: u64,
    last_lan_status: Option<LanStatus>,
    lifecycle: Arc<Lifecycle>,
}

#[derive(Clone)]
enum Mode {
    Solo,
    Host(Arc<LanHost>),
    Guest(Arc<LanClient>),
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LanStatus {
    mode: &'static str,
    state: &'static str,
    room: Option<String>,
    address: Option<String>,
    guest_connected: bool,
    guest_country_id: Option<u32>,
    can_takeover: bool,
    temporary_ai: bool,
    verified_checksums: u64,
    checksum_mismatches: u64,
    detail: Option<String>,
    generation: u64,
}

#[derive(Serialize)]
pub(crate) struct RoomView {
    name: String,
    address: String,
    player_count: u8,
    version: u16,
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
    sex: &'static str,
    spouse_id: Option<u32>,
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
    tribute_paid: String,
    tribute_received: String,
    expenses: String,
    tax_rate: i64,
    monthly_spending: String,
    bankrupt: bool,
    regency: bool,
    ruler: Option<CharacterView>,
    heir: Option<CharacterView>,
    research_points: Option<u32>,
    known_technologies: Vec<String>,
    known_ideas: Vec<String>,
    government: String,
    known_reforms: Vec<String>,
    chosen_reform_tiers: Vec<u16>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct AcceptanceFactorView {
    key: &'static str,
    value: i32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct ProposalAcceptanceView {
    outcome: &'static str,
    factors: Vec<AcceptanceFactorView>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct CountryAcceptancePreviewView {
    country_id: u32,
    alliance: ProposalAcceptanceView,
    vassalage: ProposalAcceptanceView,
    marriage: Option<ProposalAcceptanceView>,
}

#[derive(Serialize)]
struct TechnologyEffectView {
    key: String,
    value: String,
    kind: &'static str,
    source_key: String,
}

#[derive(Serialize)]
struct TechnologyView {
    key: String,
    branch: String,
    cost: u32,
    requires: Vec<String>,
    pos_x: i32,
    pos_y: i32,
    effects: Vec<TechnologyEffectView>,
}

#[derive(Serialize)]
struct IdeaEffectView {
    key: String,
    value: String,
    kind: &'static str,
}

#[derive(Serialize)]
struct IdeaView {
    key: String,
    country: String,
    name_key: String,
    available_countries: Vec<u32>,
    effects: Vec<IdeaEffectView>,
}

#[derive(Serialize)]
struct ReformView {
    key: String,
    government: String,
    tier: u16,
    ai_weight: u16,
    effects: Vec<IdeaEffectView>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct EventView {
    country_id: u32,
    key: String,
    title_key: String,
    desc_key: String,
    options: Vec<EventOptionView>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct EventOptionView {
    text_key: String,
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
    embarked_on: Option<u32>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct FleetView {
    id: u32,
    country_id: u32,
    sea_province_id: u32,
    ships: u32,
    cargo_army_id: Option<u32>,
    to_sea_province_id: Option<u32>,
    progress: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct ShipyardView {
    port_province_id: u32,
    sea_province_id: u32,
    country_id: u32,
    days_remaining: u8,
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
    attacker_supporters: Vec<u32>,
    defender_supporters: Vec<u32>,
    goal_province_id: u32,
    independence: bool,
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

#[derive(Clone, PartialEq, Eq, Serialize)]
struct GuaranteeView {
    guarantor_id: u32,
    protected_id: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct AllianceView {
    a_id: u32,
    b_id: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct AllianceOfferView {
    from_id: u32,
    toward_id: u32,
    expires_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct MarriageOfferView {
    from_country_id: u32,
    toward_country_id: u32,
    from_character_id: u32,
    toward_character_id: u32,
    expires_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct OpinionView {
    from_id: u32,
    toward_id: u32,
    points: i16,
    effective_points: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct TruceView {
    a_id: u32,
    b_id: u32,
    remaining_days: u32,
    ends_on: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct VassalView {
    subject_id: u32,
    overlord_id: u32,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct VassalOfferView {
    overlord_id: u32,
    subject_id: u32,
    expires_at: String,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct CallToArmsOfferView {
    war_id: u32,
    from_id: u32,
    toward_id: u32,
    side: String,
    expires_at: String,
}

#[derive(Default)]
struct MilitaryView {
    armies: BTreeMap<u32, ArmyView>,
    fleets: BTreeMap<u32, FleetView>,
    wars: BTreeMap<u32, WarView>,
    sieges: Vec<SiegeView>,
}

#[derive(Serialize)]
pub(crate) struct Snapshot {
    sequence: u64,
    generation: u64,
    clock: ClockView,
    local_country_id: Option<u32>,
    countries: Vec<CountryView>,
    acceptance_previews: Vec<CountryAcceptancePreviewView>,
    technologies: Vec<TechnologyView>,
    ideas: Vec<IdeaView>,
    reforms: Vec<ReformView>,
    pending_events: Vec<EventView>,
    owners: Vec<OwnerView>,
    max_tax_rate: i64,
    max_monthly_spending: String,
    guarantees: Vec<GuaranteeView>,
    alliances: Vec<AllianceView>,
    alliance_offers: Vec<AllianceOfferView>,
    marriage_offers: Vec<MarriageOfferView>,
    marriage_adulthood_years: u16,
    opinions: Vec<OpinionView>,
    truces: Vec<TruceView>,
    blockaded_ports: Vec<u32>,
    vassals: Vec<VassalView>,
    vassal_offers: Vec<VassalOfferView>,
    call_to_arms_offers: Vec<CallToArmsOfferView>,
    armies: Vec<ArmyView>,
    fleets: Vec<FleetView>,
    shipyards: Vec<ShipyardView>,
    wars: Vec<WarView>,
    sieges: Vec<SiegeView>,
    forts: Vec<FortView>,
    min_raise_soldiers: u32,
    max_raise_soldiers: u32,
}

#[derive(Clone, Serialize)]
struct Update {
    sequence: u64,
    generation: u64,
    clock: ClockView,
    countries: Vec<CountryView>,
    acceptance_previews: Vec<CountryAcceptancePreviewView>,
    pending_events: Option<Vec<EventView>>,
    guarantees: Vec<GuaranteeView>,
    alliances: Vec<AllianceView>,
    alliance_offers: Vec<AllianceOfferView>,
    marriage_offers: Vec<MarriageOfferView>,
    marriage_adulthood_years: u16,
    opinions: Vec<OpinionView>,
    truces: Vec<TruceView>,
    blockaded_ports: Vec<u32>,
    vassals: Vec<VassalView>,
    vassal_offers: Vec<VassalOfferView>,
    call_to_arms_offers: Vec<CallToArmsOfferView>,
    owners: Vec<OwnerView>,
    armies: Vec<ArmyView>,
    removed_armies: Vec<u32>,
    fleets: Vec<FleetView>,
    shipyards: Vec<ShipyardView>,
    removed_fleets: Vec<u32>,
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
    MoveFleet {
        country_id: u32,
        fleet_id: u32,
        destination: u32,
    },
    BuildShip {
        country_id: u32,
        port_id: u32,
        sea_id: u32,
    },
    EmbarkArmy {
        country_id: u32,
        army_id: u32,
        fleet_id: u32,
    },
    DisembarkArmy {
        country_id: u32,
        fleet_id: u32,
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
    DeclareIndependence {
        country_id: u32,
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
    AcceptCallToArms {
        country_id: u32,
        war_id: u32,
        from_id: u32,
    },
    RejectCallToArms {
        country_id: u32,
        war_id: u32,
        from_id: u32,
    },
    OfferAlliance {
        country_id: u32,
        toward_id: u32,
    },
    AcceptAlliance {
        country_id: u32,
        from_id: u32,
    },
    RejectAlliance {
        country_id: u32,
        from_id: u32,
    },
    BreakAlliance {
        country_id: u32,
        other_id: u32,
    },
    OfferRoyalMarriage {
        country_id: u32,
        character_id: u32,
        toward_id: u32,
        partner_id: u32,
    },
    AcceptRoyalMarriage {
        country_id: u32,
        from_id: u32,
        character_id: u32,
        partner_id: u32,
    },
    RejectRoyalMarriage {
        country_id: u32,
        from_id: u32,
        character_id: u32,
        partner_id: u32,
    },
    UnlockTechnology {
        country_id: u32,
        key: String,
    },
    UnlockIdea {
        country_id: u32,
        key: String,
    },
    UnlockReform {
        country_id: u32,
        key: String,
    },
    ChooseEvent {
        country_id: u32,
        key: String,
        option: u32,
    },
    Guarantee {
        country_id: u32,
        protected_id: u32,
    },
    RevokeGuarantee {
        country_id: u32,
        protected_id: u32,
    },
    OfferVassalage {
        country_id: u32,
        toward_id: u32,
    },
    AcceptVassalage {
        country_id: u32,
        from_id: u32,
    },
    RejectVassalage {
        country_id: u32,
        from_id: u32,
    },
    ReleaseVassal {
        country_id: u32,
        subject_id: u32,
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
            Self::MoveFleet {
                country_id,
                fleet_id,
                destination,
            } => Command::MoveFleet {
                country: CountryId::new(country_id),
                fleet: FleetId::new(fleet_id),
                destination: ProvinceId::new(destination),
            },
            Self::BuildShip {
                country_id,
                port_id,
                sea_id,
            } => Command::BuildShip {
                country: CountryId::new(country_id),
                port: ProvinceId::new(port_id),
                sea: ProvinceId::new(sea_id),
            },
            Self::EmbarkArmy {
                country_id,
                army_id,
                fleet_id,
            } => Command::EmbarkArmy {
                country: CountryId::new(country_id),
                army: ArmyId::new(army_id),
                fleet: FleetId::new(fleet_id),
            },
            Self::DisembarkArmy {
                country_id,
                fleet_id,
                destination,
            } => Command::DisembarkArmy {
                country: CountryId::new(country_id),
                fleet: FleetId::new(fleet_id),
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
            Self::DeclareIndependence { country_id } => Command::DeclareIndependence {
                country: CountryId::new(country_id),
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
            Self::AcceptCallToArms {
                country_id,
                war_id,
                from_id,
            } => Command::AcceptCallToArms {
                country: CountryId::new(country_id),
                war: WarId::new(war_id),
                from: CountryId::new(from_id),
            },
            Self::RejectCallToArms {
                country_id,
                war_id,
                from_id,
            } => Command::RejectCallToArms {
                country: CountryId::new(country_id),
                war: WarId::new(war_id),
                from: CountryId::new(from_id),
            },
            Self::OfferAlliance {
                country_id,
                toward_id,
            } => Command::OfferAlliance {
                country: CountryId::new(country_id),
                toward: CountryId::new(toward_id),
            },
            Self::AcceptAlliance {
                country_id,
                from_id,
            } => Command::AcceptAlliance {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
            },
            Self::RejectAlliance {
                country_id,
                from_id,
            } => Command::RejectAlliance {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
            },
            Self::BreakAlliance {
                country_id,
                other_id,
            } => Command::BreakAlliance {
                country: CountryId::new(country_id),
                other: CountryId::new(other_id),
            },
            Self::OfferRoyalMarriage {
                country_id,
                character_id,
                toward_id,
                partner_id,
            } => Command::OfferRoyalMarriage {
                country: CountryId::new(country_id),
                character: strategy_sim::CharacterId::new(character_id),
                toward: CountryId::new(toward_id),
                partner: strategy_sim::CharacterId::new(partner_id),
            },
            Self::AcceptRoyalMarriage {
                country_id,
                from_id,
                character_id,
                partner_id,
            } => Command::AcceptRoyalMarriage {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
                character: strategy_sim::CharacterId::new(character_id),
                partner: strategy_sim::CharacterId::new(partner_id),
            },
            Self::RejectRoyalMarriage {
                country_id,
                from_id,
                character_id,
                partner_id,
            } => Command::RejectRoyalMarriage {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
                character: strategy_sim::CharacterId::new(character_id),
                partner: strategy_sim::CharacterId::new(partner_id),
            },
            Self::UnlockTechnology { country_id, key } => Command::UnlockTechnology {
                country: CountryId::new(country_id),
                key,
            },
            Self::UnlockIdea { country_id, key } => Command::UnlockIdea {
                country: CountryId::new(country_id),
                key,
            },
            Self::UnlockReform { country_id, key } => Command::UnlockReform {
                country: CountryId::new(country_id),
                key,
            },
            Self::ChooseEvent {
                country_id,
                key,
                option,
            } => Command::ChooseEvent {
                country: CountryId::new(country_id),
                key,
                option,
            },
            Self::Guarantee {
                country_id,
                protected_id,
            } => Command::Guarantee {
                country: CountryId::new(country_id),
                protected: CountryId::new(protected_id),
            },
            Self::RevokeGuarantee {
                country_id,
                protected_id,
            } => Command::RevokeGuarantee {
                country: CountryId::new(country_id),
                protected: CountryId::new(protected_id),
            },
            Self::OfferVassalage {
                country_id,
                toward_id,
            } => Command::OfferVassalage {
                country: CountryId::new(country_id),
                toward: CountryId::new(toward_id),
            },
            Self::AcceptVassalage {
                country_id,
                from_id,
            } => Command::AcceptVassalage {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
            },
            Self::RejectVassalage {
                country_id,
                from_id,
            } => Command::RejectVassalage {
                country: CountryId::new(country_id),
                from: CountryId::new(from_id),
            },
            Self::ReleaseVassal {
                country_id,
                subject_id,
            } => Command::ReleaseVassal {
                country: CountryId::new(country_id),
                subject: CountryId::new(subject_id),
            },
        })
    }
}

pub(crate) fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).peekable();
    let lan_log = if args.peek().is_some_and(|argument| argument == "--lan-log") {
        args.next();
        let path = args.next().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Supply a new LAN log file path.",
            )
        })?;
        Some(LanLog::open(std::path::Path::new(&path))?)
    } else {
        None
    };
    let startup = Startup::parse(args)?;
    let lifecycle = Arc::new(Lifecycle {
        stopping: AtomicBool::new(false),
        exit_code: AtomicI32::new(0),
        logging: lan_log.is_some(),
        log: Mutex::new(lan_log),
    });
    app.manage(Arc::clone(&lifecycle));
    let game = load_embedded_scenario().map_err(std::io::Error::other)?;
    let visible_countries = country_views(&game);
    let visible_events = event_views(&game, HOST);
    let visible_owners = owner_views(&game);
    let military = military_view(&game);
    app.manage(Mutex::new(Session {
        game,
        sequence: 0,
        visible_countries,
        visible_events,
        visible_owners,
        military,
        ai_log: AiDecisionLog::default(),
        mode: Mode::Solo,
        generation: 0,
        operation: 0,
        network_sequence: 0,
        client_revision: 0,
        last_lan_status: None,
        lifecycle,
    }));

    let handle = app.handle().clone();
    thread::Builder::new()
        .name("strategy-host-clock".into())
        .spawn(move || run_clock(handle))?;
    if let Some(startup) = startup {
        let handle = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = startup.run(&handle).await {
                eprintln!("LAN startup failed: {error}");
                handle.exit(1);
            }
        });
    }
    Ok(())
}

enum Startup {
    Host { config: HostConfig, autostart: bool },
    Guest { address: String, name: String },
}

impl Startup {
    fn parse(args: impl Iterator<Item = String>) -> Result<Option<Self>, std::io::Error> {
        let mut args = args.peekable();
        let invalid = |message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message);
        let Some(action) = args.next() else {
            return Ok(None);
        };
        let startup = match action.as_str() {
            "--lan-host-config" => {
                let path = args
                    .next()
                    .ok_or_else(|| invalid("Supply a host config JSON path."))?;
                let config = serde_json::from_slice(&std::fs::read(path)?)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
                let autostart = args
                    .peek()
                    .is_some_and(|argument| argument == "--lan-autostart");
                if autostart {
                    args.next();
                }
                Self::Host { config, autostart }
            }
            "--lan-join" => Self::Guest {
                address: args
                    .next()
                    .ok_or_else(|| invalid("Supply a host IP:port."))?,
                name: args
                    .next()
                    .ok_or_else(|| invalid("Supply a player name."))?,
            },
            _ => return Err(invalid("Unknown startup option.")),
        };
        if args.next().is_some() {
            return Err(invalid("Unexpected trailing startup arguments."));
        }
        Ok(Some(startup))
    }

    async fn run(self, app: &AppHandle) -> Result<(), String> {
        let state = app.state::<Mutex<Session>>();
        match self {
            Self::Host { config, autostart } => {
                let status = lan_start(app.clone(), state, config).await?;
                eprintln!(
                    "LAN host ready address={}",
                    status.address.as_deref().unwrap_or_default()
                );
                if autostart {
                    let state = app.state::<Mutex<Session>>();
                    let session = state
                        .lock()
                        .map_err(|_| "The game session is unavailable.")?;
                    if let Mode::Host(host) = &session.mode {
                        host.command(Command::Time(TimeCommand::SetPaused(false)))?;
                    }
                }
            }
            Self::Guest { address, name } => {
                lan_join(app.clone(), state, address, name).await?;
                eprintln!("LAN guest connected.");
            }
        }
        Ok(())
    }
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
    let player = local_player(&session.mode);
    let local_country_id = session.game.countries().find_map(|(id, country)| {
        (country.controller() == &Controller::Human(player)).then_some(id.get())
    });
    let war = session.game.war_state();
    Ok(Snapshot {
        sequence: session.sequence,
        generation: session.generation,
        clock: clock_view(&session.game),
        local_country_id,
        countries: session.visible_countries.values().cloned().collect(),
        acceptance_previews: acceptance_preview_views(&session.game, local_country_id),
        technologies: technology_views(&session.game)?,
        ideas: idea_views(&session.game),
        reforms: reform_views(&session.game),
        pending_events: event_views(&session.game, player),
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
        guarantees: guarantee_views(&session.game),
        alliances: alliance_views(&session.game),
        alliance_offers: alliance_offer_views(&session.game),
        marriage_offers: marriage_offer_views(&session.game),
        marriage_adulthood_years: polity.rules.adulthood_years,
        opinions: opinion_views(&session.game),
        truces: truce_views(&session.game),
        blockaded_ports: session
            .game
            .blockaded_ports()
            .into_iter()
            .map(|id| id.get())
            .collect(),
        vassals: vassal_views(&session.game),
        vassal_offers: vassal_offer_views(&session.game),
        call_to_arms_offers: call_to_arms_offer_views(&session.game),
        armies: session.military.armies.values().cloned().collect(),
        fleets: session.military.fleets.values().cloned().collect(),
        shipyards: shipyard_views(&session.game),
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
pub(crate) async fn game_command(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
    command: UiCommand,
) -> Result<(), String> {
    let command = command.into_sim()?;
    let client = {
        let mut session = state
            .lock()
            .map_err(|_| "The game session is unavailable.")?;
        match session.mode.clone() {
            Mode::Solo => {
                let delta = session
                    .game
                    .apply(CommandActor::Host(HOST), &command)
                    .map_err(|error| format!("Command rejected ({error:?})."))?;
                if !delta.changes().is_empty() {
                    publish(&app, &mut session);
                }
                return Ok(());
            }
            Mode::Host(host) => return host.command(command),
            Mode::Guest(client) => client,
        }
    };
    client.command(command).await
}

#[tauri::command]
pub(crate) fn lan_status(state: State<'_, Mutex<Session>>) -> Result<LanStatus, String> {
    let session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    Ok(status(&session))
}

#[tauri::command]
pub(crate) async fn lan_discover() -> Result<Vec<RoomView>, String> {
    let rooms = discover_rooms(Duration::from_secs(2))
        .await
        .map_err(|error| error.to_string())?;
    Ok(rooms
        .into_iter()
        .flat_map(|room| {
            room.addresses.into_iter().map(move |address| RoomView {
                name: room.room.clone(),
                address: address.to_string(),
                player_count: room.player_count,
                version: room.version,
            })
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn lan_start(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
    config: HostConfig,
) -> Result<LanStatus, String> {
    if config.port == 0 {
        return Err("Enter a TCP port from 1 to 65535.".into());
    }
    let operation = begin_operation(&state)?;
    let host = Arc::new(LanHost::start(config, load_embedded_scenario()?).await?);
    let view = host.view();
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    if session.operation != operation {
        return Err("Another room action superseded this request.".into());
    }
    session.mode = Mode::Host(host);
    reset_game(&mut session, &view.game, view.sequence);
    start_lan_log(&session, "host");
    Ok(emit_lan_status(&app, &mut session))
}

#[tauri::command]
pub(crate) async fn lan_join(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
    address: String,
    name: String,
) -> Result<LanStatus, String> {
    let address = address
        .trim()
        .parse()
        .map_err(|_| "Enter the host's IP address and port, for example 192.168.1.10:47620.")?;
    let operation = begin_operation(&state)?;
    let graph = load_embedded_scenario()?.province_graph().clone();
    let client = Arc::new(LanClient::join(address, name, graph).await?);
    let view = client.subscribe().borrow().clone();
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    if session.operation != operation {
        return Err("Another room action superseded this request.".into());
    }
    session.mode = Mode::Guest(client);
    reset_game(&mut session, &view.game, view.sequence);
    session.client_revision = view.revision;
    start_lan_log(&session, "guest");
    Ok(emit_lan_status(&app, &mut session))
}

#[tauri::command]
pub(crate) fn lan_leave(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
) -> Result<LanStatus, String> {
    let game = load_embedded_scenario()?;
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    session.operation += 1;
    finish_lan_log(&session, "room_leave");
    session.mode = Mode::Solo;
    reset_game(&mut session, &game, 0);
    Ok(emit_lan_status(&app, &mut session))
}

#[tauri::command]
pub(crate) fn lan_takeover(
    app: AppHandle,
    state: State<'_, Mutex<Session>>,
) -> Result<LanStatus, String> {
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    let Mode::Host(host) = &session.mode else {
        return Err("Only the LAN host can assign temporary AI.".into());
    };
    host.take_over()?;
    Ok(emit_lan_status(&app, &mut session))
}

fn begin_operation(state: &Mutex<Session>) -> Result<u64, String> {
    let mut session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    if !matches!(session.mode, Mode::Solo) {
        return Err("Leave the current room first.".into());
    }
    session.operation += 1;
    Ok(session.operation)
}

#[tauri::command]
pub(crate) fn game_ai_log(state: State<'_, Mutex<Session>>) -> Result<Vec<AiLogEntry>, String> {
    let session = state
        .lock()
        .map_err(|_| "The game session is unavailable.")?;
    let decisions = match &session.mode {
        Mode::Solo => session.ai_log.entries().cloned().collect(),
        Mode::Host(host) => host.ai_log(),
        Mode::Guest(_) => Vec::new(),
    };
    Ok(decisions
        .iter()
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
    let lifecycle = app.state::<Arc<Lifecycle>>();
    let mut previous = None;
    let mut deadline = Instant::now();
    while !lifecycle.stopping.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(16));
        if lifecycle.stopping.load(Ordering::Relaxed) {
            break;
        }
        let Ok(mut session) = state.lock() else {
            return;
        };
        refresh_network_game(&app, &mut session);
        let time = session.game.time();
        let speed = time.speed();
        let paused = time.is_paused();
        let now = Instant::now();
        if lifecycle.logging && !matches!(session.mode, Mode::Solo) {
            let tick = time.tick().get();
            let metrics = network_metrics(&session.mode);
            lifecycle.with_log(|log| log.observe(now, tick, metrics));
        }
        let period = Duration::from_millis(1_000 >> (speed.get() - 1));
        let clock_state = (session.generation, speed, paused);
        if previous != Some(clock_state) {
            previous = Some(clock_state);
            deadline = now + period;
        }
        if paused || now < deadline || matches!(session.mode, Mode::Guest(_)) {
            continue;
        }
        let outcome = match session.mode.clone() {
            Mode::Solo => {
                let Session { game, ai_log, .. } = &mut *session;
                tick_host_with_log(game, ai_log)
                    .map(|_| publish(&app, &mut session))
                    .map_err(|error| format!("{error:?}"))
            }
            Mode::Host(host) => host.tick(),
            Mode::Guest(_) => unreachable!("guests do not advance the clock"),
        };
        match outcome {
            Ok(()) => {}
            Err(error) => {
                eprintln!("Host clock stopped: {error}");
                finish_lan_log(&session, "host_clock_error");
                return;
            }
        }
        deadline = now + period;
    }
}

fn network_metrics(mode: &Mode) -> LanMetrics {
    match mode {
        Mode::Solo => LanMetrics::default(),
        Mode::Host(host) => host.metrics(),
        Mode::Guest(client) => client.metrics(),
    }
}

struct Lifecycle {
    stopping: AtomicBool,
    exit_code: AtomicI32,
    logging: bool,
    log: Mutex<Option<LanLog>>,
}

impl Lifecycle {
    fn with_log(&self, action: impl FnOnce(&mut LanLog)) {
        if let Ok(mut log) = self.log.lock() {
            if let Some(log) = log.as_mut() {
                action(log);
            }
        }
    }
}

fn start_lan_log(session: &Session, role: &'static str) {
    let tick = session.game.time().tick().get();
    session.lifecycle.with_log(|log| log.start(role, tick));
}

fn finish_lan_log(session: &Session, reason: &str) {
    if session.lifecycle.logging {
        let tick = session.game.time().tick().get();
        let metrics = network_metrics(&session.mode);
        session.lifecycle.with_log(|log| {
            log.observe(Instant::now(), tick, metrics);
            log.finish(reason);
        });
    }
}

pub(crate) fn on_app_event(app: &AppHandle, event: tauri::RunEvent) {
    let Some(lifecycle) = app.try_state::<Arc<Lifecycle>>() else {
        return;
    };
    // The window thread must not acquire the game mutex: the clock can be
    // emitting a WebView update while the last window is closing.
    match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { .. },
            ..
        } if label == "main" => {
            lifecycle.stopping.store(true, Ordering::Relaxed);
            app.exit(0);
        }
        tauri::RunEvent::ExitRequested { code, .. } => {
            lifecycle.stopping.store(true, Ordering::Relaxed);
            if let Some(code) = code {
                lifecycle.exit_code.store(code, Ordering::Relaxed);
            }
        }
        tauri::RunEvent::Exit => lifecycle.with_log(|log| {
            log.finish("app_exit");
            log.exit(lifecycle.exit_code.load(Ordering::Relaxed));
        }),
        _ => {}
    }
}

fn reset_game(session: &mut Session, game: &GameState, sequence: u64) {
    session.generation += 1;
    session.network_sequence = sequence;
    session.game = game.clone();
    session.visible_countries = country_views(game);
    session.visible_events = event_views(game, local_player(&session.mode));
    session.visible_owners = owner_views(game);
    session.military = military_view(game);
    session.ai_log = AiDecisionLog::default();
}

fn refresh_network_game(app: &AppHandle, session: &mut Session) {
    match session.mode.clone() {
        Mode::Solo => return,
        Mode::Host(host) => {
            let view = host.view();
            sync_delta(app, session, &view.game, view.sequence);
        }
        Mode::Guest(client) => {
            let view = client.subscribe().borrow().clone();
            if view.revision != session.client_revision {
                session.client_revision = view.revision;
                reset_game(session, &view.game, view.sequence);
            } else {
                sync_delta(app, session, &view.game, view.sequence);
            }
        }
    }
    emit_lan_status(app, session);
}

fn sync_delta(app: &AppHandle, session: &mut Session, game: &GameState, sequence: u64) {
    if sequence != session.network_sequence {
        session.network_sequence = sequence;
        session.game = game.clone();
        publish(app, session);
    }
}

fn status(session: &Session) -> LanStatus {
    let mut status = LanStatus {
        mode: "solo",
        state: "idle",
        room: None,
        address: None,
        guest_connected: false,
        guest_country_id: None,
        can_takeover: false,
        temporary_ai: false,
        verified_checksums: 0,
        checksum_mismatches: 0,
        detail: None,
        generation: session.generation,
    };
    match &session.mode {
        Mode::Solo => {}
        Mode::Host(host) => {
            let view = host.view();
            status.mode = "host";
            status.state = "hosting";
            status.room = Some(view.room);
            status.address = Some(view.address.to_string());
            status.guest_connected = view.guest_connected;
            status.guest_country_id = Some(view.guest_country_id);
            status.can_takeover = view.can_takeover;
            status.temporary_ai = view.temporary_ai;
            status.detail = view.detail;
        }
        Mode::Guest(client) => {
            let view = client.subscribe().borrow().clone();
            status.mode = "guest";
            status.state = match view.phase {
                ClientPhase::Connected => "connected",
                ClientPhase::Reconnecting => "reconnecting",
                ClientPhase::Resyncing => "resyncing",
            };
            status.room = Some(view.room);
            status.address = Some(view.address.to_string());
            status.guest_connected = view.phase == ClientPhase::Connected;
            status.guest_country_id = view.game.polity().and_then(|polity| {
                polity
                    .countries
                    .iter()
                    .find_map(|(id, country)| (country.tag == view.guest_tag).then_some(id.get()))
            });
            status.verified_checksums = view.verified_checksums;
            status.checksum_mismatches = view.checksum_mismatches;
            status.detail = view.detail;
        }
    }
    status
}

fn emit_lan_status(app: &AppHandle, session: &mut Session) -> LanStatus {
    let next = status(session);
    if session.last_lan_status.as_ref() != Some(&next) {
        if next.mode == "guest"
            && session.last_lan_status.as_ref().is_none_or(|previous| {
                previous.verified_checksums != next.verified_checksums
                    || previous.checksum_mismatches != next.checksum_mismatches
            })
        {
            eprintln!(
                "LAN checksums tick={} sequence={} verified={} mismatches={}",
                session.game.time().tick().get(),
                session.network_sequence,
                next.verified_checksums,
                next.checksum_mismatches
            );
        }
        session.last_lan_status = Some(next.clone());
        if let Err(error) = app.emit(LAN_EVENT, next.clone()) {
            eprintln!("Could not publish LAN status: {error}");
        }
    }
    next
}

fn publish(app: &AppHandle, session: &mut Session) {
    let countries = country_views(&session.game);
    let events = event_views(&session.game, local_player(&session.mode));
    let owners = owner_views(&session.game);
    let military = military_view(&session.game);
    session.sequence += 1;
    let update = Update {
        sequence: session.sequence,
        generation: session.generation,
        clock: clock_view(&session.game),
        countries: changed_rows(&session.visible_countries, &countries),
        acceptance_previews: acceptance_preview_views(
            &session.game,
            session.game.countries().find_map(|(id, country)| {
                (country.controller() == &Controller::Human(local_player(&session.mode)))
                    .then_some(id.get())
            }),
        ),
        pending_events: (session.visible_events != events).then(|| events.clone()),
        guarantees: guarantee_views(&session.game),
        alliances: alliance_views(&session.game),
        alliance_offers: alliance_offer_views(&session.game),
        marriage_offers: marriage_offer_views(&session.game),
        marriage_adulthood_years: session
            .game
            .polity()
            .map_or(18, |p| p.rules.adulthood_years),
        opinions: opinion_views(&session.game),
        truces: truce_views(&session.game),
        blockaded_ports: session
            .game
            .blockaded_ports()
            .into_iter()
            .map(|id| id.get())
            .collect(),
        vassals: vassal_views(&session.game),
        vassal_offers: vassal_offer_views(&session.game),
        call_to_arms_offers: call_to_arms_offer_views(&session.game),
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
        fleets: changed_rows(&session.military.fleets, &military.fleets),
        removed_fleets: removed_rows(&session.military.fleets, &military.fleets),
        shipyards: shipyard_views(&session.game),
        wars: changed_rows(&session.military.wars, &military.wars),
        removed_wars: removed_rows(&session.military.wars, &military.wars),
        sieges: (session.military.sieges != military.sieges).then(|| military.sieges.clone()),
    };
    session.visible_countries = countries;
    session.visible_events = events;
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

fn guarantee_views(game: &GameState) -> Vec<GuaranteeView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.guarantees)
        .map(|&(guarantor, protected)| GuaranteeView {
            guarantor_id: guarantor.get(),
            protected_id: protected.get(),
        })
        .collect()
}

fn alliance_views(game: &GameState) -> Vec<AllianceView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.alliances)
        .map(|&(a, b)| AllianceView {
            a_id: a.get(),
            b_id: b.get(),
        })
        .collect()
}

fn alliance_offer_views(game: &GameState) -> Vec<AllianceOfferView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.offers)
        .map(|(&(from, toward), &expires)| AllianceOfferView {
            from_id: from.get(),
            toward_id: toward.get(),
            expires_at: format_date(expires),
        })
        .collect()
}

fn marriage_offer_views(game: &GameState) -> Vec<MarriageOfferView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.marriage_offers)
        .filter_map(|(&(from, toward), &expires)| {
            Some(MarriageOfferView {
                from_country_id: game.character(from)?.country.get(),
                toward_country_id: game.character(toward)?.country.get(),
                from_character_id: from.get(),
                toward_character_id: toward.get(),
                expires_at: format_date(expires),
            })
        })
        .collect()
}

fn opinion_views(game: &GameState) -> Vec<OpinionView> {
    game.opinions()
        .map(|(from, toward, opinion)| OpinionView {
            from_id: from.get(),
            toward_id: toward.get(),
            points: opinion.points(),
            effective_points: game
                .effective_opinion(from, toward)
                .unwrap_or(opinion)
                .points(),
        })
        .collect()
}

fn truce_views(game: &GameState) -> Vec<TruceView> {
    game.war_state().map_or_else(Vec::new, |war| {
        collect_truce_views(war, game.time().tick(), game.time().date())
    })
}

fn collect_truce_views(war: &WarState, now: Tick, today: Date) -> Vec<TruceView> {
    let mut pairs = BTreeMap::<(CountryId, CountryId), u32>::new();
    let duration = u64::from(war.rules.truce_ticks);
    let mut record = |a: CountryId, b: CountryId, concluded_at: Tick| {
        let Some(elapsed) = now.get().checked_sub(concluded_at.get()) else {
            return;
        };
        if elapsed >= duration {
            return;
        }
        let pair = if a < b { (a, b) } else { (b, a) };
        let remaining = (duration - elapsed) as u32;
        pairs
            .entry(pair)
            .and_modify(|previous| *previous = (*previous).max(remaining))
            .or_insert(remaining);
    };
    for treaty in war.treaties.values() {
        record(treaty.attacker, treaty.defender, treaty.concluded_at);
    }
    for (&(a, b), &concluded_at) in &war.coalition_truces {
        record(a, b, concluded_at);
    }
    pairs
        .into_iter()
        .map(|((a, b), remaining_days)| TruceView {
            a_id: a.get(),
            b_id: b.get(),
            remaining_days,
            ends_on: today
                .checked_add_days(u64::from(remaining_days))
                .map(format_date),
        })
        .collect()
}

fn vassal_views(game: &GameState) -> Vec<VassalView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.vassals)
        .map(|(&subject, &overlord)| VassalView {
            subject_id: subject.get(),
            overlord_id: overlord.get(),
        })
        .collect()
}

fn vassal_offer_views(game: &GameState) -> Vec<VassalOfferView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.vassal_offers)
        .map(|(&(overlord, subject), &expires)| VassalOfferView {
            overlord_id: overlord.get(),
            subject_id: subject.get(),
            expires_at: format_date(expires),
        })
        .collect()
}

fn call_to_arms_offer_views(game: &GameState) -> Vec<CallToArmsOfferView> {
    game.diplomacy()
        .into_iter()
        .flat_map(|diplomacy| &diplomacy.call_to_arms_offers)
        .map(
            |(&(war, from, toward), &(side, expires_at))| CallToArmsOfferView {
                war_id: war.get(),
                from_id: from.get(),
                toward_id: toward.get(),
                side: match side {
                    strategy_sim::WarSide::Attacker => "attacker",
                    strategy_sim::WarSide::Defender => "defender",
                }
                .to_owned(),
                expires_at: format_date(expires_at),
            },
        )
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
        fleets: game
            .navy()
            .into_iter()
            .flat_map(|navy| &navy.fleets)
            .map(|(&id, fleet)| {
                let movement = fleet.movement.as_ref();
                let progress = movement.map_or(0, |movement| {
                    movement.route.first().map_or(0, |leg| {
                        i64::from(movement.elapsed_ticks) * 1000 / i64::from(leg.cost)
                    })
                });
                (
                    id.get(),
                    FleetView {
                        id: id.get(),
                        country_id: fleet.country.get(),
                        sea_province_id: fleet.sea.get(),
                        ships: fleet.ships,
                        cargo_army_id: fleet.cargo.map(ArmyId::get),
                        to_sea_province_id: movement
                            .and_then(|m| m.route.first())
                            .map(|leg| leg.to.get()),
                        progress,
                    },
                )
            })
            .collect(),
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
                        embarked_on: game.navy().and_then(|navy| {
                            navy.fleets.iter().find_map(|(&fleet_id, fleet)| {
                                (fleet.cargo == Some(id)).then_some(fleet_id.get())
                            })
                        }),
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
                        attacker_supporters: war
                            .attacker_supporters
                            .iter()
                            .map(|id| id.get())
                            .collect(),
                        defender_supporters: war
                            .defender_supporters
                            .iter()
                            .map(|id| id.get())
                            .collect(),
                        goal_province_id: war.goal.get(),
                        independence: war.independence,
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

fn shipyard_views(game: &GameState) -> Vec<ShipyardView> {
    game.navy()
        .into_iter()
        .flat_map(|navy| navy.shipyards.iter())
        .map(|(&port, order)| ShipyardView {
            port_province_id: port.get(),
            sea_province_id: order.sea.get(),
            country_id: order.country.get(),
            days_remaining: order.days_remaining,
        })
        .collect()
}

fn local_player(mode: &Mode) -> PlayerId {
    if matches!(mode, Mode::Guest(_)) {
        GUEST
    } else {
        HOST
    }
}

fn event_views(game: &GameState, player: PlayerId) -> Vec<EventView> {
    let Some(events) = game.events() else {
        return Vec::new();
    };
    events
        .pending
        .iter()
        .filter_map(|((country, key), _)| {
            let is_local = game.countries().any(|(id, state)| {
                id == *country && state.controller() == &Controller::Human(player)
            });
            let definition = events.definitions.get(key)?;
            is_local.then(|| EventView {
                country_id: country.get(),
                key: key.clone(),
                title_key: definition.title_key.clone(),
                desc_key: definition.desc_key.clone(),
                options: definition
                    .options
                    .iter()
                    .map(|option| EventOptionView {
                        text_key: option.text_key.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
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
                    sex: match character.sex {
                        strategy_sim::Sex::Male => "male",
                        strategy_sim::Sex::Female => "female",
                        strategy_sim::Sex::Unknown => "unknown",
                    },
                    spouse_id: character.spouse.map(|spouse| spouse.get()),
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
                    tribute_paid: money(
                        economy
                            .last_month
                            .map_or(Fixed::ZERO, |budget| budget.tribute_paid),
                    ),
                    tribute_received: money(
                        economy
                            .last_month
                            .map_or(Fixed::ZERO, |budget| budget.tribute_received),
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
                    research_points: game.research_points(id),
                    known_technologies: game
                        .country(id)
                        .map(|state| state.known_technologies().map(str::to_owned).collect())
                        .unwrap_or_default(),
                    known_ideas: game
                        .ideas()
                        .into_iter()
                        .flat_map(|ideas| ideas.sources.keys())
                        .filter(|&(country_id, _)| *country_id == id)
                        .map(|(_, key)| key.clone())
                        .collect(),
                    government: country.government.clone(),
                    known_reforms: game
                        .reforms()
                        .into_iter()
                        .flat_map(|reforms| reforms.sources.keys())
                        .filter(|&(country_id, _)| *country_id == id)
                        .map(|(_, key)| key.clone())
                        .collect(),
                    chosen_reform_tiers: game
                        .reforms()
                        .into_iter()
                        .flat_map(|reforms| reforms.choices.keys())
                        .filter_map(|&(country_id, tier)| (country_id == id).then_some(tier))
                        .collect(),
                },
            )
        })
        .collect()
}

fn acceptance_preview_views(
    game: &GameState,
    local_country_id: Option<u32>,
) -> Vec<CountryAcceptancePreviewView> {
    let Some(local_country) = local_country_id.map(CountryId::new) else {
        return Vec::new();
    };
    let Some(polity) = game.polity() else {
        return Vec::new();
    };
    let Some(local_data) = polity.countries.get(&local_country) else {
        return Vec::new();
    };
    let mut previews = Vec::new();
    for (target, country_state) in game.countries() {
        let Controller::Ai(profile_key) = country_state.controller() else {
            continue;
        };
        if target == local_country {
            continue;
        }
        let Some(profile) = polity.ai_profiles.get(profile_key.as_str()) else {
            continue;
        };
        let alliance_factors = alliance_acceptance_factors(game, local_country, target, profile);
        let alliance_opinion = alliance_factors
            .iter()
            .find(|factor| factor.key == "opinion")
            .map_or(0, |factor| factor.value);
        let alliance_minimum = alliance_factors
            .iter()
            .find(|factor| factor.key == "profile_minimum")
            .map_or(0, |factor| factor.value);
        let direct_war = alliance_factors
            .iter()
            .find(|factor| factor.key == "direct_war")
            .is_some_and(|factor| factor.value == 1);
        let negative_chance = alliance_factors
            .iter()
            .find(|factor| factor.key == "negative_opinion_accept_percent")
            .map_or(0, |factor| factor.value);
        let alliance_outcome = if direct_war {
            "blocked by war"
        } else if alliance_opinion < 0 && negative_chance > 0 {
            "10% acceptance chance"
        } else if alliance_opinion >= 0 && alliance_opinion >= alliance_minimum {
            "would accept"
        } else {
            "would decline"
        };
        let alliance = ProposalAcceptanceView {
            outcome: alliance_outcome,
            factors: alliance_factors
                .into_iter()
                .map(|factor| AcceptanceFactorView {
                    key: factor.key,
                    value: factor.value,
                })
                .collect(),
        };

        let vassal_factors = vassal_acceptance_factors(game, local_country, target, profile);
        let vassalage = ProposalAcceptanceView {
            outcome: if vassal_factors[0].value >= vassal_factors[1].value
                && vassal_factors[2].value > vassal_factors[3].value
            {
                "would accept"
            } else {
                "would decline"
            },
            factors: vassal_factors
                .into_iter()
                .map(|factor| AcceptanceFactorView {
                    key: factor.key,
                    value: factor.value,
                })
                .collect(),
        };

        let target_data = &polity.countries[&target];
        let marriage = local_data
            .succession
            .ruler
            .into_iter()
            .chain(local_data.succession.heir)
            .flat_map(|character| {
                target_data
                    .succession
                    .ruler
                    .into_iter()
                    .chain(target_data.succession.heir)
                    .map(move |partner| (character, partner))
            })
            .find(|(character, partner)| {
                game.marriage_eligibility(local_country, *character, target, *partner)
                    .is_ok()
            })
            .map(|(character, partner)| {
                let factors = marriage_acceptance_factors(
                    game,
                    local_country,
                    character,
                    target,
                    partner,
                    profile,
                );
                let accepts = factors[2].value == 1 && factors[0].value >= factors[1].value;
                ProposalAcceptanceView {
                    outcome: if accepts {
                        "would accept"
                    } else {
                        "would decline"
                    },
                    factors: factors
                        .into_iter()
                        .map(|factor| AcceptanceFactorView {
                            key: factor.key,
                            value: factor.value,
                        })
                        .collect(),
                }
            });

        previews.push(CountryAcceptancePreviewView {
            country_id: target.get(),
            alliance,
            vassalage,
            marriage,
        });
    }
    previews
}

fn technology_views(game: &GameState) -> Result<Vec<TechnologyView>, String> {
    let Some(technology) = game.technologies() else {
        return Ok(Vec::new());
    };
    let packaged = embedded_technologies()?;
    technology
        .definitions
        .iter()
        .map(|(key, definition)| {
            let layout = packaged
                .get(key)
                .ok_or_else(|| format!("missing packaged technology layout for {key}"))?;
            let effects = definition
                .effects
                .iter()
                .map(|effect| match effect {
                    Effect::AddModifier {
                        key, value, kind, ..
                    } => TechnologyEffectView {
                        key: key.as_str().to_owned(),
                        value: value.raw().to_string(),
                        kind: match kind {
                            ModifierKind::Additive => "additive",
                            ModifierKind::Multiplicative => "multiplicative",
                        },
                        source_key: definition.key.clone(),
                    },
                })
                .collect();
            Ok(TechnologyView {
                key: key.clone(),
                branch: definition.branch.clone(),
                cost: definition.cost.get(),
                requires: definition.requires.clone(),
                pos_x: layout.position.x,
                pos_y: layout.position.y,
                effects,
            })
        })
        .collect()
}

fn idea_views(game: &GameState) -> Vec<IdeaView> {
    let Some(ideas) = game.ideas() else {
        return Vec::new();
    };
    ideas
        .definitions
        .values()
        .map(|definition| IdeaView {
            key: definition.key.clone(),
            country: definition.country.clone(),
            name_key: definition.name_key.clone(),
            available_countries: game
                .countries()
                .filter_map(|(id, _)| {
                    game.country(id)
                        .is_some_and(|country| definition.unlock.evaluate(country))
                        .then_some(id.get())
                })
                .collect(),
            effects: definition
                .modifiers
                .iter()
                .map(|effect| match effect {
                    Effect::AddModifier {
                        key, value, kind, ..
                    } => IdeaEffectView {
                        key: key.as_str().to_owned(),
                        value: value.raw().to_string(),
                        kind: match kind {
                            ModifierKind::Additive => "additive",
                            ModifierKind::Multiplicative => "multiplicative",
                        },
                    },
                })
                .collect(),
        })
        .collect()
}

fn reform_views(game: &GameState) -> Vec<ReformView> {
    let Some(reforms) = game.reforms() else {
        return Vec::new();
    };
    reforms
        .definitions
        .values()
        .map(|definition| ReformView {
            key: definition.key.clone(),
            government: definition.government.clone(),
            tier: definition.tier,
            ai_weight: definition.ai_weight,
            effects: definition
                .effects
                .iter()
                .map(|effect| match effect {
                    Effect::AddModifier {
                        key, value, kind, ..
                    } => IdeaEffectView {
                        key: key.as_str().to_owned(),
                        value: value.raw().to_string(),
                        kind: match kind {
                            ModifierKind::Additive => "additive",
                            ModifierKind::Multiplicative => "multiplicative",
                        },
                    },
                })
                .collect(),
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

#[cfg(test)]
mod tests {
    use super::{
        acceptance_preview_views, collect_truce_views, event_views, technology_views, Startup,
        TruceView, UiCommand, GUEST, HOST,
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU32, NonZeroU64},
    };
    use strategy_app::load_embedded_scenario;
    use strategy_data::parse_event_yaml;
    use strategy_sim::{
        Command, Controller, CountryId, CountryState, Date, GameState, MapIdentity, ModifierStore,
        PeaceTreaty, PlayerId, ProvinceGraph, SimTime, Speed, Tick, Trigger, WarId,
    };

    #[test]
    fn acceptance_preview_projection_covers_ai_countries_and_their_factors() {
        let game = load_embedded_scenario().unwrap();
        let local = game
            .countries()
            .find_map(|(id, country)| {
                (country.controller() == &Controller::Human(PlayerId::new(1))).then_some(id)
            })
            .expect("the scenario assigns a human player");
        let previews = acceptance_preview_views(&game, Some(local.get()));
        let ai_countries: BTreeSet<_> = game
            .countries()
            .filter_map(|(id, country)| {
                matches!(country.controller(), Controller::Ai(_)).then_some(id.get())
            })
            .collect();
        let preview_countries: BTreeSet<_> = previews.iter().map(|row| row.country_id).collect();

        assert!(!ai_countries.is_empty());
        assert_eq!(preview_countries, ai_countries);
        assert!(previews.iter().all(|row| {
            row.alliance
                .factors
                .iter()
                .any(|factor| factor.key == "negative_opinion_accept_percent")
                && row.vassalage.factors.len() == 4
        }));
    }

    #[test]
    fn truce_projection_includes_coalition_pairs_and_expires_on_time() {
        let mut war = load_embedded_scenario()
            .unwrap()
            .war_state()
            .unwrap()
            .clone();
        war.rules.truce_ticks = 5;
        war.treaties.insert(
            WarId::new(1),
            PeaceTreaty {
                attacker: CountryId::new(2),
                defender: CountryId::new(1),
                ceded: None,
                concluded_at: Tick::ZERO,
            },
        );
        war.coalition_truces
            .insert((CountryId::new(1), CountryId::new(3)), Tick::new(1));
        let start = Date::new(1440, 1, 1).unwrap();
        let at =
            |day| collect_truce_views(&war, Tick::new(day), start.checked_add_days(day).unwrap());
        assert_eq!(
            at(3),
            vec![
                TruceView {
                    a_id: 1,
                    b_id: 2,
                    remaining_days: 2,
                    ends_on: Some("1440-01-06".into()),
                },
                TruceView {
                    a_id: 1,
                    b_id: 3,
                    remaining_days: 3,
                    ends_on: Some("1440-01-07".into()),
                },
            ]
        );
        assert_eq!(at(5).len(), 1);
        assert_eq!(at(5)[0].remaining_days, 1);
        assert!(at(6).is_empty());
        war.treaties.insert(
            WarId::new(2),
            PeaceTreaty {
                attacker: CountryId::new(1),
                defender: CountryId::new(2),
                ceded: None,
                concluded_at: Tick::new(4),
            },
        );
        let rows = collect_truce_views(&war, Tick::new(5), start.checked_add_days(5).unwrap());
        assert_eq!(rows[0].remaining_days, 4);
        assert_eq!(rows[0].ends_on.as_deref(), Some("1440-01-10"));
    }

    #[test]
    fn packaged_technology_layout_projects_authoritative_effects() {
        let game = strategy_app::load_embedded_scenario().unwrap();
        let nodes = technology_views(&game).unwrap();
        assert_eq!(nodes.len(), game.technologies().unwrap().definitions.len());
        let military = nodes
            .iter()
            .find(|node| node.key == "professional_officers")
            .unwrap();
        assert_eq!((military.pos_x, military.pos_y), (1, 0));
        assert_eq!(military.requires, ["recordkeeping"]);
        assert_eq!(military.effects.len(), 1);
        assert_eq!(military.effects[0].source_key, military.key);
        assert_eq!(military.effects[0].value, "50");
    }
    #[test]
    fn guarantee_ui_commands_preserve_direction_and_country_ids() {
        for (json, revoke) in [
            (
                r#"{"type":"guarantee","country_id":3,"protected_id":2}"#,
                false,
            ),
            (
                r#"{"type":"revoke_guarantee","country_id":3,"protected_id":2}"#,
                true,
            ),
        ] {
            let command = serde_json::from_str::<UiCommand>(json)
                .unwrap()
                .into_sim()
                .unwrap();
            assert_eq!(
                command,
                if revoke {
                    Command::RevokeGuarantee {
                        country: CountryId::new(3),
                        protected: CountryId::new(2),
                    }
                } else {
                    Command::Guarantee {
                        country: CountryId::new(3),
                        protected: CountryId::new(2),
                    }
                }
            );
        }
    }

    #[test]
    fn alliance_ui_commands_preserve_direction_and_country_ids() {
        let cases = [
            (
                r#"{"type":"offer_alliance","country_id":3,"toward_id":2}"#,
                Command::OfferAlliance {
                    country: CountryId::new(3),
                    toward: CountryId::new(2),
                },
            ),
            (
                r#"{"type":"accept_alliance","country_id":2,"from_id":3}"#,
                Command::AcceptAlliance {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                },
            ),
            (
                r#"{"type":"reject_alliance","country_id":2,"from_id":3}"#,
                Command::RejectAlliance {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                },
            ),
            (
                r#"{"type":"break_alliance","country_id":3,"other_id":2}"#,
                Command::BreakAlliance {
                    country: CountryId::new(3),
                    other: CountryId::new(2),
                },
            ),
        ];
        for (json, expected) in cases {
            assert_eq!(
                serde_json::from_str::<UiCommand>(json)
                    .unwrap()
                    .into_sim()
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn royal_marriage_ui_commands_preserve_country_and_character_ids() {
        let cases = [
            (
                r#"{"type":"offer_royal_marriage","country_id":3,"character_id":31,"toward_id":2,"partner_id":22}"#,
                Command::OfferRoyalMarriage {
                    country: CountryId::new(3),
                    character: strategy_sim::CharacterId::new(31),
                    toward: CountryId::new(2),
                    partner: strategy_sim::CharacterId::new(22),
                },
            ),
            (
                r#"{"type":"accept_royal_marriage","country_id":2,"from_id":3,"character_id":22,"partner_id":31}"#,
                Command::AcceptRoyalMarriage {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                    character: strategy_sim::CharacterId::new(22),
                    partner: strategy_sim::CharacterId::new(31),
                },
            ),
            (
                r#"{"type":"reject_royal_marriage","country_id":2,"from_id":3,"character_id":22,"partner_id":31}"#,
                Command::RejectRoyalMarriage {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                    character: strategy_sim::CharacterId::new(22),
                    partner: strategy_sim::CharacterId::new(31),
                },
            ),
        ];
        for (json, expected) in cases {
            assert_eq!(
                serde_json::from_str::<UiCommand>(json)
                    .unwrap()
                    .into_sim()
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn vassal_ui_commands_preserve_direction_and_country_ids() {
        let cases = [
            (
                r#"{"type":"offer_vassalage","country_id":3,"toward_id":2}"#,
                Command::OfferVassalage {
                    country: CountryId::new(3),
                    toward: CountryId::new(2),
                },
            ),
            (
                r#"{"type":"accept_vassalage","country_id":2,"from_id":3}"#,
                Command::AcceptVassalage {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                },
            ),
            (
                r#"{"type":"reject_vassalage","country_id":2,"from_id":3}"#,
                Command::RejectVassalage {
                    country: CountryId::new(2),
                    from: CountryId::new(3),
                },
            ),
            (
                r#"{"type":"release_vassal","country_id":3,"subject_id":2}"#,
                Command::ReleaseVassal {
                    country: CountryId::new(3),
                    subject: CountryId::new(2),
                },
            ),
            (
                r#"{"type":"declare_independence","country_id":2}"#,
                Command::DeclareIndependence {
                    country: CountryId::new(2),
                },
            ),
        ];
        for (json, expected) in cases {
            let actual = serde_json::from_str::<UiCommand>(json)
                .unwrap()
                .into_sim()
                .unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn normal_launch_and_guest_startup_arguments_are_validated() {
        assert!(Startup::parse(std::iter::empty()).unwrap().is_none());
        let args = ["--lan-join", "127.0.0.1:47620", "Guest"];
        assert!(matches!(
            Startup::parse(args.map(String::from).into_iter()).unwrap(),
            Some(Startup::Guest { address, name }) if address == "127.0.0.1:47620" && name == "Guest"
        ));
        for args in [
            vec!["--lan-join"],
            vec!["--lan-join", "127.0.0.1:47620"],
            vec!["--lan-join", "127.0.0.1:47620", "Guest", "extra"],
            vec!["--unknown"],
            vec!["--lan-host-config"],
        ] {
            assert!(Startup::parse(args.into_iter().map(String::from)).is_err());
        }
    }

    #[test]
    fn pending_event_projection_is_local_and_choice_maps_to_host_command() {
        let mut definition = parse_event_yaml(include_str!(
            "../../tools/tests/fixtures/good_event/events/harvest.yaml"
        ))
        .unwrap();
        definition.trigger = Trigger::All(Vec::new());
        definition.mean_time_to_happen = NonZeroU64::new(1);
        let graph = ProvinceGraph::try_new(
            MapIdentity {
                version: NonZeroU32::new(1).unwrap(),
                sha256: [7; 32],
            },
            [],
        )
        .unwrap();
        let countries = [HOST, GUEST]
            .into_iter()
            .enumerate()
            .map(|(index, player)| {
                (
                    CountryId::new(index as u32 + 1),
                    CountryState::new(
                        Controller::Human(player),
                        BTreeSet::new(),
                        false,
                        ModifierStore::default(),
                    ),
                )
            })
            .collect();
        let mut game = GameState::new(
            SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
            42,
            graph,
            countries,
            [],
        )
        .unwrap()
        .with_events(BTreeMap::from([(definition.key.clone(), definition)]))
        .unwrap();
        game.tick(|_, _| Vec::new()).unwrap();
        let host_country = game
            .countries()
            .find_map(|(id, country)| {
                (country.controller() == &Controller::Human(HOST)).then_some(id.get())
            })
            .unwrap();
        let guest_country = game
            .countries()
            .find_map(|(id, country)| {
                (country.controller() == &Controller::Human(GUEST)).then_some(id.get())
            })
            .unwrap();
        let host_events = event_views(&game, HOST);
        let guest_events = event_views(&game, GUEST);
        assert_eq!(host_events.len(), 1);
        assert_eq!(guest_events.len(), 1);
        assert_eq!(host_events[0].country_id, host_country);
        assert_eq!(guest_events[0].country_id, guest_country);
        assert_eq!(host_events[0].key, "harvest_festival");
        assert_eq!(host_events[0].options.len(), 2);
        let command = UiCommand::ChooseEvent {
            country_id: host_country,
            key: host_events[0].key.clone(),
            option: 1,
        }
        .into_sim()
        .unwrap();
        assert!(
            matches!(command, Command::ChooseEvent { country, option: 1, .. }
            if country.get() == host_country)
        );
    }
}
