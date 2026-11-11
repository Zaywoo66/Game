use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    AiProfileKey, CharacterId, CharacterState, CharacterStats, CountryId, CountryPolity, Date,
    EconomyState, FiscalPolicy, Fixed, MonthlyBudget, PolityState, ProvinceId, SuccessionState,
};

use super::{require_sorted, required_optional, SaveError, SourceFile};
use crate::polity_format::{ProfileFile, RulesFile, StatsFile};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PolityFile {
    rules: RulesFile,
    countries: Vec<CountryFile>,
    characters: Vec<CharacterFile>,
    ai_profiles: Vec<ProfileFile>,
}

impl From<&PolityState> for PolityFile {
    fn from(state: &PolityState) -> Self {
        Self {
            rules: (&state.rules).into(),
            countries: state
                .countries
                .iter()
                .map(|(&id, country)| CountryFile::snapshot(id, country))
                .collect(),
            characters: state
                .characters
                .iter()
                .map(|(&id, character)| CharacterFile::snapshot(id, character))
                .collect(),
            ai_profiles: state
                .ai_profiles
                .iter()
                .map(|(key, profile)| ProfileFile::snapshot(key, profile))
                .collect(),
        }
    }
}

impl PolityFile {
    pub(super) fn restore(self) -> Result<PolityState, SaveError> {
        require_sorted(&self.countries, "polity countries", |country| country.id)?;
        require_sorted(&self.characters, "characters", |character| character.id)?;
        if self
            .ai_profiles
            .windows(2)
            .any(|pair| pair[0].key >= pair[1].key)
        {
            return Err(SaveError::UnorderedRecords("AI profiles"));
        }
        Ok(PolityState {
            rules: self.rules.into(),
            countries: self
                .countries
                .into_iter()
                .map(CountryFile::restore)
                .collect::<Result<BTreeMap<_, _>, _>>()?,
            characters: self
                .characters
                .into_iter()
                .map(CharacterFile::restore)
                .collect::<Result<BTreeMap<_, _>, _>>()?,
            ai_profiles: self
                .ai_profiles
                .into_iter()
                .map(ProfileFile::restore)
                .collect(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountryFile {
    id: u32,
    tag: String,
    name: String,
    color: [u8; 3],
    capital: u32,
    provinces: Vec<u32>,
    ai_profile: String,
    economy: EconomyFile,
    succession: SuccessionFile,
}

impl CountryFile {
    fn snapshot(id: CountryId, country: &CountryPolity) -> Self {
        Self {
            id: id.get(),
            tag: country.tag.clone(),
            name: country.name.clone(),
            color: country.color,
            capital: country.capital.get(),
            provinces: country.provinces.iter().map(|id| id.get()).collect(),
            ai_profile: country.ai_profile.as_str().into(),
            economy: (&country.economy).into(),
            succession: SuccessionFile {
                ruler: country.succession.ruler.map(CharacterId::get),
                heir: country.succession.heir.map(CharacterId::get),
            },
        }
    }

    fn restore(self) -> Result<(CountryId, CountryPolity), SaveError> {
        require_sorted(&self.provinces, "country provinces", |id| *id)?;
        Ok((
            CountryId::new(self.id),
            CountryPolity {
                tag: self.tag,
                name: self.name,
                color: self.color,
                capital: ProvinceId::new(self.capital),
                provinces: self.provinces.into_iter().map(ProvinceId::new).collect(),
                ai_profile: AiProfileKey::new(self.ai_profile),
                economy: self.economy.into(),
                succession: SuccessionState {
                    ruler: self.succession.ruler.map(CharacterId::new),
                    heir: self.succession.heir.map(CharacterId::new),
                },
            },
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EconomyFile {
    treasury: i64,
    policy: PolicyFile,
    #[serde(deserialize_with = "required_optional")]
    last_month: Option<BudgetFile>,
    bankrupt: bool,
}

impl From<&EconomyState> for EconomyFile {
    fn from(economy: &EconomyState) -> Self {
        Self {
            treasury: economy.treasury.raw(),
            policy: PolicyFile {
                tax_rate: economy.policy.tax_rate.raw(),
                monthly_spending: economy.policy.monthly_spending.raw(),
            },
            last_month: economy.last_month.as_ref().map(|budget| BudgetFile {
                income: budget.income.raw(),
                expenses: budget.expenses.raw(),
            }),
            bankrupt: economy.bankrupt,
        }
    }
}

impl From<EconomyFile> for EconomyState {
    fn from(file: EconomyFile) -> Self {
        Self {
            treasury: Fixed::from_raw(file.treasury),
            policy: FiscalPolicy {
                tax_rate: Fixed::from_raw(file.policy.tax_rate),
                monthly_spending: Fixed::from_raw(file.policy.monthly_spending),
            },
            last_month: file.last_month.map(|budget| MonthlyBudget {
                income: Fixed::from_raw(budget.income),
                expenses: Fixed::from_raw(budget.expenses),
            }),
            bankrupt: file.bankrupt,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    tax_rate: i64,
    monthly_spending: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetFile {
    income: i64,
    expenses: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuccessionFile {
    #[serde(deserialize_with = "required_optional")]
    ruler: Option<u32>,
    #[serde(deserialize_with = "required_optional")]
    heir: Option<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterFile {
    id: u32,
    name: String,
    country: u32,
    dynasty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sex: Option<CharacterSexFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    spouse: Option<u32>,
    birth: (u32, u8, u8),
    #[serde(deserialize_with = "required_optional")]
    death: Option<(u32, u8, u8)>,
    #[serde(with = "StatsFile")]
    stats: CharacterStats,
    traits: Vec<String>,
    sources: Vec<SourceFile>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CharacterSexFile {
    Male,
    Female,
}

impl CharacterFile {
    fn snapshot(id: CharacterId, character: &CharacterState) -> Self {
        Self {
            id: id.get(),
            name: character.name.clone(),
            country: character.country.get(),
            dynasty: character.dynasty.clone(),
            sex: match character.sex {
                strategy_sim::Sex::Unknown => None,
                strategy_sim::Sex::Male => Some(CharacterSexFile::Male),
                strategy_sim::Sex::Female => Some(CharacterSexFile::Female),
            },
            spouse: character.spouse.map(CharacterId::get),
            birth: character.birth.components(),
            death: character.death.map(Date::components),
            stats: character.stats,
            traits: character.traits.iter().cloned().collect(),
            sources: SourceFile::snapshot_all(&character.modifiers),
        }
    }

    fn restore(self) -> Result<(CharacterId, CharacterState), SaveError> {
        if self.traits.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(SaveError::UnorderedRecords("character traits"));
        }
        let date = |(year, month, day)| Date::new(year, month, day).map_err(SaveError::InvalidDate);
        Ok((
            CharacterId::new(self.id),
            CharacterState {
                name: self.name,
                country: CountryId::new(self.country),
                dynasty: self.dynasty,
                sex: match self.sex {
                    None => strategy_sim::Sex::Unknown,
                    Some(CharacterSexFile::Male) => strategy_sim::Sex::Male,
                    Some(CharacterSexFile::Female) => strategy_sim::Sex::Female,
                },
                spouse: self.spouse.map(CharacterId::new),
                birth: date(self.birth)?,
                death: self.death.map(date).transpose()?,
                stats: self.stats,
                traits: self.traits.into_iter().collect(),
                modifiers: SourceFile::restore_all(self.sources, "character modifier sources")?,
            },
        ))
    }
}
