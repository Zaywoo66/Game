use std::collections::BTreeMap;
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    ConditionalAiWeight, CountryId, Effect, EventDefinition, EventError, EventOption, EventState,
    Fixed, ModKey, ModifierKind, Tick, Trigger,
};

use super::{require_sorted, SaveError};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EventStateFile {
    definitions: Vec<EventDefinitionFile>,
    pending: Vec<EventOccurrenceFile>,
    resolved: Vec<EventOccurrenceFile>,
    next_source_id: u32,
}

impl From<&EventState> for EventStateFile {
    fn from(state: &EventState) -> Self {
        Self {
            definitions: state
                .definitions
                .values()
                .map(EventDefinitionFile::from)
                .collect(),
            pending: state
                .pending
                .iter()
                .map(|((country, key), tick)| EventOccurrenceFile {
                    country: country.get(),
                    key: key.clone(),
                    tick: tick.get(),
                })
                .collect(),
            resolved: state
                .resolved
                .iter()
                .map(|((country, key), tick)| EventOccurrenceFile {
                    country: country.get(),
                    key: key.clone(),
                    tick: tick.get(),
                })
                .collect(),
            next_source_id: state.next_source_id,
        }
    }
}

impl EventStateFile {
    pub(super) fn restore(self) -> Result<EventState, SaveError> {
        require_sorted(&self.definitions, "event definitions", |entry| {
            entry.key.clone()
        })?;
        require_sorted(&self.pending, "pending events", |entry| {
            (entry.country, entry.key.clone())
        })?;
        require_sorted(&self.resolved, "resolved events", |entry| {
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
        let mut state =
            EventState::new(definitions, self.next_source_id).map_err(SaveError::InvalidEvent)?;
        state.pending = self
            .pending
            .into_iter()
            .map(|entry| {
                (
                    (CountryId::new(entry.country), entry.key),
                    Tick::new(entry.tick),
                )
            })
            .collect();
        state.resolved = self
            .resolved
            .into_iter()
            .map(|entry| {
                (
                    (CountryId::new(entry.country), entry.key),
                    Tick::new(entry.tick),
                )
            })
            .collect();
        Ok(state)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventOccurrenceFile {
    country: u32,
    key: String,
    tick: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventDefinitionFile {
    key: String,
    title_key: String,
    desc_key: String,
    trigger: TriggerFile,
    mean_time_to_happen: Option<NonZeroU64>,
    repeatable: bool,
    cooldown_days: Option<NonZeroU64>,
    options: Vec<EventOptionFile>,
}

impl From<&EventDefinition> for EventDefinitionFile {
    fn from(event: &EventDefinition) -> Self {
        Self {
            key: event.key.clone(),
            title_key: event.title_key.clone(),
            desc_key: event.desc_key.clone(),
            trigger: TriggerFile::from(&event.trigger),
            mean_time_to_happen: event.mean_time_to_happen,
            repeatable: event.repeatable,
            cooldown_days: event.cooldown_days,
            options: event.options.iter().map(EventOptionFile::from).collect(),
        }
    }
}

impl EventDefinitionFile {
    fn restore(self) -> Result<EventDefinition, SaveError> {
        Ok(EventDefinition {
            key: self.key,
            title_key: self.title_key,
            desc_key: self.desc_key,
            trigger: self.trigger.restore(),
            mean_time_to_happen: self.mean_time_to_happen,
            repeatable: self.repeatable,
            cooldown_days: self.cooldown_days,
            options: self
                .options
                .into_iter()
                .map(EventOptionFile::restore)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventOptionFile {
    text_key: String,
    effects: Vec<EffectFile>,
    ai_weight: u32,
    ai_weight_if: Option<ConditionalAiWeightFile>,
}

impl From<&EventOption> for EventOptionFile {
    fn from(option: &EventOption) -> Self {
        Self {
            text_key: option.text_key.clone(),
            effects: option.effects.iter().map(EffectFile::from).collect(),
            ai_weight: option.ai_weight,
            ai_weight_if: option
                .ai_weight_if
                .as_ref()
                .map(ConditionalAiWeightFile::from),
        }
    }
}

impl EventOptionFile {
    fn restore(self) -> Result<EventOption, SaveError> {
        Ok(EventOption {
            text_key: self.text_key,
            effects: self
                .effects
                .into_iter()
                .map(EffectFile::restore)
                .collect::<Result<_, _>>()?,
            ai_weight: self.ai_weight,
            ai_weight_if: self.ai_weight_if.map(ConditionalAiWeightFile::restore),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConditionalAiWeightFile {
    when: TriggerFile,
    add: i32,
}

impl From<&ConditionalAiWeight> for ConditionalAiWeightFile {
    fn from(weight: &ConditionalAiWeight) -> Self {
        Self {
            when: TriggerFile::from(&weight.when),
            add: weight.add,
        }
    }
}

impl ConditionalAiWeightFile {
    fn restore(self) -> ConditionalAiWeight {
        ConditionalAiWeight {
            when: self.when.restore(),
            add: self.add,
        }
    }
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
#[serde(tag = "kind", rename_all = "snake_case")]
enum EffectFile {
    AddModifier {
        key: String,
        value: i64,
        modifier_kind: String,
        duration_days: Option<NonZeroU64>,
    },
}

impl From<&Effect> for EffectFile {
    fn from(effect: &Effect) -> Self {
        match effect {
            Effect::AddModifier {
                key,
                value,
                kind,
                duration_days,
            } => Self::AddModifier {
                key: key.as_str().into(),
                value: value.raw(),
                modifier_kind: match kind {
                    ModifierKind::Additive => "additive",
                    ModifierKind::Multiplicative => "multiplicative",
                }
                .into(),
                duration_days: *duration_days,
            },
        }
    }
}

impl EffectFile {
    fn restore(self) -> Result<Effect, SaveError> {
        match self {
            Self::AddModifier {
                key,
                value,
                modifier_kind,
                duration_days,
            } => {
                let kind = match modifier_kind.as_str() {
                    "additive" => ModifierKind::Additive,
                    "multiplicative" => ModifierKind::Multiplicative,
                    _ => return Err(SaveError::InvalidEvent(EventError::InvalidDefinition(key))),
                };
                Ok(Effect::AddModifier {
                    key: ModKey::new(key),
                    value: Fixed::from_raw(value),
                    kind,
                    duration_days,
                })
            }
        }
    }
}
