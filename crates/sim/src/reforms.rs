use std::collections::BTreeMap;

use crate::{Effect, ModifierSourceId};

/// Whole research points paid for one government reform.
pub const REFORM_COST: u32 = 150;

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
    InvalidSavedState,
    ModifierSourceExhausted,
}
