use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    CountryId, Effect, Fixed, ModKey, ModifierKind, ModifierSourceId, TechnologyDefinition,
    TechnologyState,
};

use super::{require_sorted, SaveError};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TechnologyStateFile {
    definitions: Vec<TechnologyDefinitionFile>,
    points: Vec<ResearchPointsFile>,
    sources: Vec<TechnologySourceFile>,
    next_source_id: u32,
}

impl From<&TechnologyState> for TechnologyStateFile {
    fn from(state: &TechnologyState) -> Self {
        Self {
            definitions: state
                .definitions
                .values()
                .map(TechnologyDefinitionFile::from)
                .collect(),
            points: state
                .points
                .iter()
                .map(|(&country, &points)| ResearchPointsFile {
                    country: country.get(),
                    points,
                })
                .collect(),
            sources: state
                .sources
                .iter()
                .map(|((country, key), source)| TechnologySourceFile {
                    country: country.get(),
                    key: key.clone(),
                    source: source.get(),
                })
                .collect(),
            next_source_id: state.next_source_id,
        }
    }
}

impl TechnologyStateFile {
    pub(super) fn restore(self) -> Result<TechnologyState, SaveError> {
        require_sorted(&self.definitions, "technology definitions", |entry| {
            entry.key.clone()
        })?;
        require_sorted(&self.points, "research points", |entry| entry.country)?;
        require_sorted(&self.sources, "technology sources", |entry| {
            (entry.country, entry.key.clone())
        })?;
        let definitions = self
            .definitions
            .into_iter()
            .map(|entry| {
                let definition = entry.restore()?;
                Ok((definition.key.clone(), definition))
            })
            .collect::<Result<BTreeMap<_, _>, SaveError>>()?;
        Ok(TechnologyState {
            definitions,
            points: self
                .points
                .into_iter()
                .map(|entry| (CountryId::new(entry.country), entry.points))
                .collect(),
            sources: self
                .sources
                .into_iter()
                .map(|entry| {
                    (
                        (CountryId::new(entry.country), entry.key),
                        ModifierSourceId::new(entry.source),
                    )
                })
                .collect(),
            next_source_id: self.next_source_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TechnologyDefinitionFile {
    key: String,
    branch: String,
    cost: NonZeroU32,
    requires: Vec<String>,
    effects: Vec<TechnologyEffectFile>,
}

impl From<&TechnologyDefinition> for TechnologyDefinitionFile {
    fn from(definition: &TechnologyDefinition) -> Self {
        Self {
            key: definition.key.clone(),
            branch: definition.branch.clone(),
            cost: definition.cost,
            requires: definition.requires.clone(),
            effects: definition
                .effects
                .iter()
                .map(TechnologyEffectFile::from)
                .collect(),
        }
    }
}

impl TechnologyDefinitionFile {
    fn restore(self) -> Result<TechnologyDefinition, SaveError> {
        Ok(TechnologyDefinition {
            key: self.key,
            branch: self.branch,
            cost: self.cost,
            requires: self.requires,
            effects: self
                .effects
                .into_iter()
                .map(TechnologyEffectFile::restore)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TechnologyEffectFile {
    AddModifier {
        key: String,
        value: i64,
        modifier_kind: String,
    },
}

impl From<&Effect> for TechnologyEffectFile {
    fn from(effect: &Effect) -> Self {
        match effect {
            Effect::AddModifier {
                key, value, kind, ..
            } => Self::AddModifier {
                key: key.as_str().into(),
                value: value.raw(),
                modifier_kind: match kind {
                    ModifierKind::Additive => "additive",
                    ModifierKind::Multiplicative => "multiplicative",
                }
                .into(),
            },
        }
    }
}

impl TechnologyEffectFile {
    fn restore(self) -> Result<Effect, SaveError> {
        match self {
            Self::AddModifier {
                key,
                value,
                modifier_kind,
            } => {
                let kind = match modifier_kind.as_str() {
                    "additive" => ModifierKind::Additive,
                    "multiplicative" => ModifierKind::Multiplicative,
                    _ => {
                        return Err(SaveError::InvalidTechnology(
                            strategy_sim::TechnologyError::InvalidSavedState,
                        ))
                    }
                };
                Ok(Effect::AddModifier {
                    key: ModKey::new(key),
                    value: Fixed::from_raw(value),
                    kind,
                    duration_days: None,
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResearchPointsFile {
    country: u32,
    points: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TechnologySourceFile {
    country: u32,
    key: String,
    source: u32,
}
