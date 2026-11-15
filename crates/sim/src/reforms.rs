use std::collections::BTreeMap;

use crate::{CountryId, Effect, ModifierSourceId, Sex, Tick};

/// Whole research points paid for one government reform.
pub const REFORM_COST: u32 = 150;
pub const GOVERNMENT_CHANGE_COST: u32 = 500;
pub const GOVERNMENT_CHANGE_DAYS: u64 = 90;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGovernmentChange {
    pub target: String,
    pub completes_at: Tick,
    pub monarch: Option<NewMonarch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewMonarch {
    pub name: String,
    pub dynasty: String,
    pub sex: Sex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReformDefinition {
    pub key: String,
    pub government: String,
    /// One-based position in the government's ordered reform stages.
    pub tier: u16,
    pub ai_weight: u16,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReformState {
    pub definitions: BTreeMap<String, ReformDefinition>,
    /// A country may select one reform key for each tier.
    pub choices: BTreeMap<(crate::CountryId, u16), String>,
    pub sources: BTreeMap<(crate::CountryId, String), ModifierSourceId>,
    pub pending_government_changes: BTreeMap<CountryId, PendingGovernmentChange>,
    pub next_source_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReformError {
    AlreadyConfigured,
    EmptyCatalog,
    InvalidDefinition(String),
    UnknownReform(String),
    WrongGovernment(String),
    TierAlreadyChosen(u16),
    AlreadyKnown(String),
    InsufficientPoints { available: u32 },
    GovernmentUnavailable(String),
    GovernmentChangePending,
    SameGovernment,
    InvalidMonarch,
    ScheduleOverflow,
    InvalidSavedState,
    ModifierSourceExhausted,
}
