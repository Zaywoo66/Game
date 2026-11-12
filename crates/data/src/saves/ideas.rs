use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    CountryId, Effect, Fixed, IdeaDefinition, IdeaError, IdeaState, ModKey, ModifierKind,
    ModifierSourceId, Trigger,
};

use super::{require_sorted, SaveError};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdeaStateFile {
    definitions: Vec<IdeaDefinitionFile>,
    sources: Vec<IdeaSourceFile>,
    next_source_id: u32,
}

impl From<&IdeaState> for IdeaStateFile {
    fn from(state: &IdeaState) -> Self {
        Self {
            definitions: state
                .definitions
                .values()
                .map(IdeaDefinitionFile::from)
                .collect(),
            sources: state
                .sources
                .iter()
                .map(|((country, key), source)| IdeaSourceFile {
                    country: country.get(),
                    key: key.clone(),
                    source: source.get(),
                })
                .collect(),
            next_source_id: state.next_source_id,
        }
    }
}

impl IdeaStateFile {
    pub(super) fn restore(self) -> Result<IdeaState, SaveError> {
        require_sorted(&self.definitions, "idea definitions", |entry| {
            entry.key.clone()
        })?;
        require_sorted(&self.sources, "idea sources", |entry| {
            (entry.country, entry.key.clone())
        })?;
        let definitions = self
            .definitions
            .into_iter()
            .map(|file| {
                let definition = file.restore()?;
                Ok((definition.key.clone(), definition))
            })
            .collect::<Result<BTreeMap<_, _>, SaveError>>()?;
        let sources = self
            .sources
            .into_iter()
            .map(|entry| {
                (
                    (CountryId::new(entry.country), entry.key),
                    ModifierSourceId::new(entry.source),
                )
            })
            .collect();
        Ok(IdeaState {
            definitions,
            sources,
            next_source_id: self.next_source_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdeaDefinitionFile {
    key: String,
    country: String,
    name_key: String,
    unlock: TriggerFile,
    modifiers: Vec<IdeaModifierFile>,
}

impl From<&IdeaDefinition> for IdeaDefinitionFile {
    fn from(idea: &IdeaDefinition) -> Self {
        Self {
            key: idea.key.clone(),
            country: idea.country.clone(),
            name_key: idea.name_key.clone(),
            unlock: TriggerFile::from(&idea.unlock),
            modifiers: idea.modifiers.iter().map(IdeaModifierFile::from).collect(),
        }
    }
}

impl IdeaDefinitionFile {
    fn restore(self) -> Result<IdeaDefinition, SaveError> {
        Ok(IdeaDefinition {
            key: self.key,
            country: self.country,
            name_key: self.name_key,
            unlock: self.unlock.restore(),
            modifiers: self
                .modifiers
                .into_iter()
                .map(IdeaModifierFile::restore)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdeaSourceFile {
    country: u32,
    key: String,
    source: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum TriggerFile {
    All(Vec<Self>),
    Any(Vec<Self>),
    Not(Box<Self>),
    CountryHasTech(String),
    AtWar(bool),
}

impl From<&Trigger> for TriggerFile {
    fn from(trigger: &Trigger) -> Self {
        match trigger {
            Trigger::All(items) => Self::All(items.iter().map(Self::from).collect()),
            Trigger::Any(items) => Self::Any(items.iter().map(Self::from).collect()),
            Trigger::Not(item) => Self::Not(Box::new(Self::from(item.as_ref()))),
            Trigger::CountryHasTech(key) => Self::CountryHasTech(key.clone()),
            Trigger::AtWar(value) => Self::AtWar(*value),
        }
    }
}

impl TriggerFile {
    fn restore(self) -> Trigger {
        match self {
            Self::All(items) => Trigger::All(items.into_iter().map(Self::restore).collect()),
            Self::Any(items) => Trigger::Any(items.into_iter().map(Self::restore).collect()),
            Self::Not(item) => Trigger::Not(Box::new(item.restore())),
            Self::CountryHasTech(key) => Trigger::CountryHasTech(key),
            Self::AtWar(value) => Trigger::AtWar(value),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdeaModifierFile {
    key: String,
    value: i64,
    kind: String,
}

impl From<&Effect> for IdeaModifierFile {
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

impl IdeaModifierFile {
    fn restore(self) -> Result<Effect, SaveError> {
        let kind = match self.kind.as_str() {
            "additive" => ModifierKind::Additive,
            "multiplicative" => ModifierKind::Multiplicative,
            _ => return Err(SaveError::InvalidIdea(IdeaError::InvalidSavedState)),
        };
        Ok(Effect::AddModifier {
            key: ModKey::new(self.key),
            value: Fixed::from_raw(self.value),
            kind,
            duration_days: None,
        })
    }
}
