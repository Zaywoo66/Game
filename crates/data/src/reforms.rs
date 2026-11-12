use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use strategy_sim::{Effect, Fixed, ModKey, ModifierKind, ReformDefinition};

use crate::is_content_key;

#[derive(Debug)]
pub enum ReformLoadError {
    Yaml(yaml_serde::Error),
    Invalid(String),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ReformLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => write!(f, "invalid government reform YAML: {error}"),
            Self::Invalid(reason) => f.write_str(reason),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}
impl std::error::Error for ReformLoadError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    schema_version: u32,
    governments: BTreeMap<String, Vec<RawTier>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier {
    reforms: Vec<RawReform>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReform {
    key: String,
    ai_weight: u16,
    effects: Vec<RawEffect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEffect {
    key: String,
    value: i64,
    kind: String,
}

pub fn parse_reforms_yaml(
    yaml: &str,
) -> Result<BTreeMap<String, ReformDefinition>, ReformLoadError> {
    if let Some((line, token)) = crate::yaml_tokens::noncanonical_integer(yaml) {
        return Err(ReformLoadError::Invalid(format!(
            "line {line}: integer {token:?} must be plain decimal"
        )));
    }
    let raw: RawCatalog = yaml_serde::from_str(yaml).map_err(ReformLoadError::Yaml)?;
    if raw.schema_version != 1 || raw.governments.is_empty() {
        return Err(ReformLoadError::Invalid(
            "schema_version must be 1 and governments must not be empty".into(),
        ));
    }
    let mut definitions = BTreeMap::new();
    for (government, tiers) in raw.governments {
        if !is_content_key(&government) || tiers.is_empty() || tiers.len() > u16::MAX as usize {
            return Err(ReformLoadError::Invalid(format!(
                "invalid government or reform tiers: {government}"
            )));
        }
        for (index, tier) in tiers.into_iter().enumerate() {
            if tier.reforms.is_empty() {
                return Err(ReformLoadError::Invalid(format!(
                    "government {government} tier {} has no reforms",
                    index + 1
                )));
            }
            for reform in tier.reforms {
                if !is_content_key(&reform.key)
                    || reform.ai_weight > 1000
                    || reform.effects.is_empty()
                    || definitions.contains_key(&reform.key)
                {
                    return Err(ReformLoadError::Invalid(format!(
                        "invalid or duplicate reform {}",
                        reform.key
                    )));
                }
                let effects = reform
                    .effects
                    .into_iter()
                    .map(|effect| {
                        if !is_content_key(&effect.key) {
                            return Err(ReformLoadError::Invalid(format!(
                                "invalid modifier key {}",
                                effect.key
                            )));
                        }
                        let kind = match effect.kind.as_str() {
                            "additive" => ModifierKind::Additive,
                            "multiplicative" => ModifierKind::Multiplicative,
                            _ => {
                                return Err(ReformLoadError::Invalid(
                                    "effect kind must be additive or multiplicative".into(),
                                ))
                            }
                        };
                        if kind == ModifierKind::Multiplicative
                            && effect.value > i64::MAX - Fixed::SCALE
                        {
                            return Err(ReformLoadError::Invalid(
                                "multiplier exceeds Fixed range".into(),
                            ));
                        }
                        Ok(Effect::AddModifier {
                            key: ModKey::new(effect.key),
                            value: Fixed::from_raw(effect.value),
                            kind,
                            duration_days: None,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let key = reform.key.clone();
                definitions.insert(
                    key.clone(),
                    ReformDefinition {
                        key,
                        government: government.clone(),
                        tier: (index + 1) as u16,
                        ai_weight: reform.ai_weight,
                        effects,
                    },
                );
            }
        }
    }
    Ok(definitions)
}

pub fn load_reforms(path: &Path) -> Result<BTreeMap<String, ReformDefinition>, ReformLoadError> {
    let yaml = fs::read_to_string(path).map_err(|source| ReformLoadError::Io {
        path: path.into(),
        source,
    })?;
    parse_reforms_yaml(&yaml)
}

#[cfg(test)]
mod tests {
    use super::parse_reforms_yaml;

    #[test]
    fn modifier_values_must_be_raw_fixed_integers() {
        let yaml = "schema_version: 1\ngovernments:\n  monarchy:\n    - reforms:\n        - key: test_reform\n          ai_weight: 1\n          effects:\n            - {key: tax_income, value: 0.1, kind: additive}\n";
        assert!(parse_reforms_yaml(yaml).is_err());
    }
}
