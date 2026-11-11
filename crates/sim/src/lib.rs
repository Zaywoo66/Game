//! Deterministic simulation core.

mod effect;
mod fixed;
mod ids;
mod modifiers;
mod polity;
mod province_graph;
mod rng;
mod state;
mod time;
mod trigger;
mod war;

pub use effect::{Effect, EffectError, PreparedModifier};
pub use fixed::Fixed;
pub use ids::{ArmyId, CharacterId, CountryId, PlayerId, ProvinceId, WarId};
pub use modifiers::{
    effective_value, ModKey, Modifier, ModifierInsertError, ModifierKind, ModifierSourceId,
    ModifierStore,
};
pub use polity::{
    AiThresholds, AiWeights, CharacterState, CharacterStats, CountryPolity, EconomyState,
    FiscalAiProfile, FiscalError, FiscalPolicy, MonthlyBudget, PolityError, PolityRules,
    PolityState, PolityTickFailure, SuccessionState,
};
pub use province_graph::{
    EdgeKind, MapIdentity, MapIdentityMismatch, Province, ProvinceEdge, ProvinceGraph,
    ProvinceGraphError, ProvinceKind,
};
pub use rng::{derive_ai_rng, SeededRng};
pub use state::{
    AiCommandRejection, AiProfileKey, Command, CommandActor, CommandError, Controller,
    CountryState, Delta, GameState, ScheduleError, ScheduledEffect, ScheduledEffectFailure,
    StateChange, TickReport,
};
pub use time::{
    Date, InvalidDate, InvalidSpeed, SimTime, Speed, Tick, TimeActor, TimeCommand, TimeError,
};
pub use trigger::{Trigger, TriggerView};
pub use war::{
    Army, ArmyMovement, MilitaryTickFailure, MovementLeg, PeaceOffer, PeaceTreaty, Siege, War,
    WarError, WarRules, WarState,
};
