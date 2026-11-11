use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CharacterId, CharacterState, CountryId, CountryPolity, Date, EconomyState, Effect, EffectError,
    FiscalError, FiscalPolicy, Fixed, MapIdentity, ModKey, ModifierInsertError, ModifierSourceId,
    ModifierStore, PlayerId, PolityError, PolityState, PolityTickFailure, PreparedModifier,
    ProvinceGraph, ProvinceId, SeededRng, SimTime, Speed, SuccessionState, Tick, TimeActor,
    TimeCommand, TimeError, TriggerView,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    Time(TimeError),
    HostRequired,
    UnknownCountry(CountryId),
    PolityUnavailable,
    NotCountryController(CountryId),
    UnknownAiProfile(CountryId),
    Fiscal(FiscalError),
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
    polity: Option<PolityState>,
}

enum PreparedCommand<'a> {
    Time(SimTime),
    SetController(CountryId, &'a Controller),
    SetFiscalPolicy(CountryId, FiscalPolicy),
    NominateHeir(CountryId, Option<CharacterId>),
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
            polity: None,
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
        }
    }

    fn authorized_polity(
        &self,
        actor: CommandActor,
        country: CountryId,
    ) -> Result<&PolityState, CommandError> {
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
        self.polity().ok_or(CommandError::PolityUnavailable)
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
        Ok(report)
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
