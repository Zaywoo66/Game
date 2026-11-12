use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    AiThresholds, AiWeights, CharacterStats, FiscalAiProfile, Fixed, ModKey, PolityRules,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RulesFile {
    pub adulthood_years: u16,
    pub base_taxable_development_per_province: u32,
    pub tax_per_development: i64,
    pub max_tax_rate: i64,
    pub max_monthly_spending: i64,
    pub debt_limit: i64,
    pub income_modifier: String,
}

impl From<RulesFile> for PolityRules {
    fn from(file: RulesFile) -> Self {
        Self {
            adulthood_years: file.adulthood_years,
            base_taxable_development_per_province: file.base_taxable_development_per_province,
            tax_per_development: Fixed::from_raw(file.tax_per_development),
            max_tax_rate: Fixed::from_raw(file.max_tax_rate),
            max_monthly_spending: Fixed::from_raw(file.max_monthly_spending),
            debt_limit: Fixed::from_raw(file.debt_limit),
            income_modifier: ModKey::new(file.income_modifier),
        }
    }
}

impl From<&PolityRules> for RulesFile {
    fn from(rules: &PolityRules) -> Self {
        Self {
            adulthood_years: rules.adulthood_years,
            base_taxable_development_per_province: rules.base_taxable_development_per_province,
            tax_per_development: rules.tax_per_development.raw(),
            max_tax_rate: rules.max_tax_rate.raw(),
            max_monthly_spending: rules.max_monthly_spending.raw(),
            debt_limit: rules.debt_limit.raw(),
            income_modifier: rules.income_modifier.as_str().into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "AiWeights", deny_unknown_fields)]
pub(super) struct WeightsFile {
    aggression: u16,
    expansion: u16,
    caution: u16,
    diplomacy: u16,
    economy: u16,
    naval: u16,
    loyalty: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "AiThresholds", deny_unknown_fields)]
pub(super) struct ThresholdsFile {
    war_min_score: i32,
    peace_accept_score: i32,
    alliance_min_opinion: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    marriage_min_opinion: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    vassal_min_opinion: i32,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "CharacterStats", deny_unknown_fields)]
pub(super) struct StatsFile {
    adm: u8,
    dip: u8,
    mil: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileFile {
    pub key: String,
    #[serde(with = "WeightsFile")]
    pub weights: AiWeights,
    #[serde(with = "ThresholdsFile")]
    pub thresholds: AiThresholds,
    pub preferred_tax_rate: i64,
    pub preferred_monthly_spending: i64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tech_branch_weights: BTreeMap<String, u16>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub idea_priority: u16,
}

fn is_zero(value: &u16) -> bool {
    *value == 0
}

impl ProfileFile {
    pub fn snapshot(key: &str, profile: &FiscalAiProfile) -> Self {
        Self {
            key: key.into(),
            weights: profile.weights,
            thresholds: profile.thresholds,
            preferred_tax_rate: profile.preferred_tax_rate.raw(),
            preferred_monthly_spending: profile.preferred_monthly_spending.raw(),
            tech_branch_weights: profile.tech_branch_weights.clone(),
            idea_priority: profile.idea_priority,
        }
    }

    pub fn restore(self) -> (String, FiscalAiProfile) {
        (
            self.key,
            FiscalAiProfile {
                weights: self.weights,
                thresholds: self.thresholds,
                preferred_tax_rate: Fixed::from_raw(self.preferred_tax_rate),
                preferred_monthly_spending: Fixed::from_raw(self.preferred_monthly_spending),
                tech_branch_weights: self.tech_branch_weights,
                idea_priority: self.idea_priority,
            },
        )
    }
}
