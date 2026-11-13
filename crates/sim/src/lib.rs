//! Deterministic simulation core.

mod diplomacy;
mod effect;
mod events;
mod fixed;
mod ideas;
mod ids;
mod modifiers;
mod navy;
mod opinion;
mod polity;
mod province_graph;
mod reforms;
mod rng;
mod state;
mod technology;
mod time;
mod trigger;
mod war;

pub use diplomacy::{
    AllianceOffer, CallToArmsOffer, DiplomacyError, DiplomacyState, MarriageError, MarriageOffer,
};
pub use effect::{Effect, EffectError, PreparedModifier};
pub use events::{
    event_rng, ConditionalAiWeight, EventDefinition, EventError, EventOption, EventState,
};
pub use fixed::Fixed;
pub use ideas::{IdeaDefinition, IdeaError, IdeaState, IDEA_COST};
pub use ids::{ArmyId, CharacterId, CountryId, FleetId, PlayerId, ProvinceId, WarId};
pub use modifiers::{
    effective_value, ModKey, Modifier, ModifierInsertError, ModifierKind, ModifierSourceId,
    ModifierStore,
};
pub use navy::{
    Fleet, FleetMovement, NavalBattle, NavyError, NavyState, ShipBuildOrder, SHIP_BUILD_COST_RAW,
    SHIP_BUILD_DAYS, SHIP_CAPACITY,
};
pub use opinion::{Opinion, OpinionError, OpinionStateError};
pub use polity::{
    AiThresholds, AiWeights, BirthPrecision, CharacterState, CharacterStats, CountryPolity,
    EconomyState, FiscalAiProfile, FiscalError, FiscalPolicy, MonthlyBudget, PolityError,
    PolityRules, PolityState, PolityTickFailure, Sex, SuccessionState,
};
pub use province_graph::{
    EdgeKind, MapIdentity, MapIdentityMismatch, Province, ProvinceEdge, ProvinceGraph,
    ProvinceGraphError, ProvinceKind,
};
pub use reforms::{ReformDefinition, ReformError, ReformState, REFORM_COST};
pub use rng::{derive_ai_rng, SeededRng};
pub use state::{
    AiCommandRejection, AiProfileKey, Command, CommandActor, CommandError, Controller,
    CountryState, Delta, GameState, ScheduleError, ScheduledEffect, ScheduledEffectFailure,
    StateChange, TickReport,
};
pub use technology::{TechnologyDefinition, TechnologyError, TechnologyState};
pub use time::{
    Date, InvalidDate, InvalidSpeed, SimTime, Speed, Tick, TimeActor, TimeCommand, TimeError,
};
pub use trigger::{Trigger, TriggerView};
pub use war::{
    Army, ArmyMovement, MilitaryTickFailure, MovementLeg, PeaceOffer, PeaceTreaty, Siege, War,
    WarError, WarRules, WarSide, WarState,
};
