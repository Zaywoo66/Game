use std::collections::{BTreeMap, BTreeSet};

use crate::{
    province_graph::is_content_key, AiProfileKey, CharacterId, Controller, CountryId, CountryState,
    Date, Fixed, ModKey, ModifierSourceId, ModifierStore, ProvinceGraph, ProvinceId, ProvinceKind,
    StateChange,
};

mod economy;
mod succession;

use economy::monthly_economy;
pub use economy::{EconomyState, FiscalError, FiscalPolicy, MonthlyBudget};
pub use succession::{CharacterState, CharacterStats, SuccessionState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolityRules {
    pub adulthood_years: u16,
    pub base_taxable_development_per_province: u32,
    pub tax_per_development: Fixed,
    pub max_tax_rate: Fixed,
    pub max_monthly_spending: Fixed,
    pub debt_limit: Fixed,
    pub income_modifier: ModKey,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiWeights {
    pub aggression: u16,
    pub expansion: u16,
    pub caution: u16,
    pub diplomacy: u16,
    pub economy: u16,
    pub naval: u16,
    pub loyalty: u16,
}

impl AiWeights {
    fn is_valid(self) -> bool {
        [
            self.aggression,
            self.expansion,
            self.caution,
            self.diplomacy,
            self.economy,
            self.naval,
            self.loyalty,
        ]
        .into_iter()
        .all(|value| value <= 1000)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiThresholds {
    pub war_min_score: i32,
    pub peace_accept_score: i32,
    pub alliance_min_opinion: i32,
}

/// Scenario policy preferences. Bot decisions stay in the AI crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiscalAiProfile {
    pub weights: AiWeights,
    pub thresholds: AiThresholds,
    pub preferred_tax_rate: Fixed,
    pub preferred_monthly_spending: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountryPolity {
    pub tag: String,
    pub name: String,
    pub color: [u8; 3],
    pub capital: ProvinceId,
    pub provinces: BTreeSet<ProvinceId>,
    pub ai_profile: AiProfileKey,
    pub economy: EconomyState,
    pub succession: SuccessionState,
}

/// Scenario inputs and authoritative country data. GameState exposes this read-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolityState {
    pub rules: PolityRules,
    pub countries: BTreeMap<CountryId, CountryPolity>,
    pub characters: BTreeMap<CharacterId, CharacterState>,
    pub ai_profiles: BTreeMap<String, FiscalAiProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolityError {
    AlreadyConfigured,
    InvalidRules,
    MissingCountry(CountryId),
    UnknownCountry(CountryId),
    DuplicateTag(String),
    InvalidCapital {
        country: CountryId,
        capital: ProvinceId,
    },
    InvalidProvince {
        country: CountryId,
        province: ProvinceId,
    },
    SharedProvince {
        province: ProvinceId,
        first: CountryId,
        second: CountryId,
    },
    UnknownCharacterCountry {
        character: CharacterId,
        country: CountryId,
    },
    InvalidCharacterDates(CharacterId),
    TimedTrait {
        character: CharacterId,
        source: ModifierSourceId,
    },
    InvalidRuler {
        country: CountryId,
        ruler: CharacterId,
    },
    InvalidHeir {
        country: CountryId,
        heir: CharacterId,
    },
    InvalidFiscalPolicy(CountryId),
    InvalidEconomy(CountryId),
    InvalidProfile(String),
    UnknownProfile {
        country: CountryId,
        profile: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolityTickFailure {
    pub country: CountryId,
    pub error: FiscalError,
}

impl PolityState {
    pub(crate) fn validate(
        &self,
        graph: &ProvinceGraph,
        base_countries: &BTreeMap<CountryId, CountryState>,
        date: Date,
    ) -> Result<(), PolityError> {
        if self.rules.adulthood_years == 0
            || self.rules.tax_per_development < Fixed::ZERO
            || self.rules.max_tax_rate < Fixed::ZERO
            || self.rules.max_monthly_spending < Fixed::ZERO
            || self.rules.debt_limit < Fixed::ZERO
            || !is_content_key(self.rules.income_modifier.as_str())
        {
            return Err(PolityError::InvalidRules);
        }
        for (key, profile) in &self.ai_profiles {
            if !is_content_key(key)
                || !profile.weights.is_valid()
                || self
                    .rules
                    .check_policy(FiscalPolicy {
                        tax_rate: profile.preferred_tax_rate,
                        monthly_spending: profile.preferred_monthly_spending,
                    })
                    .is_err()
            {
                return Err(PolityError::InvalidProfile(key.clone()));
            }
        }
        for &country in base_countries.keys() {
            if !self.countries.contains_key(&country) {
                return Err(PolityError::MissingCountry(country));
            }
        }
        for (&character, state) in &self.characters {
            if !self.countries.contains_key(&state.country) {
                return Err(PolityError::UnknownCharacterCountry {
                    character,
                    country: state.country,
                });
            }
            if state.death.is_some_and(|death| death <= state.birth) {
                return Err(PolityError::InvalidCharacterDates(character));
            }
            if let Some((source, _, _)) = state
                .modifiers
                .sources()
                .find(|(_, expiry, _)| expiry.is_some())
            {
                return Err(PolityError::TimedTrait { character, source });
            }
        }
        let mut ownership = BTreeMap::new();
        let mut tags = BTreeSet::new();
        for (&country, state) in &self.countries {
            let base = base_countries
                .get(&country)
                .ok_or(PolityError::UnknownCountry(country))?;
            if !tags.insert(&state.tag) {
                return Err(PolityError::DuplicateTag(state.tag.clone()));
            }
            if !state.provinces.contains(&state.capital) {
                return Err(PolityError::InvalidCapital {
                    country,
                    capital: state.capital,
                });
            }
            for &province in &state.provinces {
                if !graph
                    .get(province)
                    .is_some_and(|province| province.kind == ProvinceKind::Land)
                {
                    return Err(PolityError::InvalidProvince { country, province });
                }
                if let Some(first) = ownership.insert(province, country) {
                    return Err(PolityError::SharedProvince {
                        province,
                        first,
                        second: country,
                    });
                }
            }
            self.require_profile(country, &state.ai_profile)?;
            if let Controller::Ai(profile) = base.controller() {
                self.require_profile(country, profile)?;
            }
            self.rules
                .check_policy(state.economy.policy)
                .map_err(|_| PolityError::InvalidFiscalPolicy(country))?;
            if !state.economy.is_valid(&self.rules) {
                return Err(PolityError::InvalidEconomy(country));
            }
            if let Some(ruler) = state.succession.ruler {
                if !self.character_is_available(country, ruler, date) {
                    return Err(PolityError::InvalidRuler { country, ruler });
                }
            }
            if let Some(heir) = state.succession.heir {
                if !self.heir_is_eligible(country, heir, date) {
                    return Err(PolityError::InvalidHeir { country, heir });
                }
            }
        }
        Ok(())
    }

    fn require_profile(
        &self,
        country: CountryId,
        profile: &AiProfileKey,
    ) -> Result<(), PolityError> {
        if self.ai_profiles.contains_key(profile.as_str()) {
            Ok(())
        } else {
            Err(PolityError::UnknownProfile {
                country,
                profile: profile.as_str().to_owned(),
            })
        }
    }

    pub(crate) fn advance(
        &mut self,
        previous_date: Date,
        date: Date,
        graph: &ProvinceGraph,
        base_countries: &BTreeMap<CountryId, CountryState>,
    ) -> (Vec<StateChange>, Vec<PolityTickFailure>) {
        let mut changes = self.advance_characters(previous_date, date);
        let mut failures = Vec::new();
        if date.components().2 != 1 {
            return (changes, failures);
        }

        for (&country, state) in &mut self.countries {
            let ruler_traits = state
                .succession
                .ruler
                .and_then(|ruler| self.characters.get(&ruler))
                .map(|ruler| &ruler.modifiers);
            match monthly_economy(
                &self.rules,
                state,
                graph,
                base_countries[&country].modifiers(),
                ruler_traits,
                state.economy.policy,
            ) {
                Ok(economy) if economy != state.economy => {
                    state.economy = economy;
                    changes.push(StateChange::EconomyChanged { country, economy });
                }
                Ok(_) => {}
                Err(error) => failures.push(PolityTickFailure { country, error }),
            }
        }
        (changes, failures)
    }

    pub(crate) fn forecast(
        &self,
        country: CountryId,
        state: &CountryPolity,
        date: Date,
        graph: &ProvinceGraph,
        modifiers: &ModifierStore,
        policy: FiscalPolicy,
    ) -> Result<EconomyState, FiscalError> {
        let ruler_traits = self
            .projected_succession(country, state, date)
            .ruler
            .and_then(|ruler| self.characters.get(&ruler))
            .map(|ruler| &ruler.modifiers);
        monthly_economy(&self.rules, state, graph, modifiers, ruler_traits, policy)
    }
}
