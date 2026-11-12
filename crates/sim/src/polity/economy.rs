use std::collections::BTreeSet;

use crate::{modifiers::effective_value_from, Fixed, ModifierStore, ProvinceGraph, ProvinceId};

use super::{CountryPolity, PolityRules};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FiscalPolicy {
    pub tax_rate: Fixed,
    pub monthly_spending: Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthlyBudget {
    pub income: Fixed,
    /// Paid spending, which can be below the policy target during bankruptcy.
    pub expenses: Fixed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EconomyState {
    pub treasury: Fixed,
    pub policy: FiscalPolicy,
    pub last_month: Option<MonthlyBudget>,
    pub bankrupt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiscalError {
    PolicyOutOfRange,
    ArithmeticOverflow,
}

impl PolityRules {
    pub(crate) fn check_policy(&self, policy: FiscalPolicy) -> Result<(), FiscalError> {
        if policy.tax_rate < Fixed::ZERO
            || policy.tax_rate > self.max_tax_rate
            || policy.monthly_spending < Fixed::ZERO
            || policy.monthly_spending > self.max_monthly_spending
        {
            return Err(FiscalError::PolicyOutOfRange);
        }
        Ok(())
    }
}

impl EconomyState {
    pub(super) fn is_valid(self, rules: &PolityRules) -> bool {
        if self.treasury.raw() < -rules.debt_limit.raw()
            || (self.bankrupt && self.treasury > Fixed::ZERO)
            || (self.bankrupt && self.treasury == Fixed::ZERO && rules.debt_limit != Fixed::ZERO)
        {
            return false;
        }
        self.last_month.is_none_or(|month| {
            month.income >= Fixed::ZERO
                && month.expenses >= Fixed::ZERO
                && month.expenses <= rules.max_monthly_spending
        })
    }
}

pub(super) fn monthly_economy(
    rules: &PolityRules,
    country: &CountryPolity,
    graph: &ProvinceGraph,
    blockaded_ports: &BTreeSet<ProvinceId>,
    country_modifiers: &ModifierStore,
    ruler_traits: Option<&ModifierStore>,
    policy: FiscalPolicy,
) -> Result<EconomyState, FiscalError> {
    rules.check_policy(policy)?;
    let development: i128 = country
        .provinces
        .iter()
        .filter(|id| !blockaded_ports.contains(id))
        .map(|id| {
            let province = graph.get(*id).expect("polity owns a known province");
            i128::from(province.development)
                + i128::from(rules.base_taxable_development_per_province)
        })
        .sum();
    let raw_rate = i128::from(rules.tax_per_development.raw()) * i128::from(policy.tax_rate.raw());
    let tax_raw = development
        .checked_mul(raw_rate)
        .ok_or(FiscalError::ArithmeticOverflow)?
        / i128::from(Fixed::SCALE);
    let tax = fixed_from_wide(tax_raw)?;
    let country_grants = country_modifiers
        .sources()
        .flat_map(|(_, _, modifiers)| modifiers);
    let trait_grants = ruler_traits
        .into_iter()
        .flat_map(ModifierStore::sources)
        .flat_map(|(_, _, modifiers)| modifiers);
    let income = effective_value_from(
        tax,
        &rules.income_modifier,
        country_grants.chain(trait_grants),
    )
    .ok_or(FiscalError::ArithmeticOverflow)?
    .max(Fixed::ZERO);

    let funds = i128::from(country.economy.treasury.raw()) + i128::from(income.raw());
    let planned = i128::from(policy.monthly_spending.raw());
    let paid = planned.min(funds + i128::from(rules.debt_limit.raw()));
    let treasury = fixed_from_wide(funds - paid)?;
    let bankrupt = paid < planned || (country.economy.bankrupt && treasury < Fixed::ZERO);

    Ok(EconomyState {
        treasury,
        policy,
        last_month: Some(MonthlyBudget {
            income,
            expenses: fixed_from_wide(paid)?,
        }),
        bankrupt,
    })
}

fn fixed_from_wide(raw: i128) -> Result<Fixed, FiscalError> {
    i64::try_from(raw)
        .map(Fixed::from_raw)
        .map_err(|_| FiscalError::ArithmeticOverflow)
}
