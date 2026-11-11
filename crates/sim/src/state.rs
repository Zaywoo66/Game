use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CountryId, Date, Effect, EffectError, Fixed, MapIdentity, ModKey, ModifierInsertError,
    ModifierSourceId, ModifierStore, PlayerId, PreparedModifier, ProvinceGraph, SeededRng, SimTime,
    Speed, Tick, TimeActor, TimeCommand, TimeError, TriggerView,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    Time(TimeError),
    HostRequired,
    UnknownCountry(CountryId),
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
}

enum PreparedCommand<'a> {
    Time(SimTime),
    SetController(CountryId, &'a Controller),
}

impl GameState {
    /// Initializes state and validates all scheduled effects before play starts.
    pub fn new(
        time: SimTime,
        seed: u64,
        province_graph: ProvinceGraph,
        countries: BTreeMap<CountryId, CountryState>,
        schedule: impl IntoIterator<Item = ScheduledEffect>,
    ) -> Result<Self, ScheduleError> {
        let mut scheduled = ModifierSchedule::new();
        for ScheduledEffect {
            due,
            country,
            source,
            effect,
        } in schedule
        {
            if due <= time.tick() {
                return Err(ScheduleError::NotFuture {
                    due,
                    current: time.tick(),
                });
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
        })
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
                Ok(PreparedCommand::SetController(*country, controller))
            }
        }
    }

    /// Advances time, expires sources, activates due effects, then dispatches AI.
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
