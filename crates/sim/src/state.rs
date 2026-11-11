use std::collections::{BTreeMap, BTreeSet};

use crate::modifiers::effective_value_from;
use crate::war::{MilitaryAction, MilitaryBonuses, MilitaryOrder};
use crate::{
    Army, ArmyId, MilitaryTickFailure, MovementLeg, Siege, War, WarError, WarId, WarState,
};
use crate::{Fleet, FleetId, FleetMovement, NavyError, NavyState, ProvinceEdge};

use crate::{
    CharacterId, CharacterState, CountryId, CountryPolity, Date, DiplomacyError, DiplomacyState,
    EconomyState, EdgeKind, Effect, EffectError, EventDefinition, EventError, EventState,
    FiscalError, FiscalPolicy, Fixed, MapIdentity, MarriageError, ModKey, ModifierInsertError,
    ModifierSourceId, ModifierStore, Opinion, OpinionStateError, PlayerId, PolityError,
    PolityState, PolityTickFailure, PreparedModifier, ProvinceGraph, ProvinceId, SeededRng, Sex,
    SimTime, Speed, SuccessionState, Tick, TimeActor, TimeCommand, TimeError, TriggerView,
};

/// Reference to a scenario's data-driven AI profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiProfileKey(String);

impl AiProfileKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Controller {
    Human(PlayerId),
    Ai(AiProfileKey),
}

/// The role and identity assigned by the authoritative caller.
///
/// This metadata comes from the host session or AI dispatch, separate from
/// the command a player or bot supplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandActor {
    Host(PlayerId),
    Guest(PlayerId),
    Ai(CountryId),
}

