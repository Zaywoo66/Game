use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use strategy_sim::{Effect, Fixed, ModKey, ModifierKind, Trigger};
use yaml_serde::Value;

use crate::is_content_key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdeaDefinition {
    pub key: String,
    pub country: String,
    pub name_key: String,
    pub unlock: Trigger,
    pub modifiers: Vec<Effect>,
}

impl From<IdeaDefinition> for strategy_sim::IdeaDefinition {
    fn from(idea: IdeaDefinition) -> Self {
        Self {
            key: idea.key,
            country: idea.country,
            name_key: idea.name_key,
            unlock: idea.unlock,
            modifiers: idea.modifiers,
        }
    }
}

#[derive(Debug)]
pub enum IdeaLoadError {
    Yaml(yaml_serde::Error),
    Invalid(String),
    Trigger(crate::EventLoadError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for IdeaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => write!(f, "invalid idea YAML: {error}"),
            Self::Invalid(reason) => write!(f, "{reason}"),
            Self::Trigger(error) => write!(f, "invalid idea unlock: {error}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}
impl std::error::Error for IdeaLoadError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdeas {
    schema_version: u32,
    country: String,
    ideas: Vec<RawIdea>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIdea {
    key: String,
    name: String,
    unlock: Value,
    modifiers: Vec<RawModifier>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawModifier {
    key: String,
    value: i64,
    kind: String,
}

pub fn parse_ideas_yaml(
    yaml: &str,
    expected_country: &str,
) -> Result<Vec<IdeaDefinition>, IdeaLoadError> {
    if let Some((line, token)) = crate::yaml_tokens::noncanonical_integer(yaml) {
        return Err(IdeaLoadError::Invalid(format!(
            "line {line}: integer {token:?} must be plain decimal"
        )));
    }
    let raw: RawIdeas = yaml_serde::from_str(yaml).map_err(IdeaLoadError::Yaml)?;
    if raw.schema_version != 1 || !is_content_key(&raw.country) || raw.country != expected_country {
        return Err(IdeaLoadError::Invalid(
            "schema_version must be 1 and country must match filename".into(),
        ));
    }
    if raw.ideas.is_empty() {
        return Err(IdeaLoadError::Invalid(
            "ideas must contain at least one entry".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    raw.ideas
        .into_iter()
        .map(|idea| {
            if !is_content_key(&idea.key)
                || !keys.insert(idea.key.clone())
                || !is_content_key(&idea.name)
            {
                return Err(IdeaLoadError::Invalid(format!(
                    "invalid or duplicate idea key/name: {}",
                    idea.key
                )));
            }
            if idea.modifiers.is_empty() {
                return Err(IdeaLoadError::Invalid(format!(
                    "idea {} requires a modifier",
                    idea.key
                )));
            }
            let unlock = crate::events::parse_trigger(&idea.unlock, "unlock")
                .map_err(IdeaLoadError::Trigger)?;
            let modifiers = idea
                .modifiers
                .into_iter()
                .map(|modifier| {
                    if !is_content_key(&modifier.key) {
                        return Err(IdeaLoadError::Invalid(format!(
                            "invalid modifier key {}",
                            modifier.key
                        )));
                    }
                    let kind = match modifier.kind.as_str() {
                        "additive" => ModifierKind::Additive,
                        "multiplicative" => ModifierKind::Multiplicative,
                        _ => {
                            return Err(IdeaLoadError::Invalid(
                                "modifier kind must be additive or multiplicative".into(),
                            ))
                        }
                    };
                    if kind == ModifierKind::Multiplicative
                        && modifier.value > i64::MAX - Fixed::SCALE
                    {
                        return Err(IdeaLoadError::Invalid(
                            "multiplier exceeds Fixed range".into(),
                        ));
                    }
                    Ok(Effect::AddModifier {
                        key: ModKey::new(modifier.key),
                        value: Fixed::from_raw(modifier.value),
                        kind,
                        duration_days: None,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(IdeaDefinition {
                key: idea.key,
                country: raw.country.clone(),
                name_key: idea.name,
                unlock,
                modifiers,
            })
        })
        .collect()
}

pub fn load_ideas_from_dir(dir: &Path) -> Result<BTreeMap<String, IdeaDefinition>, IdeaLoadError> {
    let entries = fs::read_dir(dir).map_err(|source| IdeaLoadError::Io {
        path: dir.into(),
        source,
    })?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| IdeaLoadError::Io {
                    path: dir.into(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut catalog = BTreeMap::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            return Err(IdeaLoadError::Invalid(format!(
                "{}: ideas must use YAML",
                path.display()
            )));
        }
        if !path
            .extension()
            .is_some_and(|extension| extension == "yaml" || extension == "yml")
        {
            continue;
        }
        let country = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| IdeaLoadError::Invalid("idea filename must be UTF-8".into()))?;
        let text = fs::read_to_string(&path).map_err(|source| IdeaLoadError::Io {
            path: path.clone(),
            source,
        })?;
        for idea in parse_ideas_yaml(&text, country)? {
            if catalog.insert(idea.key.clone(), idea).is_some() {
                return Err(IdeaLoadError::Invalid(format!(
                    "{}: duplicate idea key",
                    path.display()
                )));
            }
        }
    }
    Ok(catalog)
}
