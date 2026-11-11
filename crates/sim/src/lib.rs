//! Deterministic simulation core.

mod fixed;
mod ids;
mod modifiers;
mod province_graph;
mod rng;
mod time;
mod trigger;

pub use fixed::Fixed;
pub use ids::{ArmyId, CharacterId, CountryId, ProvinceId};
pub use modifiers::{
    effective_value, ModKey, Modifier, ModifierInsertError, ModifierKind, ModifierSourceId,
    ModifierStore,
};
pub use province_graph::{
    EdgeKind, MapIdentity, MapIdentityMismatch, Province, ProvinceEdge, ProvinceGraph,
    ProvinceGraphError, ProvinceKind,
};
pub use rng::{derive_ai_rng, SeededRng};
pub use time::{
    Date, InvalidDate, InvalidSpeed, SimTime, Speed, Tick, TimeActor, TimeCommand, TimeError,
};
pub use trigger::{Trigger, TriggerView};
