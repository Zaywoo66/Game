use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use strategy_sim::Effect;
use yaml_serde::Value;

use crate::events::parse_effect;
use crate::is_content_key;

/// A technology loaded from a validated content branch. Costs are whole research points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TechnologyDefinition {
    pub key: String,
    pub branch: String,
    pub cost: NonZeroU32,
    pub requires: Vec<String>,
    pub position: TechnologyPosition,
    pub effects: Vec<Effect>,
}

/// Signed layout grid coordinates used by the future technology tree UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TechnologyPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub enum TechnologyLoadError {
    Yaml(yaml_serde::Error),
    Invalid {
        field: String,
        reason: String,
    },
    Effect(crate::EventLoadError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    UnsupportedFormat {
        path: PathBuf,
    },
    InFile {
        path: PathBuf,
        source: Box<Self>,
    },
    DuplicateBranch {
        branch: String,
        first: PathBuf,
        second: PathBuf,
    },
    DuplicateKey {
        key: String,
        first: PathBuf,
        second: PathBuf,
    },
    MissingRequirement {
        key: String,
        required: String,
    },
    Cycle {
        keys: Vec<String>,
    },
}

impl fmt::Display for TechnologyLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => write!(f, "invalid technology YAML: {error}"),
            Self::Invalid { field, reason } => write!(f, "{field}: {reason}"),
            Self::Effect(error) => write!(f, "{error}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::UnsupportedFormat { path } => {
                write!(f, "{}: technologies must use YAML", path.display())
            }
            Self::InFile { path, source } => write!(f, "{}: {source}", path.display()),
            Self::DuplicateBranch {
                branch,
                first,
                second,
            } => write!(
                f,
                "duplicate technology branch {branch} in {} and {}",
                first.display(),
                second.display()
            ),
            Self::DuplicateKey { key, first, second } => write!(
                f,
                "duplicate technology key {key} in {} and {}",
                first.display(),
                second.display()
            ),
            Self::MissingRequirement { key, required } => {
                write!(f, "technology {key} requires missing technology {required}")
            }
            Self::Cycle { keys } => {
                write!(f, "technology prerequisite cycle: {}", keys.join(" -> "))
            }
        }
    }
}

