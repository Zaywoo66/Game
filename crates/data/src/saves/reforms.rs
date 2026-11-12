use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    CountryId, Effect, Fixed, ModKey, ModifierKind, ModifierSourceId, ReformDefinition,
    ReformError, ReformState,
};

use super::{require_sorted, SaveError};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReformStateFile {
    definitions: Vec<ReformDefinitionFile>,
    sources: Vec<ReformSourceFile>,
    next_source_id: u32,
}

impl From<&ReformState> for ReformStateFile {
    fn from(state: &ReformState) -> Self {
        Self {
            definitions: state
                .definitions
                .values()
                .map(ReformDefinitionFile::from)
                .collect(),
            sources: state
                .sources
                .iter()
                .map(|((country, key), source)| ReformSourceFile {
                    country: country.get(),
                    key: key.clone(),
                    source: source.get(),
                })
                .collect(),
            next_source_id: state.next_source_id,
        }
    }
}

impl ReformStateFile {
    pub(super) fn restore(self) -> Result<ReformState, SaveError> {
        require_sorted(&self.definitions, "reform definitions", |item| {
            item.key.clone()
        })?;
        require_sorted(&self.sources, "reform sources", |item| {
            (item.country, item.key.clone())
        })?;
        let definitions = self
            .definitions
            .into_iter()
            .map(|item| {
                let definition = item.restore()?;
                Ok((definition.key.clone(), definition))
            })
            .collect::<Result<BTreeMap<_, _>, SaveError>>()?;
        let sources = self
            .sources
            .into_iter()
            .map(|item| {
                (
                    (CountryId::new(item.country), item.key),
                    ModifierSourceId::new(item.source),
                )
            })
            .collect();
        Ok(ReformState {
            definitions,
            choices: BTreeMap::new(),
            sources,
            next_source_id: self.next_source_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReformDefinitionFile {
    key: String,
    government: String,
    tier: u16,
    ai_weight: u16,
    effects: Vec<ReformEffectFile>,
}

impl From<&ReformDefinition> for ReformDefinitionFile {
    fn from(value: &ReformDefinition) -> Self {
        Self {
            key: value.key.clone(),
            government: value.government.clone(),
            tier: value.tier,
            ai_weight: value.ai_weight,
            effects: value.effects.iter().map(ReformEffectFile::from).collect(),
        }
    }
}

impl ReformDefinitionFile {
    fn restore(self) -> Result<ReformDefinition, SaveError> {
        Ok(ReformDefinition {
            key: self.key,
            government: self.government,
            tier: self.tier,
            ai_weight: self.ai_weight,
            effects: self
                .effects
                .into_iter()
                .map(ReformEffectFile::restore)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReformSourceFile {
    country: u32,
    key: String,
    source: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReformEffectFile {
    key: String,
    value: i64,
    kind: String,
}

impl From<&Effect> for ReformEffectFile {
    fn from(effect: &Effect) -> Self {
        match effect {
            Effect::AddModifier {
                key, value, kind, ..
            } => Self {
                key: key.as_str().into(),
                value: value.raw(),
                kind: match kind {
                    ModifierKind::Additive => "additive",
                    ModifierKind::Multiplicative => "multiplicative",
                }
                .into(),
            },
        }
    }
}

impl ReformEffectFile {
    fn restore(self) -> Result<Effect, SaveError> {
        let kind = match self.kind.as_str() {
            "additive" => ModifierKind::Additive,
            "multiplicative" => ModifierKind::Multiplicative,
            _ => return Err(SaveError::InvalidReform(ReformError::InvalidSavedState)),
        };
        Ok(Effect::AddModifier {
            key: ModKey::new(self.key),
            value: Fixed::from_raw(self.value),
            kind,
            duration_days: None,
        })
    }
}