impl CommandActor {
    fn time_actor(self) -> TimeActor {
        match self {
            Self::Host(_) => TimeActor::Host,
            Self::Guest(_) => TimeActor::Guest,
            Self::Ai(_) => TimeActor::Ai,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Time(TimeCommand),
    SetController {
        country: CountryId,
        controller: Controller,
    },
    SetFiscalPolicy {
        country: CountryId,
        policy: FiscalPolicy,
    },
    NominateHeir {
        country: CountryId,
        heir: Option<CharacterId>,
    },
    RaiseArmy {
        country: CountryId,
        province: ProvinceId,
        soldiers: u32,
    },
    MergeArmies {
        country: CountryId,
        into: ArmyId,
        from: ArmyId,
    },
    SplitArmy {
        country: CountryId,
        army: ArmyId,
        soldiers: u32,
    },
    MoveArmy {
        country: CountryId,
        army: ArmyId,
        destination: ProvinceId,
    },
    MoveFleet {
        country: CountryId,
        fleet: FleetId,
        destination: ProvinceId,
    },
    EmbarkArmy {
        country: CountryId,
        army: ArmyId,
        fleet: FleetId,
    },
    DisembarkArmy {
        country: CountryId,
        fleet: FleetId,
        destination: ProvinceId,
    },
    RetreatArmy {
        country: CountryId,
        army: ArmyId,
        destination: ProvinceId,
    },
    DeclareWar {
        country: CountryId,
        defender: CountryId,
        goal: ProvinceId,
    },
    OfferPeace {
        country: CountryId,
        war: WarId,
        cede_goal: bool,
    },
    AcceptPeace {
        country: CountryId,
        war: WarId,
    },
    OfferAlliance {
        country: CountryId,
        toward: CountryId,
    },
    AcceptAlliance {
        country: CountryId,
        from: CountryId,
    },
    RejectAlliance {
        country: CountryId,
        from: CountryId,
    },
    BreakAlliance {
        country: CountryId,
        other: CountryId,
    },
    ChooseEvent {
        country: CountryId,
        key: String,
        option: u32,
    },
    OfferRoyalMarriage {
        country: CountryId,
        character: CharacterId,
        toward: CountryId,
        partner: CharacterId,
    },
    AcceptRoyalMarriage {
        country: CountryId,
        from: CountryId,
        character: CharacterId,
        partner: CharacterId,
    },
    RejectRoyalMarriage {
        country: CountryId,
        from: CountryId,
        character: CharacterId,
        partner: CharacterId,
    },
    Guarantee {
        country: CountryId,
        protected: CountryId,
    },
    RevokeGuarantee {
        country: CountryId,
        protected: CountryId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    Time(TimeError),
    HostRequired,
    UnknownCountry(CountryId),
    PolityUnavailable,
    NotCountryController(CountryId),
    UnknownAiProfile(CountryId),
    Fiscal(FiscalError),
    War(WarError),
    Navy(NavyError),
    Diplomacy(DiplomacyError),
    Event(EventError),
    Marriage(MarriageError),
    InvalidHeir {
        country: CountryId,
        heir: CharacterId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateChange {
    ClockAdvanced {
        tick: Tick,
        date: Date,
    },
    PauseChanged(bool),
    SpeedChanged(Speed),
    ControllerChanged {
        country: CountryId,
        controller: Controller,
    },
    ModifierSourceActivated {
        country: CountryId,
        grant: PreparedModifier,
    },
    ModifierSourceExpired {
        country: CountryId,
        source: ModifierSourceId,
    },
    FiscalPolicyChanged {
        country: CountryId,
        policy: FiscalPolicy,
    },
    EconomyChanged {
        country: CountryId,
        economy: EconomyState,
    },
    CharacterDied {
        character: CharacterId,
        country: CountryId,
    },
    SuccessionChanged {
        country: CountryId,
        succession: SuccessionState,
        regency: bool,
    },
    ArmyChanged {
        army: ArmyId,
        state: Option<Army>,
    },
    FleetChanged {
        fleet: FleetId,
        state: Option<Fleet>,
    },
    NavalBattleResolved(crate::NavalBattle),
    WarChanged {
        war: WarId,
        state: Option<War>,
    },
    SiegeChanged {
        province: ProvinceId,
        state: Option<Siege>,
    },
    BattleResolved {
        war: WarId,
        province: ProvinceId,
        winner: CountryId,
        loser: CountryId,
        winner_casualties: u64,
        loser_casualties: u64,
    },
    ProvinceOwnershipChanged {
        province: ProvinceId,
        owner: CountryId,
    },
    PeaceConcluded {
        war: WarId,
        attacker: CountryId,
        defender: CountryId,
        ceded: Option<ProvinceId>,
    },
    AllianceOffered {
        from: CountryId,
        toward: CountryId,
        expires_at: Date,
    },
    AllianceOfferRemoved {
        from: CountryId,
        toward: CountryId,
    },
    AllianceChanged {
        a: CountryId,
        b: CountryId,
        active: bool,
    },
    MarriageOffered {
        from: CharacterId,
        toward: CharacterId,
        expires_at: Date,
    },
    MarriageOfferRemoved {
        from: CharacterId,
        toward: CharacterId,
    },
    MarriageChanged {
        a: CharacterId,
        b: CharacterId,
        active: bool,
    },
    GuaranteeChanged {
        guarantor: CountryId,
        protected: CountryId,
        active: bool,
    },
    OpinionChanged {
        from: CountryId,
        toward: CountryId,
        opinion: Opinion,
    },
    EventFired {
        country: CountryId,
        key: String,
    },
    EventResolved {
        country: CountryId,
        key: String,
        option: u32,
    },
}

/// The ordered state changes caused by an accepted command or tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delta {
    changes: Vec<StateChange>,
}

impl Delta {
    pub fn changes(&self) -> &[StateChange] {
        &self.changes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiCommandRejection {
    pub country: CountryId,
    pub command: Command,
    pub error: CommandError,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub delta: Delta,
    pub ai_rejections: Vec<AiCommandRejection>,
    pub scheduled_failures: Vec<ScheduledEffectFailure>,
    pub polity_failures: Vec<PolityTickFailure>,
    pub military_failures: Vec<MilitaryTickFailure>,
}

/// A trusted scenario input for an effect due on an exact simulation tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledEffect {
    pub due: Tick,
    pub country: CountryId,
    pub source: ModifierSourceId,
    pub effect: Effect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleError {
    ExpiredActiveSource {
        country: CountryId,
        source: ModifierSourceId,
        expires_at: u64,
        current: Tick,
    },
    NotFuture {
        due: Tick,
        current: Tick,
    },
    UnknownCountry(CountryId),
    Duplicate {
        due: Tick,
        country: CountryId,
        source: ModifierSourceId,
    },
    InvalidEffect {
        due: Tick,
        country: CountryId,
        source: ModifierSourceId,
        error: EffectError,
    },
}

/// A due effect that could not be activated and was consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledEffectFailure {
    pub tick: Tick,
    pub country: CountryId,
    pub source: ModifierSourceId,
    pub error: ModifierInsertError,
}

/// Country facts loaded from scenario data and held by the simulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountryState {
    controller: Controller,
    known_technologies: BTreeSet<String>,
    at_war: bool,
    modifiers: ModifierStore,
}

impl CountryState {
    pub fn new(
        controller: Controller,
        known_technologies: BTreeSet<String>,
        at_war: bool,
        modifiers: ModifierStore,
    ) -> Self {
        Self {
            controller,
            known_technologies,
            at_war,
            modifiers,
        }
    }

    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    pub fn known_technologies(&self) -> impl Iterator<Item = &str> {
        self.known_technologies.iter().map(String::as_str)
    }

    pub const fn modifiers(&self) -> &ModifierStore {
        &self.modifiers
    }

    pub fn effective_value(&self, base: Fixed, key: &ModKey) -> Option<Fixed> {
        self.modifiers.effective_value(base, key)
    }
}

impl TriggerView for CountryState {
    fn has_technology(&self, key: &str) -> bool {
        self.known_technologies.contains(key)
    }

    fn is_at_war(&self) -> bool {
        self.at_war
    }
}

type ModifierSchedule = BTreeMap<Tick, BTreeMap<(CountryId, ModifierSourceId), PreparedModifier>>;

/// Authoritative simulation state. Runtime writes happen in apply or tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    time: SimTime,
    seed: u64,
    rng: SeededRng,
    province_graph: ProvinceGraph,
    countries: BTreeMap<CountryId, CountryState>,
    scheduled: ModifierSchedule,
    opinions: BTreeMap<(CountryId, CountryId), Opinion>,
    diplomacy: Option<DiplomacyState>,
    events: Option<EventState>,
    polity: Option<PolityState>,
    war_state: Option<WarState>,
    navy: Option<NavyState>,
}

enum PreparedCommand<'a> {
    Time(SimTime),
    SetController(CountryId, &'a Controller),
    SetFiscalPolicy(CountryId, FiscalPolicy),
    NominateHeir(CountryId, Option<CharacterId>),
    Military(MilitaryAction, Vec<(CountryId, CountryId)>),
    MoveFleet(FleetId, Vec<ProvinceEdge>),
    Embark(ArmyId, FleetId),
    Disembark(ArmyId, FleetId, ProvinceId),
    Diplomacy(DiplomacyAction),
    ChooseEvent(
        CountryId,
        &'a str,
        u32,
        Vec<(ModifierSourceId, PreparedModifier)>,
    ),
    Marriage(MarriageAction),
}

enum DiplomacyAction {
    Offer(CountryId, CountryId, Date),
    Accept(CountryId, CountryId),
    Reject(CountryId, CountryId),
    Break(CountryId, CountryId, Date),
    Guarantee(CountryId, CountryId),
    RevokeGuarantee(CountryId, CountryId),
}

enum MarriageAction {
    Offer(CharacterId, CharacterId, Date),
    Accept(CharacterId, CharacterId),
    Reject(CharacterId, CharacterId),
}

impl GameState {
    /// Initializes state after validating active expiries and scheduled effects.
    pub fn new(
        time: SimTime,
        seed: u64,
        province_graph: ProvinceGraph,
        countries: BTreeMap<CountryId, CountryState>,
        schedule: impl IntoIterator<Item = ScheduledEffect>,
    ) -> Result<Self, ScheduleError> {
        let current = time.tick();
        for (&country, state) in &countries {
            for (source, expires_at, _) in state.modifiers().sources() {
                if let Some(expires_at) = expires_at.filter(|&expiry| expiry <= current.get()) {
                    return Err(ScheduleError::ExpiredActiveSource {
                        country,
                        source,
                        expires_at,
                        current,
                    });
                }
            }
        }

        let mut scheduled = ModifierSchedule::new();
        for ScheduledEffect {
            due,
            country,
            source,
            effect,
        } in schedule
        {
            if due <= current {
                return Err(ScheduleError::NotFuture { due, current });
            }
            if !countries.contains_key(&country) {
                return Err(ScheduleError::UnknownCountry(country));
            }
            let entries = scheduled.entry(due).or_default();
            let std::collections::btree_map::Entry::Vacant(entry) =
                entries.entry((country, source))
            else {
                return Err(ScheduleError::Duplicate {
                    due,
                    country,
                    source,
                });
            };
            let grant = effect.prepare(source, due.get()).map_err(|error| {
                ScheduleError::InvalidEffect {
                    due,
                    country,
                    source,
                    error,
                }
            })?;
            entry.insert(grant);
        }
        Ok(Self {
            time,
            seed,
            rng: SeededRng::new(seed),
            province_graph,
            countries,
            scheduled,
            opinions: BTreeMap::new(),
            diplomacy: None,
            events: None,
            polity: None,
            war_state: None,
            navy: None,
        })
    }

    /// Restores authoritative state after its save header and map identity are checked.
    pub fn restore(
        time: SimTime,
        seed: u64,
        rng: SeededRng,
        province_graph: ProvinceGraph,
        countries: BTreeMap<CountryId, CountryState>,
        schedule: impl IntoIterator<Item = ScheduledEffect>,
    ) -> Result<Self, ScheduleError> {
        let mut state = Self::new(time, seed, province_graph, countries, schedule)?;
        state.rng = rng;
        Ok(state)
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng.state()
    }

    /// Iterates pending grants by due tick, then by country and source ID.
    pub fn pending_effects(&self) -> impl Iterator<Item = (Tick, CountryId, &PreparedModifier)> {
        self.scheduled.iter().flat_map(|(&due, grants)| {
            grants
                .iter()
                .map(move |(&(country, _), grant)| (due, country, grant))
        })
    }

    /// Attaches validated country and ruler data during scenario setup or restore.
    pub fn with_polity(mut self, polity: PolityState) -> Result<Self, PolityError> {
        if self.polity.is_some() {
            return Err(PolityError::AlreadyConfigured);
        }
        polity.validate(&self.province_graph, &self.countries, self.time.date())?;
        self.polity = Some(polity);
        Ok(self)
    }

    /// Attaches initial directional opinions after all countries are known.
    pub fn with_opinions(
        mut self,
        opinions: impl IntoIterator<Item = (CountryId, CountryId, Opinion)>,
    ) -> Result<Self, OpinionStateError> {
        let mut resolved = BTreeMap::new();
        for (from, toward, opinion) in opinions {
            for country in [from, toward] {
                if !self.countries.contains_key(&country) {
                    return Err(OpinionStateError::UnknownCountry(country));
                }
            }
            if from == toward {
                return Err(OpinionStateError::SameCountry(from));
            }
            if resolved.insert((from, toward), opinion).is_some() {
                return Err(OpinionStateError::Duplicate { from, toward });
            }
        }
        self.opinions = resolved;
        Ok(self)
    }

    /// Returns a country's opinion toward another country. Missing entries are neutral.
    /// This read-only query is shared by UI previews and host-side AI decisions.
    pub fn opinion(&self, from: CountryId, toward: CountryId) -> Option<Opinion> {
        (from != toward
            && self.countries.contains_key(&from)
            && self.countries.contains_key(&toward))
        .then(|| {
            self.opinions
                .get(&(from, toward))
                .copied()
                .unwrap_or(Opinion::NEUTRAL)
        })
    }

    /// Base opinion plus a temporary royal-marriage bonus, clamped to the opinion range.
    pub fn effective_opinion(&self, from: CountryId, toward: CountryId) -> Option<Opinion> {
        let base = self.opinion(from, toward)?;
        let married = self.polity.as_ref().is_some_and(|polity| {
            polity.characters.values().any(|character| {
                character.country == from
                    && character.spouse.is_some_and(|spouse| {
                        polity
                            .characters
                            .get(&spouse)
                            .is_some_and(|partner| partner.country == toward)
                    })
            })
        });
        Some(if married { base.adjusted(20) } else { base })
    }

    /// Shared eligibility result for host validation, AI and UI previews.
    pub fn marriage_eligibility(
        &self,
        from: CountryId,
        character: CharacterId,
        toward: CountryId,
        partner: CharacterId,
    ) -> Result<(), MarriageError> {
        let polity = self.polity.as_ref().ok_or(MarriageError::InvalidState)?;
        if from == toward {
            return Err(MarriageError::SameCountry);
        }
        let a = polity
            .characters
            .get(&character)
            .ok_or(MarriageError::UnknownCharacter(character))?;
        let b = polity
            .characters
            .get(&partner)
            .ok_or(MarriageError::UnknownCharacter(partner))?;
        if a.country != from {
            return Err(MarriageError::WrongCountry(character));
        }
        if b.country != toward {
            return Err(MarriageError::WrongCountry(partner));
        }
        let now = self.time.date();
        for (id, person) in [(character, a), (partner, b)] {
            if person.name.trim().is_empty() {
                return Err(MarriageError::Unnamed(id));
            }
            if !person.is_alive_on(now) {
                return Err(MarriageError::NotAlive(id));
            }
            if person
                .age_years(now)
                .is_none_or(|age| age < u32::from(polity.rules.adulthood_years))
            {
                return Err(MarriageError::Underage(id));
            }
            if person.spouse.is_some() {
                return Err(MarriageError::AlreadyMarried(id));
            }
        }
        if a.sex == Sex::Unknown {
            return Err(MarriageError::UnknownSex(character));
        }
        if b.sex == Sex::Unknown {
            return Err(MarriageError::UnknownSex(partner));
        }
        if a.sex == b.sex {
            return Err(MarriageError::SameSex);
        }
        let is_dynastic =
            [(from, character), (toward, partner)]
                .into_iter()
                .any(|(country, id)| {
                    polity.countries.get(&country).is_some_and(|nation| {
                        nation.succession.ruler == Some(id) || nation.succession.heir == Some(id)
                    })
                });
        if !is_dynastic {
            return Err(MarriageError::NoRulerOrHeir);
        }
        Ok(())
    }

    pub fn opinions(&self) -> impl Iterator<Item = (CountryId, CountryId, Opinion)> + '_ {
        self.opinions
            .iter()
            .map(|(&(from, toward), &opinion)| (from, toward, opinion))
    }

    pub fn with_diplomacy(mut self, diplomacy: DiplomacyState) -> Result<Self, DiplomacyError> {
        diplomacy.validate(self.countries.keys().copied(), self.time.date())?;
        if diplomacy
            .alliances
            .iter()
            .any(|&(a, b)| self.directly_at_war(a, b))
        {
            return Err(DiplomacyError::DirectWar);
        }
        if !diplomacy.marriage_offers.is_empty() {
            let Some(polity) = &self.polity else {
                return Err(DiplomacyError::InvalidState);
            };
            for (&(from, toward), &expires) in &diplomacy.marriage_offers {
                let (Some(a), Some(b)) =
                    (polity.characters.get(&from), polity.characters.get(&toward))
                else {
                    return Err(DiplomacyError::InvalidState);
                };
                let eligibility = self.marriage_eligibility(a.country, from, b.country, toward);
                if expires <= self.time.date()
                    || !matches!(eligibility, Ok(()) | Err(MarriageError::NoRulerOrHeir))
                {
                    return Err(DiplomacyError::InvalidState);
                }
            }
        }
        self.diplomacy = Some(diplomacy);
        Ok(self)
    }

    pub fn diplomacy(&self) -> Option<&DiplomacyState> {
        self.diplomacy.as_ref()
    }

    /// Copies validated YAML definitions into the authoritative game snapshot.
    pub fn with_events(
        mut self,
        definitions: BTreeMap<String, EventDefinition>,
    ) -> Result<Self, EventError> {
        if self.events.is_some() {
            return Err(EventError::AlreadyConfigured);
        }
        let highest_source = self.highest_modifier_source();
        let next = highest_source
            .checked_add(1)
            .ok_or(EventError::ModifierSourceExhausted)?;
        self.events = Some(EventState::new(definitions, next)?);
        Ok(self)
    }

    /// Restores an event snapshot after the save decoder checks its encoding.
    pub fn with_event_state(mut self, events: EventState) -> Result<Self, EventError> {
        if self.events.is_some() {
            return Err(EventError::AlreadyConfigured);
        }
        events.validate_saved(
            self.time.tick(),
            self.countries.keys().copied(),
            self.highest_modifier_source(),
        )?;
        self.events = Some(events);
        Ok(self)
    }

    pub fn events(&self) -> Option<&EventState> {
        self.events.as_ref()
    }

    fn highest_modifier_source(&self) -> u32 {
        self.countries
            .values()
            .flat_map(|country| {
                country
                    .modifiers()
                    .sources()
                    .map(|(source, _, _)| source.get())
            })
            .chain(
                self.scheduled
                    .values()
                    .flat_map(|entries| entries.keys().map(|(_, source)| source.get())),
            )
            .max()
            .unwrap_or(0)
    }

    pub fn allied(&self, a: CountryId, b: CountryId) -> bool {
        self.diplomacy
            .as_ref()
            .is_some_and(|state| state.allied(a, b))
    }

    pub fn directly_at_war(&self, a: CountryId, b: CountryId) -> bool {
        self.war_state
            .as_ref()
            .is_some_and(|state| state.wars.values().any(|war| war.opposed(a, b)))
    }

    /// A third country with land borders to both parties and more active soldiers
    /// than each individually is a shared threat for AI diplomacy.
    pub fn common_stronger_neighbor(&self, a: CountryId, b: CountryId) -> bool {
        let (Some(polity), Some(war)) = (&self.polity, &self.war_state) else {
            return false;
        };
        let soldiers = |id| {
            war.armies
                .values()
                .filter(|army| army.country == id)
                .fold(0_u64, |total, army| {
                    total.saturating_add(u64::from(army.soldiers))
                })
        };
        let a_strength = soldiers(a);
        let b_strength = soldiers(b);
        polity
            .countries
            .keys()
            .copied()
            .filter(|&third| third != a && third != b)
            .any(|third| {
                let strength = soldiers(third);
                strength > a_strength
                    && strength > b_strength
                    && self.land_border(a, third)
                    && self.land_border(b, third)
            })
    }

    fn land_border(&self, a: CountryId, b: CountryId) -> bool {
        let (Some(a_provinces), Some(b_provinces)) =
            (self.country_polity(a), self.country_polity(b))
        else {
            return false;
        };
        a_provinces.provinces.iter().any(|&province| {
            self.province_graph
                .neighbors(province)
                .is_some_and(|edges| {
                    edges.iter().any(|edge| {
                        matches!(edge.kind, EdgeKind::Normal | EdgeKind::River)
                            && b_provinces.provinces.contains(&edge.to)
                    })
                })
        })
    }

    /// Attaches validated military scenario or save data and derives war flags.
    pub fn with_war_state(mut self, war_state: WarState) -> Result<Self, WarError> {
        if self.war_state.is_some() {
            return Err(WarError::AlreadyConfigured);
        }
        let polity = self.polity().ok_or(WarError::Unavailable)?;
        war_state.validate(polity, &self.province_graph, self.time.tick())?;
        if self.diplomacy.as_ref().is_some_and(|diplomacy| {
            war_state
                .wars
                .values()
                .any(|war| diplomacy.alliances.iter().any(|&(a, b)| war.opposed(a, b)))
        }) {
            return Err(WarError::AllianceActive);
        }
        self.war_state = Some(war_state);
        self.sync_war_flags();
        Ok(self)
    }

    pub const fn war_state(&self) -> Option<&WarState> {
        self.war_state.as_ref()
    }

    /// Attaches scenario or save fleets after military armies have been loaded.
    pub fn with_navy(mut self, navy: NavyState) -> Result<Self, NavyError> {
        if self.navy.is_some() {
            return Err(NavyError::AlreadyConfigured);
        }
        let war = self.war_state().ok_or(NavyError::Unavailable)?;
        navy.validate(
            &self.province_graph,
            self.countries.keys().copied(),
            war.armies
                .iter()
                .map(|(&id, army)| (id, army.country, army.soldiers, army.movement.is_some())),
        )?;
        self.navy = Some(navy);
        Ok(self)
    }

    pub const fn navy(&self) -> Option<&NavyState> {
        self.navy.as_ref()
    }

    pub fn country_at_war(&self, country: CountryId) -> bool {
        self.country(country).is_some_and(TriggerView::is_at_war)
    }

    pub fn find_army_path(
        &self,
        country: CountryId,
        from: ProvinceId,
        to: ProvinceId,
    ) -> Option<Vec<MovementLeg>> {
        self.war_state()?
            .find_path(&self.province_graph, self.polity()?, country, from, to)
    }

    pub const fn polity(&self) -> Option<&PolityState> {
        self.polity.as_ref()
    }

    pub fn country_polity(&self, country: CountryId) -> Option<&CountryPolity> {
        self.polity()?.countries.get(&country)
    }

    pub fn character(&self, character: CharacterId) -> Option<&CharacterState> {
        self.polity()?.characters.get(&character)
    }

    pub fn ruler(&self, country: CountryId) -> Option<(CharacterId, &CharacterState)> {
        let ruler = self.country_polity(country)?.succession.ruler?;
        Some((ruler, self.character(ruler)?))
    }

    pub fn owner_of_province(&self, province: ProvinceId) -> Option<CountryId> {
        self.polity()?
            .countries
            .iter()
            .find_map(|(&country, state)| state.provinces.contains(&province).then_some(country))
    }

    pub fn is_in_regency(&self, country: CountryId) -> Option<bool> {
        self.polity()?.is_in_regency(country, self.time.date())
    }

    /// Returns living local candidates other than the ruler, oldest first.
    /// Ties use ascending character ID.
    pub fn eligible_heirs(&self, country: CountryId) -> Vec<CharacterId> {
        self.polity().map_or_else(Vec::new, |polity| {
            polity.eligible_heirs(country, self.time.date())
        })
    }

    /// Projects a month using the current date, modifiers and eligible ruler.
    /// This read-only forecast uses the same arithmetic as the monthly tick.
    pub fn preview_monthly_economy(
        &self,
        country: CountryId,
        policy: FiscalPolicy,
    ) -> Result<EconomyState, CommandError> {
        let base = self
            .country(country)
            .ok_or(CommandError::UnknownCountry(country))?;
        let polity = self.polity().ok_or(CommandError::PolityUnavailable)?;
        let state = &polity.countries[&country];
        polity
            .forecast(
                country,
                state,
                self.time.date(),
                &self.province_graph,
                base.modifiers(),
                policy,
            )
            .map_err(CommandError::Fiscal)
    }

    pub const fn time(&self) -> &SimTime {
        &self.time
    }

    /// Original scenario seed, for deriving independent AI streams.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    pub const fn province_graph(&self) -> &ProvinceGraph {
        &self.province_graph
    }

    pub const fn map_identity(&self) -> MapIdentity {
        self.province_graph.identity()
    }

    pub fn country(&self, country: CountryId) -> Option<&CountryState> {
        self.countries.get(&country)
    }

    /// Iterates countries in ascending ID order.
    pub fn countries(&self) -> impl Iterator<Item = (CountryId, &CountryState)> {
        self.countries.iter().map(|(&id, state)| (id, state))
    }

    /// Checks whether the command would be accepted without changing the state.
    pub fn validate_command(
        &self,
        actor: CommandActor,
        command: &Command,
    ) -> Result<(), CommandError> {
        self.prepare_command(actor, command).map(|_| ())
    }

    /// Applies an accepted command and returns its changes. Rejections are atomic.
    pub fn apply(&mut self, actor: CommandActor, command: &Command) -> Result<Delta, CommandError> {
        let mut delta = Delta::default();
        match self.prepare_command(actor, command)? {
            PreparedCommand::Time(time) => {
                if self.time.is_paused() != time.is_paused() {
                    delta
                        .changes
                        .push(StateChange::PauseChanged(time.is_paused()));
                }
                if self.time.speed() != time.speed() {
                    delta.changes.push(StateChange::SpeedChanged(time.speed()));
                }
                self.time = time;
            }
            PreparedCommand::SetController(country, controller) => {
                let state = self
                    .countries
                    .get_mut(&country)
                    .expect("country exists after validation");
                if state.controller() != controller {
                    state.controller = controller.clone();
                    delta.changes.push(StateChange::ControllerChanged {
                        country,
                        controller: controller.clone(),
                    });
                }
            }
            PreparedCommand::SetFiscalPolicy(country, policy) => {
                let state = self
                    .polity
                    .as_mut()
                    .expect("polity validated")
                    .countries
                    .get_mut(&country)
                    .expect("country validated");
                if state.economy.policy != policy {
                    state.economy.policy = policy;
                    delta
                        .changes
                        .push(StateChange::FiscalPolicyChanged { country, policy });
                }
            }
            PreparedCommand::NominateHeir(country, heir) => {
                let regency = self.is_in_regency(country).expect("polity validated");
                let state = self
                    .polity
                    .as_mut()
                    .expect("polity validated")
                    .countries
                    .get_mut(&country)
                    .expect("country validated");
                if state.succession.heir != heir {
                    state.succession.heir = heir;
                    delta.changes.push(StateChange::SuccessionChanged {
                        country,
                        succession: state.succession,
                        regency,
                    });
                }
            }
            PreparedCommand::Military(action, failed_guarantees) => {
                let war_state = self.war_state.as_mut().expect("military state validated");
                let polity = self.polity.as_mut().expect("polity validated");
                delta.changes.extend(war_state.apply(polity, action));
                if !failed_guarantees.is_empty() {
                    let diplomacy = self.diplomacy.as_mut().expect("guarantees validated");
                    for (guarantor, protected) in failed_guarantees {
                        diplomacy.guarantees.remove(&(guarantor, protected));
                        delta.changes.push(StateChange::GuaranteeChanged {
                            guarantor,
                            protected,
                            active: false,
                        });
                        let opinion = self
                            .opinions
                            .get(&(protected, guarantor))
                            .copied()
                            .unwrap_or(Opinion::NEUTRAL)
                            .adjusted(-25);
                        self.opinions.insert((protected, guarantor), opinion);
                        delta.changes.push(StateChange::OpinionChanged {
                            from: protected,
                            toward: guarantor,
                            opinion,
                        });
                    }
                }
                self.sync_war_flags();
            }
            PreparedCommand::MoveFleet(id, route) => {
                let navy = self.navy.as_mut().expect("navy state validated");
                let fleet = navy.fleets.get_mut(&id).expect("fleet validated");
                fleet.movement = Some(FleetMovement {
                    route,
                    elapsed_ticks: 0,
                });
                delta.changes.push(StateChange::FleetChanged {
                    fleet: id,
                    state: Some(fleet.clone()),
                });
            }
            PreparedCommand::Embark(army_id, fleet_id) => {
                let navy = self.navy.as_mut().expect("navy state validated");
                let fleet = navy.fleets.get_mut(&fleet_id).expect("fleet validated");
                fleet.cargo = Some(army_id);
                delta.changes.push(StateChange::FleetChanged {
                    fleet: fleet_id,
                    state: Some(fleet.clone()),
                });
            }
            PreparedCommand::Disembark(army_id, fleet_id, destination) => {
                let navy = self.navy.as_mut().expect("navy state validated");
                let fleet = navy.fleets.get_mut(&fleet_id).expect("fleet validated");
                fleet.cargo = None;
                delta.changes.push(StateChange::FleetChanged {
                    fleet: fleet_id,
                    state: Some(fleet.clone()),
                });
                let army = self
                    .war_state
                    .as_mut()
                    .expect("war state validated")
                    .armies
                    .get_mut(&army_id)
                    .expect("cargo army validated");
                army.province = destination;
                delta.changes.push(StateChange::ArmyChanged {
                    army: army_id,
                    state: Some(army.clone()),
                });
            }
            PreparedCommand::Diplomacy(action) => {
                let diplomacy = self.diplomacy.get_or_insert_with(DiplomacyState::default);
                match action {
                    DiplomacyAction::Offer(from, toward, expires_at) => {
                        diplomacy.offers.insert((from, toward), expires_at);
                        delta.changes.push(StateChange::AllianceOffered {
                            from,
                            toward,
                            expires_at,
                        });
                    }
                    DiplomacyAction::Accept(from, toward) => {
                        diplomacy.offers.remove(&(from, toward));
                        diplomacy.offers.remove(&(toward, from));
                        let (a, b) = crate::diplomacy::pair(from, toward);
                        diplomacy.alliances.insert((a, b));
                        delta
                            .changes
                            .push(StateChange::AllianceOfferRemoved { from, toward });
                        delta
                            .changes
                            .push(StateChange::AllianceChanged { a, b, active: true });
                    }
                    DiplomacyAction::Reject(from, toward) => {
                        diplomacy.offers.remove(&(from, toward));
                        delta
                            .changes
                            .push(StateChange::AllianceOfferRemoved { from, toward });
                    }
                    DiplomacyAction::Break(a, b, until) => {
                        let pair = crate::diplomacy::pair(a, b);
                        diplomacy.alliances.remove(&pair);
                        diplomacy.cooldowns.insert(pair, until);
                        delta.changes.push(StateChange::AllianceChanged {
                            a: pair.0,
                            b: pair.1,
                            active: false,
                        });
                        for (from, toward) in [(a, b), (b, a)] {
                            let opinion = self
                                .opinions
                                .get(&(from, toward))
                                .copied()
                                .unwrap_or(Opinion::NEUTRAL)
                                .adjusted(-25);
                            self.opinions.insert((from, toward), opinion);
                            delta.changes.push(StateChange::OpinionChanged {
                                from,
                                toward,
                                opinion,
                            });
                        }
                    }
                    DiplomacyAction::Guarantee(guarantor, protected) => {
                        diplomacy.guarantees.insert((guarantor, protected));
                        delta.changes.push(StateChange::GuaranteeChanged {
                            guarantor,
                            protected,
                            active: true,
                        });
                    }
                    DiplomacyAction::RevokeGuarantee(guarantor, protected) => {
                        diplomacy.guarantees.remove(&(guarantor, protected));
                        delta.changes.push(StateChange::GuaranteeChanged {
                            guarantor,
                            protected,
                            active: false,
                        });
                        let opinion = self
                            .opinions
                            .get(&(protected, guarantor))
                            .copied()
                            .unwrap_or(Opinion::NEUTRAL)
                            .adjusted(-10);
                        self.opinions.insert((protected, guarantor), opinion);
                        delta.changes.push(StateChange::OpinionChanged {
                            from: protected,
                            toward: guarantor,
                            opinion,
                        });
                    }
                }
            }
            PreparedCommand::ChooseEvent(country, key, option, prepared) => {
                let used = prepared.len();
                for (source, grant) in prepared {
                    self.countries
                        .get_mut(&country)
                        .expect("validated event country")
                        .modifiers
                        .insert_source(source, grant.expires_at, vec![grant.modifier.clone()])
                        .expect("validated event source");
                    delta
                        .changes
                        .push(StateChange::ModifierSourceActivated { country, grant });
                }
                self.events
                    .as_mut()
                    .expect("validated event state")
                    .resolve(country, key, self.time.tick(), used);
                delta.changes.push(StateChange::EventResolved {
                    country,
                    key: key.into(),
                    option,
                });
            }
            PreparedCommand::Marriage(action) => {
                let diplomacy = self.diplomacy.get_or_insert_with(DiplomacyState::default);
                match action {
                    MarriageAction::Offer(from, toward, expires_at) => {
                        diplomacy.marriage_offers.insert((from, toward), expires_at);
                        delta.changes.push(StateChange::MarriageOffered {
                            from,
                            toward,
                            expires_at,
                        });
                    }
                    MarriageAction::Accept(from, toward) => {
                        let removed: Vec<_> = diplomacy
                            .marriage_offers
                            .keys()
                            .copied()
                            .filter(|&(a, b)| a == from || b == from || a == toward || b == toward)
                            .collect();
                        for (a, b) in removed {
                            diplomacy.marriage_offers.remove(&(a, b));
                            delta
                                .changes
                                .push(StateChange::MarriageOfferRemoved { from: a, toward: b });
                        }
                        let polity = self.polity.as_mut().expect("marriage polity validated");
                        polity
                            .characters
                            .get_mut(&from)
                            .expect("proposer validated")
                            .spouse = Some(toward);
                        polity
                            .characters
                            .get_mut(&toward)
                            .expect("partner validated")
                            .spouse = Some(from);
                        delta.changes.push(StateChange::MarriageChanged {
                            a: from,
                            b: toward,
                            active: true,
                        });
                    }
                    MarriageAction::Reject(from, toward) => {
                        diplomacy.marriage_offers.remove(&(from, toward));
                        delta
                            .changes
                            .push(StateChange::MarriageOfferRemoved { from, toward });
                    }
                }
            }
        }
        Ok(delta)
    }

    fn prepare_command<'a>(
        &self,
        actor: CommandActor,
        command: &'a Command,
    ) -> Result<PreparedCommand<'a>, CommandError> {
        match command {
            Command::Time(command) => {
                let mut time = self.time;
                time.apply(actor.time_actor(), *command)
                    .map_err(CommandError::Time)?;
                Ok(PreparedCommand::Time(time))
            }
            Command::SetController {
                country,
                controller,
            } => {
                if !matches!(actor, CommandActor::Host(_)) {
                    return Err(CommandError::HostRequired);
                }
                if !self.countries.contains_key(country) {
                    return Err(CommandError::UnknownCountry(*country));
                }
                if let (Some(polity), Controller::Ai(profile)) = (self.polity(), controller) {
                    if !polity.ai_profiles.contains_key(profile.as_str()) {
                        return Err(CommandError::UnknownAiProfile(*country));
                    }
                }
                Ok(PreparedCommand::SetController(*country, controller))
            }
            Command::SetFiscalPolicy { country, policy } => {
                let polity = self.authorized_polity(actor, *country)?;
                polity
                    .rules
                    .check_policy(*policy)
                    .map_err(CommandError::Fiscal)?;
                Ok(PreparedCommand::SetFiscalPolicy(*country, *policy))
            }
            Command::NominateHeir { country, heir } => {
                let polity = self.authorized_polity(actor, *country)?;
                if let Some(heir) = heir {
                    if !polity.heir_is_eligible(*country, *heir, self.time.date()) {
                        return Err(CommandError::InvalidHeir {
                            country: *country,
                            heir: *heir,
                        });
                    }
                }
                Ok(PreparedCommand::NominateHeir(*country, *heir))
            }
            Command::RaiseArmy {
                country,
                province,
                soldiers,
            } => self.prepare_military(actor, *country, MilitaryOrder::Raise(*province, *soldiers)),
            Command::MergeArmies {
                country,
                into,
                from,
            } => self.prepare_military(actor, *country, MilitaryOrder::Merge(*into, *from)),
            Command::SplitArmy {
                country,
                army,
                soldiers,
            } => self.prepare_military(actor, *country, MilitaryOrder::Split(*army, *soldiers)),
            Command::MoveArmy {
                country,
                army,
                destination,
            } => self.prepare_military(actor, *country, MilitaryOrder::Move(*army, *destination)),
            Command::MoveFleet {
                country,
                fleet,
                destination,
            } => {
                self.authorized_polity(actor, *country)?;
                let navy = self
                    .navy()
                    .ok_or(CommandError::Navy(NavyError::Unavailable))?;
                navy.fleet_route(&self.province_graph, *fleet, *country, *destination)
                    .map(|route| PreparedCommand::MoveFleet(*fleet, route))
                    .map_err(CommandError::Navy)
            }
            Command::EmbarkArmy {
                country,
                army,
                fleet,
            } => self.prepare_embark(actor, *country, *army, *fleet),
            Command::DisembarkArmy {
                country,
                fleet,
                destination,
            } => self.prepare_disembark(actor, *country, *fleet, *destination),
            Command::RetreatArmy {
                country,
                army,
                destination,
            } => {
                self.prepare_military(actor, *country, MilitaryOrder::Retreat(*army, *destination))
            }
            Command::DeclareWar {
                country,
                defender,
                goal,
            } => self.prepare_military(actor, *country, MilitaryOrder::Declare(*defender, *goal)),
            Command::OfferPeace {
                country,
                war,
                cede_goal,
            } => self.prepare_military(actor, *country, MilitaryOrder::Offer(*war, *cede_goal)),
            Command::AcceptPeace { country, war } => {
                self.prepare_military(actor, *country, MilitaryOrder::Accept(*war))
            }
            Command::OfferAlliance { country, toward } => {
                self.prepare_diplomacy(actor, *country, *toward, 0)
            }
            Command::AcceptAlliance { country, from } => {
                self.prepare_diplomacy(actor, *country, *from, 1)
            }
            Command::RejectAlliance { country, from } => {
                self.prepare_diplomacy(actor, *country, *from, 2)
            }
            Command::BreakAlliance { country, other } => {
                self.prepare_diplomacy(actor, *country, *other, 3)
            }
            Command::ChooseEvent {
                country,
                key,
                option,
            } => {
                self.authorized_country(actor, *country)?;
                let prepared = self
                    .events
                    .as_ref()
                    .ok_or_else(|| CommandError::Event(EventError::UnknownEvent(key.clone())))?
                    .prepare_choice(*country, key, *option, self.time.tick())
                    .map_err(CommandError::Event)?;
                Ok(PreparedCommand::ChooseEvent(
                    *country, key, *option, prepared,
                ))
            }
            Command::OfferRoyalMarriage {
                country,
                character,
                toward,
                partner,
            } => self.prepare_marriage(actor, *country, *character, *toward, *partner, 0),
            Command::AcceptRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => self.prepare_marriage(actor, *from, *character, *country, *partner, 1),
            Command::RejectRoyalMarriage {
                country,
                from,
                character,
                partner,
            } => self.prepare_marriage(actor, *from, *character, *country, *partner, 2),
            Command::Guarantee { country, protected } => {
                self.prepare_diplomacy(actor, *country, *protected, 4)
            }
            Command::RevokeGuarantee { country, protected } => {
                self.prepare_diplomacy(actor, *country, *protected, 5)
            }
        }
    }

    fn prepare_marriage(
        &self,
        actor: CommandActor,
        from: CountryId,
        character: CharacterId,
        toward: CountryId,
        partner: CharacterId,
        kind: u8,
    ) -> Result<PreparedCommand<'static>, CommandError> {
        self.authorized_polity(actor, if kind == 0 { from } else { toward })?;
        let now = self.time.date();
        let action = match kind {
            0 => {
                self.marriage_eligibility(from, character, toward, partner)
                    .map_err(CommandError::Marriage)?;
                if self.diplomacy.as_ref().is_some_and(|d| {
                    d.marriage_offers.contains_key(&(character, partner))
                        || d.marriage_offers.contains_key(&(partner, character))
                }) {
                    return Err(CommandError::Marriage(MarriageError::OfferAlreadyPending));
                }
                MarriageAction::Offer(
                    character,
                    partner,
                    now.checked_add_days(30)
                        .ok_or(CommandError::Marriage(MarriageError::DateOverflow))?,
                )
            }
            _ => {
                let valid = self
                    .diplomacy
                    .as_ref()
                    .and_then(|d| d.marriage_offer(character, partner))
                    .is_some_and(|offer| offer.expires_at > now);
                if !valid {
                    return Err(CommandError::Marriage(MarriageError::NoOffer));
                }
                let polity = self
                    .polity
                    .as_ref()
                    .ok_or(CommandError::PolityUnavailable)?;
                if polity
                    .characters
                    .get(&partner)
                    .is_none_or(|person| person.country != toward)
                    || polity
                        .characters
                        .get(&character)
                        .is_none_or(|person| person.country != from)
                {
                    return Err(CommandError::Marriage(MarriageError::InvalidState));
                }
                if kind == 1 {
                    self.marriage_eligibility(from, character, toward, partner)
                        .map_err(CommandError::Marriage)?;
                    MarriageAction::Accept(character, partner)
                } else {
                    MarriageAction::Reject(character, partner)
                }
            }
        };
        Ok(PreparedCommand::Marriage(action))
    }

    fn prepare_diplomacy(
        &self,
        actor: CommandActor,
        country: CountryId,
        other: CountryId,
        kind: u8,
    ) -> Result<PreparedCommand<'static>, CommandError> {
        self.authorized_polity(actor, country)?;
        if !self.countries.contains_key(&other) {
            return Err(CommandError::Diplomacy(DiplomacyError::UnknownCountry(
                other,
            )));
        }
        if country == other {
            return Err(CommandError::Diplomacy(DiplomacyError::SameCountry));
        }
        let now = self.time.date();
        let diplomacy = self.diplomacy.as_ref();
        let allied = self.allied(country, other);
        let action = match kind {
            0 => {
                if self.directly_at_war(country, other) {
                    return Err(CommandError::Diplomacy(DiplomacyError::DirectWar));
                }
                if allied {
                    return Err(CommandError::Diplomacy(DiplomacyError::AlreadyAllied));
                }
                if let Some(until) = diplomacy
                    .and_then(|state| state.cooldowns.get(&crate::diplomacy::pair(country, other)))
                    .copied()
                    .filter(|&until| until > now)
                {
                    return Err(CommandError::Diplomacy(DiplomacyError::Cooldown(until)));
                }
                if diplomacy.is_some_and(|state| {
                    state.offers.contains_key(&(country, other))
                        || state.offers.contains_key(&(other, country))
                }) {
                    return Err(CommandError::Diplomacy(DiplomacyError::OfferAlreadyPending));
                }
                let expiry = now
                    .checked_add_days(30)
                    .ok_or(CommandError::Diplomacy(DiplomacyError::DateOverflow))?;
                DiplomacyAction::Offer(country, other, expiry)
            }
            1 | 2 => {
                if diplomacy
                    .and_then(|state| state.offer(other, country))
                    .is_none_or(|offer| offer.expires_at <= now)
                {
                    return Err(CommandError::Diplomacy(DiplomacyError::NoOffer));
                }
                if kind == 1 {
                    if self.directly_at_war(country, other) {
                        return Err(CommandError::Diplomacy(DiplomacyError::DirectWar));
                    }
                    if allied {
                        return Err(CommandError::Diplomacy(DiplomacyError::AlreadyAllied));
                    }
                    DiplomacyAction::Accept(other, country)
                } else {
                    DiplomacyAction::Reject(other, country)
                }
            }
            3 => {
                if !allied {
                    return Err(CommandError::Diplomacy(DiplomacyError::NotAllied));
                }
                let until = now
                    .checked_add_months(2)
                    .ok_or(CommandError::Diplomacy(DiplomacyError::DateOverflow))?;
                DiplomacyAction::Break(country, other, until)
            }
            4 => {
                if self.country_at_war(country) || self.country_at_war(other) {
                    return Err(CommandError::Diplomacy(DiplomacyError::AtWar));
                }
                if diplomacy.is_some_and(|state| state.guarantees(country, other)) {
                    return Err(CommandError::Diplomacy(DiplomacyError::AlreadyGuaranteed));
                }
                DiplomacyAction::Guarantee(country, other)
            }
            5 => {
                if !diplomacy.is_some_and(|state| state.guarantees(country, other)) {
                    return Err(CommandError::Diplomacy(DiplomacyError::NotGuaranteed));
                }
                DiplomacyAction::RevokeGuarantee(country, other)
            }
            _ => unreachable!("known diplomacy command"),
        };
        Ok(PreparedCommand::Diplomacy(action))
    }

    fn prepare_military(
        &self,
        actor: CommandActor,
        country: CountryId,
        order: MilitaryOrder,
    ) -> Result<PreparedCommand<'static>, CommandError> {
        let polity = self.authorized_polity(actor, country)?;
        if let Some(navy) = &self.navy {
            let armies = match order {
                MilitaryOrder::Merge(a, b) => [Some(a), Some(b)],
                MilitaryOrder::Split(a, _)
                | MilitaryOrder::Move(a, _)
                | MilitaryOrder::Retreat(a, _) => [Some(a), None],
                _ => [None, None],
            };
            for army in armies.into_iter().flatten() {
                if navy.fleets.values().any(|fleet| fleet.cargo == Some(army)) {
                    return Err(CommandError::Navy(NavyError::CargoAlreadyAboard(army)));
                }
            }
        }
        if let MilitaryOrder::Declare(defender, _) = order {
            if self.allied(country, defender) {
                return Err(CommandError::Diplomacy(DiplomacyError::MustBreakAlliance));
            }
            if self
                .diplomacy
                .as_ref()
                .is_some_and(|diplomacy| diplomacy.guarantees(country, defender))
            {
                return Err(CommandError::Diplomacy(DiplomacyError::MustRevokeGuarantee));
            }
        }
        let war = self
            .war_state()
            .ok_or(CommandError::War(WarError::Unavailable))?;
        let raise_morale = if matches!(order, MilitaryOrder::Raise(..)) {
            self.military_modifier(country, war.rules.max_morale, &war.rules.morale_modifier)
        } else {
            None
        };
        let mut action = war
            .prepare(
                &self.province_graph,
                polity,
                self.time.tick(),
                country,
                order,
                raise_morale,
            )
            .map_err(CommandError::War)?;
        let mut failed_guarantees = Vec::new();
        if let MilitaryAction::Declare(_, declared) = &mut action {
            if let Some(diplomacy) = &self.diplomacy {
                for guarantor in diplomacy.guarantors_of(declared.defender) {
                    if war.war_for(guarantor).is_some() || self.allied(guarantor, country) {
                        failed_guarantees.push((guarantor, declared.defender));
                    } else if guarantor != country {
                        declared.defender_supporters.insert(guarantor);
                    }
                }
            }
        }
        Ok(PreparedCommand::Military(action, failed_guarantees))
    }

    fn prepare_embark(
        &self,
        actor: CommandActor,
        country: CountryId,
        army_id: ArmyId,
        fleet_id: FleetId,
    ) -> Result<PreparedCommand<'static>, CommandError> {
        self.authorized_polity(actor, country)?;
        let war = self
            .war_state()
            .ok_or(CommandError::Navy(NavyError::Unavailable))?;
        let navy = self
            .navy()
            .ok_or(CommandError::Navy(NavyError::Unavailable))?;
        let army = war
            .armies
            .get(&army_id)
            .ok_or(CommandError::Navy(NavyError::UnknownArmy(army_id)))?;
        let fleet = navy
            .fleets
            .get(&fleet_id)
            .ok_or(CommandError::Navy(NavyError::UnknownFleet(fleet_id)))?;
        if army.country != country || fleet.country != country {
            return Err(CommandError::Navy(NavyError::NotFleetOwner(fleet_id)));
        }
        if army.movement.is_some() {
            return Err(CommandError::Navy(NavyError::ArmyInMotion(army_id)));
        }
        if fleet.movement.is_some() {
            return Err(CommandError::Navy(NavyError::FleetInMotion(fleet_id)));
        }
        if fleet.cargo.is_some() {
            return Err(CommandError::Navy(NavyError::FleetHasCargo(fleet_id)));
        }
        if navy
            .fleets
            .values()
            .any(|fleet| fleet.cargo == Some(army_id))
        {
            return Err(CommandError::Navy(NavyError::CargoAlreadyAboard(army_id)));
        }
        if u64::from(army.soldiers) > u64::from(fleet.ships) * u64::from(crate::SHIP_CAPACITY) {
            return Err(CommandError::Navy(NavyError::InsufficientCapacity(
                fleet_id,
            )));
        }
        let port_access = self.owner_of_province(army.province).is_some_and(|owner| {
            owner == country
                || war
                    .war_for(country)
                    .is_some_and(|(_, conflict)| conflict.side(owner) == conflict.side(country))
        });
        if !port_access
            || !self.province_graph.get(army.province).is_some_and(|port| {
                port.kind == crate::ProvinceKind::Land
                    && port.ports.contains(&fleet.sea)
                    && port
                        .neighbors
                        .iter()
                        .any(|edge| edge.to == fleet.sea && edge.kind == EdgeKind::Port)
            })
        {
            return Err(CommandError::Navy(NavyError::InvalidPort(army.province)));
        }
        Ok(PreparedCommand::Embark(army_id, fleet_id))
    }

    fn prepare_disembark(
        &self,
        actor: CommandActor,
        country: CountryId,
        fleet_id: FleetId,
        destination: ProvinceId,
    ) -> Result<PreparedCommand<'static>, CommandError> {
        self.authorized_polity(actor, country)?;
        let navy = self
            .navy()
            .ok_or(CommandError::Navy(NavyError::Unavailable))?;
        let fleet = navy
            .fleets
            .get(&fleet_id)
            .ok_or(CommandError::Navy(NavyError::UnknownFleet(fleet_id)))?;
        if fleet.country != country {
            return Err(CommandError::Navy(NavyError::NotFleetOwner(fleet_id)));
        }
        if fleet.movement.is_some() {
            return Err(CommandError::Navy(NavyError::FleetInMotion(fleet_id)));
        }
        if let Some((&blocker, _)) = navy.fleets.iter().find(|(&id, other)| {
            id != fleet_id && other.sea == fleet.sea && self.directly_at_war(country, other.country)
        }) {
            return Err(CommandError::Navy(NavyError::HostileFleet(blocker)));
        }
        let army_id = fleet
            .cargo
            .ok_or(CommandError::Navy(NavyError::NoCargo(fleet_id)))?;
        let owner = self
            .owner_of_province(destination)
            .ok_or(CommandError::Navy(NavyError::InvalidPort(destination)))?;
        let same_side = self.war_state().is_some_and(|war| {
            war.war_for(country)
                .is_some_and(|(_, conflict)| conflict.side(owner) == conflict.side(country))
        });
        if owner != country && !same_side && !self.directly_at_war(country, owner) {
            return Err(CommandError::Navy(NavyError::NotAtWar));
        }
        if !self.province_graph.get(destination).is_some_and(|port| {
            port.kind == crate::ProvinceKind::Land
                && port.ports.contains(&fleet.sea)
                && port
                    .neighbors
                    .iter()
                    .any(|edge| edge.to == fleet.sea && edge.kind == EdgeKind::Port)
        }) {
            return Err(CommandError::Navy(NavyError::InvalidPort(destination)));
        }
        Ok(PreparedCommand::Disembark(army_id, fleet_id, destination))
    }

    fn military_modifier(&self, country: CountryId, base: Fixed, key: &ModKey) -> Option<Fixed> {
        let country_modifiers = self
            .country(country)?
            .modifiers()
            .sources()
            .flat_map(|(_, _, values)| values);
        let ruler_modifiers = self
            .ruler(country)
            .into_iter()
            .filter(|(_, ruler)| ruler.is_alive_on(self.time.date()))
            .flat_map(|(_, ruler)| ruler.modifiers.sources())
            .flat_map(|(_, _, values)| values);
        effective_value_from(base, key, country_modifiers.chain(ruler_modifiers))
            .map(|value| value.max(Fixed::ZERO))
    }

    fn sync_war_flags(&mut self) {
        if let Some(war) = &self.war_state {
            for (&country, state) in &mut self.countries {
                state.at_war = war.war_for(country).is_some();
            }
        }
    }

    fn authorized_polity(
        &self,
        actor: CommandActor,
        country: CountryId,
    ) -> Result<&PolityState, CommandError> {
        self.authorized_country(actor, country)?;
        self.polity().ok_or(CommandError::PolityUnavailable)
    }

    fn authorized_country(
        &self,
        actor: CommandActor,
        country: CountryId,
    ) -> Result<(), CommandError> {
        let base = self
            .country(country)
            .ok_or(CommandError::UnknownCountry(country))?;
        let authorized = match (actor, base.controller()) {
            (
                CommandActor::Host(player) | CommandActor::Guest(player),
                Controller::Human(owner),
            ) => player == *owner,
            (CommandActor::Ai(owned_country), Controller::Ai(_)) => owned_country == country,
            _ => false,
        };
        if !authorized {
            return Err(CommandError::NotCountryController(country));
        }
        Ok(())
    }

    /// Advances time, processes due effects, dispatches AI, then updates countries.
    ///
    /// The host callback gets read-only state and supplies commands to validate
    /// and apply before the next country is queried. A paused tick leaves the
    /// state unchanged and does not call the callback. The host handles and
    /// logs any AI errors or panics within the callback.
    pub fn tick(
        &mut self,
        mut decide_ai: impl FnMut(&Self, CountryId) -> Vec<Command>,
    ) -> Result<TickReport, TimeError> {
        let mut report = TickReport::default();
        let previous_date = self.time.date();
        if !self.time.advance()? {
            return Ok(report);
        }

        let tick = self.time.tick();
        report.delta.changes.push(StateChange::ClockAdvanced {
            tick,
            date: self.time.date(),
        });
        for (&country, state) in &mut self.countries {
            let expired = state.modifiers.expire_through(tick.get());
            report.delta.changes.extend(
                expired
                    .into_iter()
                    .map(|source| StateChange::ModifierSourceExpired { country, source }),
            );
        }

        self.activate_scheduled_effects(tick, &mut report);

        let ai_countries: Vec<_> = self
            .countries()
            .filter_map(|(country, state)| {
                matches!(state.controller(), Controller::Ai(_)).then_some(country)
            })
            .collect();
        for country in ai_countries {
            for command in decide_ai(self, country) {
                match self.apply(CommandActor::Ai(country), &command) {
                    Ok(delta) => report.delta.changes.extend(delta.changes),
                    Err(error) => report.ai_rejections.push(AiCommandRejection {
                        country,
                        command,
                        error,
                    }),
                }
            }
        }
        if let Some(polity) = &mut self.polity {
            let (changes, failures) = polity.advance(
                previous_date,
                self.time.date(),
                &self.province_graph,
                &self.countries,
            );
            report.delta.changes.extend(changes);
            report.polity_failures = failures;
        }
        if let Some(diplomacy) = &mut self.diplomacy {
            for offer in diplomacy.expire(self.time.date()) {
                report
                    .delta
                    .changes
                    .push(StateChange::AllianceOfferRemoved {
                        from: offer.from,
                        toward: offer.toward,
                    });
            }
            if let Some(polity) = &self.polity {
                for offer in diplomacy.expire_marriages(self.time.date(), &polity.characters) {
                    report
                        .delta
                        .changes
                        .push(StateChange::MarriageOfferRemoved {
                            from: offer.from,
                            toward: offer.toward,
                        });
                }
            }
        }
        self.advance_military(&mut report);
        if let Some(navy) = &mut self.navy {
            let previous = navy.advance();
            let battles = navy.resolve_battles(
                &self.province_graph,
                self.war_state
                    .iter()
                    .flat_map(|state| state.wars.iter().map(|(&id, war)| (id, war))),
                &previous,
                |army| {
                    self.war_state
                        .as_ref()
                        .and_then(|war| war.armies.get(&army).map(|a| a.soldiers))
                },
            );
            let mut changed: BTreeSet<_> = previous.keys().copied().collect();
            for battle in battles {
                changed.extend(battle.changed_fleets.iter().copied());
                if let Some(war) = &mut self.war_state {
                    for &army in &battle.lost_armies {
                        war.armies.remove(&army);
                        report
                            .delta
                            .changes
                            .push(StateChange::ArmyChanged { army, state: None });
                    }
                }
                report
                    .delta
                    .changes
                    .push(StateChange::NavalBattleResolved(battle));
            }
            for id in changed {
                report.delta.changes.push(StateChange::FleetChanged {
                    fleet: id,
                    state: navy.fleets.get(&id).cloned(),
                });
            }
        }
        if let Some(events) = &mut self.events {
            let countries = self
                .countries
                .iter()
                .map(|(&id, state)| (id, state as &dyn TriggerView));
            for (country, key) in events.fire_due(self.seed, tick, countries) {
                report
                    .delta
                    .changes
                    .push(StateChange::EventFired { country, key });
            }
        }
        Ok(report)
    }

    fn advance_military(&mut self, report: &mut TickReport) {
        let Some(war) = self.war_state() else { return };
        if war.armies.is_empty() {
            return;
        }
        let countries = war
            .armies
            .values()
            .map(|army| army.country)
            .collect::<BTreeSet<_>>();
        let bonuses = countries
            .into_iter()
            .map(|country| {
                let morale = self.military_modifier(
                    country,
                    war.rules.max_morale,
                    &war.rules.morale_modifier,
                );
                let combat =
                    self.military_modifier(country, Fixed::ONE, &war.rules.combat_modifier);
                (country, MilitaryBonuses { morale, combat })
            })
            .collect();
        let war = self.war_state.as_mut().expect("military state exists");
        let polity = self.polity.as_ref().expect("military state has a polity");
        // Cargo remains in the army table for stable IDs and saves, but cannot
        // fight, siege, recover, move or take land supply while aboard.
        let cargo_ids: Vec<_> = self
            .navy
            .iter()
            .flat_map(|navy| navy.fleets.values().filter_map(|fleet| fleet.cargo))
            .collect();
        let embarked: Vec<_> = cargo_ids
            .into_iter()
            .filter_map(|id| war.armies.remove(&id).map(|army| (id, army)))
            .collect();
        let (changes, failures) = war.advance(&self.province_graph, polity, &bonuses);
        war.armies.extend(embarked);
        report.delta.changes.extend(changes);
        report.military_failures = failures;
    }

    fn activate_scheduled_effects(&mut self, tick: Tick, report: &mut TickReport) {
        let Some(effects) = self.scheduled.remove(&tick) else {
            return;
        };
        for ((country, source), grant) in effects {
            let state = self
                .countries
                .get_mut(&country)
                .expect("scheduled effects target a known country");
            let activation = state.modifiers.insert_source(
                source,
                grant.expires_at,
                vec![grant.modifier.clone()],
            );
            match activation {
                Ok(()) => report
                    .delta
                    .changes
                    .push(StateChange::ModifierSourceActivated { country, grant }),
                Err(error) => report.scheduled_failures.push(ScheduledEffectFailure {
                    tick,
                    country,
                    source,
                    error,
                }),
            }
        }
    }
}
