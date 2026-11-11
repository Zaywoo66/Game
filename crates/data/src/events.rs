use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use strategy_sim::{Effect, Fixed, ModKey, ModifierKind, Trigger};
use yaml_serde::Value;

use crate::is_content_key;

pub use strategy_sim::{ConditionalAiWeight, EventDefinition, EventOption};

#[derive(Debug)]
pub enum EventLoadError {
    Yaml(yaml_serde::Error),
    Invalid {
        field: String,
        reason: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InFile {
        path: PathBuf,
        source: Box<Self>,
    },
    DuplicateKey {
        key: String,
        first: PathBuf,
        second: PathBuf,
    },
}

impl fmt::Display for EventLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => write!(f, "invalid event YAML: {error}"),
            Self::Invalid { field, reason } => write!(f, "{field}: {reason}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::InFile { path, source } => write!(f, "{}: {source}", path.display()),
            Self::DuplicateKey { key, first, second } => write!(
                f,
                "duplicate event key {key} in {} and {}",
                first.display(),
                second.display()
            ),
        }
    }
}

impl std::error::Error for EventLoadError {}

/// Parse one event YAML document into simulation trigger/effect types.
pub fn parse_event_yaml(yaml: &str) -> Result<EventDefinition, EventLoadError> {
    if let Some((line, token)) = crate::yaml_tokens::noncanonical_integer(yaml) {
        return Err(invalid(
            &format!("line {line}"),
            format!("integer token {token:?} must be plain decimal"),
        ));
    }
    let raw: RawEvent = yaml_serde::from_str(yaml).map_err(EventLoadError::Yaml)?;
    for (field, key) in [
        ("key", raw.key.as_str()),
        ("title", raw.title.as_str()),
        ("desc", raw.desc.as_str()),
    ] {
        check_key(key, field)?;
    }
    if raw.repeatable != raw.cooldown_days.is_some() {
        return Err(invalid(
            "cooldown_days",
            "required exactly when repeatable is true",
        ));
    }
    if raw.options.is_empty() {
        return Err(invalid("options", "expected at least one option"));
    }
    if !raw.options.iter().any(|option| option.ai_weight > 0) {
        return Err(invalid(
            "options",
            "at least one base ai_weight must be positive",
        ));
    }

    let trigger = parse_trigger(&raw.trigger, "trigger")?;
    let options = raw
        .options
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            let path = format!("options[{index}]");
            check_key(&option.text, &format!("{path}.text"))?;
            let effects = option
                .effects
                .iter()
                .enumerate()
                .map(|(effect_index, effect)| {
                    parse_effect(effect, &format!("{path}.effects[{effect_index}]"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ai_weight_if = option
                .ai_weight_if
                .map(|condition| {
                    Ok(ConditionalAiWeight {
                        when: parse_trigger(&condition.when, &format!("{path}.ai_weight_if.when"))?,
                        add: condition.add,
                    })
                })
                .transpose()?;
            Ok(EventOption {
                text_key: option.text,
                effects,
                ai_weight: option.ai_weight,
                ai_weight_if,
            })
        })
        .collect::<Result<Vec<_>, EventLoadError>>()?;

    Ok(EventDefinition {
        key: raw.key,
        title_key: raw.title,
        desc_key: raw.desc,
        trigger,
        mean_time_to_happen: raw.mean_time_to_happen,
        repeatable: raw.repeatable,
        cooldown_days: raw.cooldown_days,
        options,
    })
}

/// Load YAML files in filename order and reject event-key collisions.
pub fn load_events_from_dir(
    dir: &Path,
) -> Result<BTreeMap<String, EventDefinition>, EventLoadError> {
    let entries = fs::read_dir(dir).map_err(|source| EventLoadError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| EventLoadError::Io {
                    path: dir.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut loaded = BTreeMap::<String, (PathBuf, EventDefinition)>::new();
    for path in paths {
        if !path.is_file()
            || !matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yaml" | "yml")
            )
        {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|source| EventLoadError::Io {
            path: path.clone(),
            source,
        })?;
        let event = parse_event_yaml(&source).map_err(|source| EventLoadError::InFile {
            path: path.clone(),
            source: Box::new(source),
        })?;
        if let Some((first, _)) = loaded.get(&event.key) {
            return Err(EventLoadError::DuplicateKey {
                key: event.key,
                first: first.clone(),
                second: path,
            });
        }
        loaded.insert(event.key.clone(), (path, event));
    }
    Ok(loaded
        .into_iter()
        .map(|(key, (_, event))| (key, event))
        .collect())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    key: String,
    title: String,
    desc: String,
    trigger: Value,
    #[serde(default, deserialize_with = "crate::deserialize_present")]
    mean_time_to_happen: Option<NonZeroU64>,
    #[serde(default)]
    repeatable: bool,
    #[serde(default, deserialize_with = "crate::deserialize_present")]
    cooldown_days: Option<NonZeroU64>,
    options: Vec<RawOption>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOption {
    text: String,
    effects: Vec<Value>,
    ai_weight: u32,
    #[serde(default, deserialize_with = "crate::deserialize_present")]
    ai_weight_if: Option<RawConditionalAiWeight>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConditionalAiWeight {
    when: Value,
    add: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModifier {
    key: String,
    value: i64,
    kind: String,
    #[serde(default, deserialize_with = "crate::deserialize_present")]
    duration_days: Option<NonZeroU64>,
}

fn parse_trigger(value: &Value, field: &str) -> Result<Trigger, EventLoadError> {
    let (kind, child) = one_key(value, field)?;
    match kind {
        "all" | "any" => {
            let children = child
                .as_sequence()
                .ok_or_else(|| invalid(field, "expected a list of triggers"))?;
            let parsed = children
                .iter()
                .enumerate()
                .map(|(index, trigger)| parse_trigger(trigger, &format!("{field}.{kind}[{index}]")))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(if kind == "all" {
                Trigger::All(parsed)
            } else {
                Trigger::Any(parsed)
            })
        }
        "not" => Ok(Trigger::Not(Box::new(parse_trigger(
            child,
            &format!("{field}.not"),
        )?))),
        "country_has_tech" => {
            let key = child
                .as_str()
                .ok_or_else(|| invalid(field, "technology key must be a string"))?;
            check_key(key, &format!("{field}.country_has_tech"))?;
            Ok(Trigger::CountryHasTech(key.to_owned()))
        }
        "at_war" => child
            .as_bool()
            .map(Trigger::AtWar)
            .ok_or_else(|| invalid(field, "at_war must be a boolean")),
        _ => Err(invalid(field, format!("unknown trigger {kind}"))),
    }
}

pub(crate) fn parse_effect(value: &Value, field: &str) -> Result<Effect, EventLoadError> {
    let (kind, child) = one_key(value, field)?;
    if kind != "add_modifier" {
        return Err(invalid(field, format!("unknown effect {kind}")));
    }
    let raw: RawModifier = yaml_serde::from_value(child.clone()).map_err(EventLoadError::Yaml)?;
    check_key(&raw.key, &format!("{field}.add_modifier.key"))?;
    let kind = match raw.kind.as_str() {
        "additive" => ModifierKind::Additive,
        "multiplicative" => ModifierKind::Multiplicative,
        _ => {
            return Err(invalid(
                field,
                "modifier kind must be additive or multiplicative",
            ))
        }
    };
    if kind == ModifierKind::Multiplicative && raw.value > i64::MAX - Fixed::SCALE {
        return Err(invalid(field, "multiplier exceeds Fixed range"));
    }
    Ok(Effect::AddModifier {
        key: ModKey::new(raw.key),
        value: Fixed::from_raw(raw.value),
        kind,
        duration_days: raw.duration_days,
    })
}

fn one_key<'a>(value: &'a Value, field: &str) -> Result<(&'a str, &'a Value), EventLoadError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| invalid(field, "expected a single-key mapping"))?;
    if mapping.len() != 1 {
        return Err(invalid(field, "expected a single-key mapping"));
    }
    let (key, child) = mapping.iter().next().expect("mapping has one entry");
    let key = key
        .as_str()
        .ok_or_else(|| invalid(field, "mapping key must be a string"))?;
    Ok((key, child))
}

fn check_key(key: &str, field: &str) -> Result<(), EventLoadError> {
    if is_content_key(key) {
        Ok(())
    } else {
        Err(invalid(field, "expected an ASCII snake_case key"))
    }
}

fn invalid(field: &str, reason: impl Into<String>) -> EventLoadError {
    EventLoadError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}