impl std::error::Error for TechnologyLoadError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBranch {
    schema_version: u32,
    branch: String,
    technologies: Vec<RawTechnology>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTechnology {
    key: String,
    cost: NonZeroU32,
    requires: Vec<String>,
    pos: RawPosition,
    effects: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPosition {
    x: i32,
    y: i32,
}

/// Parse one v1 branch document. Cross-branch links are checked by the directory loader.
pub fn parse_technology_branch_yaml(
    yaml: &str,
    expected_branch: &str,
) -> Result<Vec<TechnologyDefinition>, TechnologyLoadError> {
    if let Some((line, token)) = crate::yaml_tokens::noncanonical_integer(yaml) {
        return Err(invalid(
            &format!("line {line}"),
            format!("integer token {token:?} must be plain decimal"),
        ));
    }
    let raw: RawBranch = yaml_serde::from_str(yaml).map_err(TechnologyLoadError::Yaml)?;
    if raw.schema_version != 1 {
        return Err(invalid("schema_version", "expected integer 1"));
    }
    check_key(&raw.branch, "branch")?;
    if raw.branch != expected_branch {
        return Err(invalid(
            "branch",
            format!("expected filename stem {expected_branch}"),
        ));
    }
    let mut keys = BTreeSet::new();
    raw.technologies
        .into_iter()
        .enumerate()
        .map(|(index, technology)| {
            let field = format!("technologies[{index}]");
            check_key(&technology.key, &format!("{field}.key"))?;
            if !keys.insert(technology.key.clone()) {
                return Err(invalid(&format!("{field}.key"), "duplicate technology key"));
            }
            let mut requirements = BTreeSet::new();
            for (requirement_index, required) in technology.requires.iter().enumerate() {
                check_key(required, &format!("{field}.requires[{requirement_index}]"))?;
                if !requirements.insert(required) {
                    return Err(invalid(
                        &format!("{field}.requires[{requirement_index}]"),
                        format!("duplicate prerequisite {required}"),
                    ));
                }
            }
            let effects = technology
                .effects
                .iter()
                .enumerate()
                .map(|(effect_index, effect)| {
                    parse_effect(effect, &format!("{field}.effects[{effect_index}]"))
                        .map_err(TechnologyLoadError::Effect)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TechnologyDefinition {
                key: technology.key,
                branch: raw.branch.clone(),
                cost: technology.cost,
                requires: technology.requires,
                position: TechnologyPosition {
                    x: technology.pos.x,
                    y: technology.pos.y,
                },
                effects,
            })
        })
        .collect()
}

/// Load branch files in filename order and validate the complete keyed DAG.
pub fn load_technologies_from_dir(
    dir: &Path,
) -> Result<BTreeMap<String, TechnologyDefinition>, TechnologyLoadError> {
    let entries = fs::read_dir(dir).map_err(|source| TechnologyLoadError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| TechnologyLoadError::Io {
                    path: dir.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut branches = BTreeMap::<String, PathBuf>::new();
    let mut loaded = BTreeMap::<String, (PathBuf, TechnologyDefinition)>::new();
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if extension.eq_ignore_ascii_case("json") {
            return Err(TechnologyLoadError::UnsupportedFormat { path });
        }
        if !extension.eq_ignore_ascii_case("yaml") && !extension.eq_ignore_ascii_case("yml") {
            continue;
        }
        let branch = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| TechnologyLoadError::Invalid {
                field: path.display().to_string(),
                reason: "filename stem must be UTF-8".into(),
            })?;
        let source = fs::read_to_string(&path).map_err(|source| TechnologyLoadError::Io {
            path: path.clone(),
            source,
        })?;
        let definitions = parse_technology_branch_yaml(&source, branch).map_err(|source| {
            TechnologyLoadError::InFile {
                path: path.clone(),
                source: Box::new(source),
            }
        })?;
        if let Some(first) = branches.insert(branch.to_owned(), path.clone()) {
            return Err(TechnologyLoadError::DuplicateBranch {
                branch: branch.to_owned(),
                first,
                second: path,
            });
        }
        for technology in definitions {
            if let Some((first, _)) = loaded.get(&technology.key) {
                return Err(TechnologyLoadError::DuplicateKey {
                    key: technology.key,
                    first: first.clone(),
                    second: path,
                });
            }
            loaded.insert(technology.key.clone(), (path.clone(), technology));
        }
    }
    let catalog = loaded
        .into_iter()
        .map(|(key, (_, tech))| (key, tech))
        .collect();
    validate_technology_dag(&catalog)?;
    Ok(catalog)
}

fn validate_technology_dag(
    catalog: &BTreeMap<String, TechnologyDefinition>,
) -> Result<(), TechnologyLoadError> {
    for (key, technology) in catalog {
        for required in &technology.requires {
            if !catalog.contains_key(required) {
                return Err(TechnologyLoadError::MissingRequirement {
                    key: key.clone(),
                    required: required.clone(),
                });
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut active = BTreeSet::new();
    let mut path = Vec::new();
    for key in catalog.keys() {
        visit(key, catalog, &mut visited, &mut active, &mut path)?;
    }
    Ok(())
}

fn visit(
    key: &str,
    catalog: &BTreeMap<String, TechnologyDefinition>,
    visited: &mut BTreeSet<String>,
    active: &mut BTreeSet<String>,
    path: &mut Vec<String>,
) -> Result<(), TechnologyLoadError> {
    if let Some(index) = path.iter().position(|entry| entry == key) {
        let mut keys = path[index..].to_vec();
        keys.push(key.to_owned());
        return Err(TechnologyLoadError::Cycle { keys });
    }
    if visited.contains(key) {
        return Ok(());
    }
    active.insert(key.to_owned());
    path.push(key.to_owned());
    for required in &catalog[key].requires {
        if active.contains(required) {
            let index = path
                .iter()
                .position(|entry| entry == required)
                .expect("active key in path");
            let mut keys = path[index..].to_vec();
            keys.push(required.clone());
            return Err(TechnologyLoadError::Cycle { keys });
        }
        visit(required, catalog, visited, active, path)?;
    }
    path.pop();
    active.remove(key);
    visited.insert(key.to_owned());
    Ok(())
}

fn check_key(key: &str, field: &str) -> Result<(), TechnologyLoadError> {
    if is_content_key(key) {
        Ok(())
    } else {
        Err(invalid(field, "expected an ASCII snake_case key"))
    }
}

fn invalid(field: &str, reason: impl Into<String>) -> TechnologyLoadError {
    TechnologyLoadError::Invalid {
        field: field.to_owned(),
        reason: reason.into(),
    }
}
