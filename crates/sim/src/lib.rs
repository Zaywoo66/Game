//! Deterministic simulation core.

mod fixed;
mod ids;
mod rng;
mod time;

pub use fixed::Fixed;
pub use ids::{ArmyId, CharacterId, CountryId, ProvinceId};
pub use rng::{derive_ai_rng, SeededRng};
pub use time::{
    Date, InvalidDate, InvalidSpeed, SimTime, Speed, Tick, TimeActor, TimeCommand, TimeError,
};
