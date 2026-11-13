use std::collections::BTreeMap;

use crate::{CountryId, Effect, ModifierSourceId, Trigger};

/// Whole research points spent once for each national idea.
pub const IDEA_COST: u32 = 100;

/// One country-specific idea copied from the validated content catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaDefinition {
    pub key: String,
    pub country: String,
    pub name_key: String,
    pub unlock: Trigger,
    pub modifiers: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaState {
    pub definitions: BTreeMap<String, IdeaDefinition>,
    pub sources: BTreeMap<(CountryId, String), ModifierSourceId>,
    pub next_source_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdeaError {
    AlreadyConfigured,
    EmptyCatalog,
    InvalidDefinition(String),
    UnknownIdea(String),
    WrongCountry(String),
    AlreadyKnown(String),
    Locked(String),
    InsufficientPoints { available: u32 },
    InvalidSavedState,
    ModifierSourceExhausted,
}
