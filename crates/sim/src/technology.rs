use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use crate::{CountryId, Effect, ModifierSourceId};

/// A validated technology copied from content into the authoritative snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologyDefinition {
    pub key: String,
    pub branch: String,
    pub cost: NonZeroU32,
    pub requires: Vec<String>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologyState {
    pub definitions: BTreeMap<String, TechnologyDefinition>,
    pub points: BTreeMap<CountryId, u32>,
    /// One permanent modifier source per unlocked technology and country.
    pub sources: BTreeMap<(CountryId, String), ModifierSourceId>,
    pub next_source_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TechnologyError {
    AlreadyConfigured,
    EmptyCatalog,
    InvalidDefinition(String),
    UnknownCountry(CountryId),
    UnknownTechnology(String),
    AlreadyKnown(String),
    MissingPrerequisite(String),
    InsufficientPoints { required: u32, available: u32 },
    InvalidSavedState,
    ModifierSourceExhausted,
}

impl TechnologyState {
    pub fn new(
        definitions: BTreeMap<String, TechnologyDefinition>,
        countries: impl IntoIterator<Item = CountryId>,
        next_source_id: u32,
    ) -> Result<Self, TechnologyError> {
        if definitions.is_empty() {
            return Err(TechnologyError::EmptyCatalog);
        }
        if next_source_id == 0 {
            return Err(TechnologyError::ModifierSourceExhausted);
        }
        let branches: BTreeSet<_> = definitions
            .values()
            .map(|tech| tech.branch.as_str())
            .collect();
        for (key, definition) in &definitions {
            if key != &definition.key
                || definition.branch.is_empty()
                || definition
                    .requires
                    .iter()
                    .any(|required| !definitions.contains_key(required))
                || definition.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        Effect::AddModifier {
                            duration_days: Some(_),
                            ..
                        }
                    )
                })
            {
                return Err(TechnologyError::InvalidDefinition(key.clone()));
            }
        }
        fn visit<'a>(
            key: &'a str,
            definitions: &'a BTreeMap<String, TechnologyDefinition>,
            visiting: &mut BTreeSet<&'a str>,
            visited: &mut BTreeSet<&'a str>,
        ) -> bool {
            if visited.contains(key) {
                return true;
            }
            if !visiting.insert(key) {
                return false;
            }
            for required in &definitions[key].requires {
                if !visit(required, definitions, visiting, visited) {
                    return false;
                }
            }
            visiting.remove(key);
            visited.insert(key);
            true
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for key in definitions.keys() {
            if !visit(key, &definitions, &mut visiting, &mut visited) {
                return Err(TechnologyError::InvalidDefinition(format!(
                    "cycle through {key}"
                )));
            }
        }
        if branches.is_empty() {
            return Err(TechnologyError::EmptyCatalog);
        }
        Ok(Self {
            definitions,
            points: countries.into_iter().map(|id| (id, 0)).collect(),
            sources: BTreeMap::new(),
            next_source_id,
        })
    }

    pub fn branches(&self) -> BTreeSet<&str> {
        self.definitions
            .values()
            .map(|tech| tech.branch.as_str())
            .collect()
    }
}
