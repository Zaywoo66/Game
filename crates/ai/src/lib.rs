//! Host-side AI decision entry point.

use strategy_sim::{
    derive_ai_rng, Command, CommandActor, Controller, CountryId, EconomyState, Effect,
    FiscalPolicy, Fixed, GameState, SeededRng,
};

mod diplomacy;
mod marriage;
mod military;
mod naval;

/// Fresh context for one country's decision on one simulation tick.
pub struct AiContext {
    rng: SeededRng,
}

impl AiContext {
    pub fn new(state: &GameState, country: CountryId) -> Self {
        Self {
            rng: derive_ai_rng(state.seed(), state.time().tick().get(), country),
        }
    }

    pub fn rng(&mut self) -> &mut SeededRng {
        &mut self.rng
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Technology,
    NationalIdea,
    GovernmentReform,
    ProfileBudget,
    AvoidDebt,
    RecoverDebt,
    NominateSuccessor,
    Military { action: MilitaryReason, score: i32 },
    Naval(NavalReason),
    AllianceAccepted,
    AllianceRejected,
    AllianceProposed,
    EventOption,
    MarriageAccepted,
    MarriageRejected,
    MarriageProposed,
    GuaranteeIssued,
    VassalageAccepted,
    VassalageRejected,
    CallToArmsAccepted,
    CallToArmsRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MilitaryReason {
    RaiseArmy,
    CombineArmies,
    ReduceSupplyPressure,
    DeclareAgainstWeakerNeighbor,
    DefendCapital,
    DefendWarGoal,
    BesiegeWarGoal,
    Retreat,
    OfferGoal,
    OfferWhitePeace,
    AcceptPeace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavalReason {
    BuildShip,
    MarchToPort,
    SailToPort,
    EmbarkArmy,
    SailToGoal,
    LandArmy,
    BlockadePort,
}

impl Reason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Technology => "Technology",
            Self::NationalIdea => "NationalIdea",
            Self::GovernmentReform => "GovernmentReform",
            Self::ProfileBudget => "ProfileBudget",
            Self::AvoidDebt => "AvoidDebt",
            Self::RecoverDebt => "RecoverDebt",
            Self::NominateSuccessor => "NominateSuccessor",
            Self::AllianceAccepted => "AllianceAccepted",
            Self::AllianceRejected => "AllianceRejected",
            Self::AllianceProposed => "AllianceProposed",
            Self::EventOption => "EventOption",
            Self::MarriageAccepted => "MarriageAccepted",
            Self::MarriageRejected => "MarriageRejected",
            Self::MarriageProposed => "MarriageProposed",
            Self::GuaranteeIssued => "GuaranteeIssued",
            Self::VassalageAccepted => "VassalageAccepted",
            Self::VassalageRejected => "VassalageRejected",
            Self::CallToArmsAccepted => "CallToArmsAccepted",
            Self::CallToArmsRejected => "CallToArmsRejected",
            Self::Military { action, .. } => match action {
                MilitaryReason::RaiseArmy => "RaiseArmy",
                MilitaryReason::CombineArmies => "CombineArmies",
                MilitaryReason::ReduceSupplyPressure => "ReduceSupplyPressure",
                MilitaryReason::DeclareAgainstWeakerNeighbor => "DeclareAgainstWeakerNeighbor",
                MilitaryReason::DefendCapital => "DefendCapital",
                MilitaryReason::DefendWarGoal => "DefendWarGoal",
                MilitaryReason::BesiegeWarGoal => "BesiegeWarGoal",
                MilitaryReason::Retreat => "Retreat",
                MilitaryReason::OfferGoal => "OfferGoal",
                MilitaryReason::OfferWhitePeace => "OfferWhitePeace",
                MilitaryReason::AcceptPeace => "AcceptPeace",
            },
            Self::Naval(action) => match action {
                NavalReason::MarchToPort => "MarchToPort",
                NavalReason::BuildShip => "BuildShip",
                NavalReason::SailToPort => "SailToPort",
                NavalReason::EmbarkArmy => "EmbarkArmy",
                NavalReason::SailToGoal => "SailToGoal",
                NavalReason::LandArmy => "LandArmy",
                NavalReason::BlockadePort => "BlockadePort",
            },
        }
    }

    pub fn score(self) -> Option<i32> {
        match self {
            Self::Military { score, .. } => Some(score),
            _ => None,
        }
    }
}

/// Chooses fiscal, succession and military commands through read-only queries.
pub fn decide(
    state: &GameState,
    country: CountryId,
    ctx: &mut AiContext,
) -> Vec<(Command, Reason)> {
    let Some(polity) = state.polity() else {
        return Vec::new();
    };
    let Some(nation) = state.country_polity(country) else {
        return Vec::new();
    };
    let Some(Controller::Ai(profile_key)) = state.country(country).map(|state| state.controller())
    else {
        return Vec::new();
    };
    let Some(profile) = polity.ai_profiles.get(profile_key.as_str()) else {
        return Vec::new();
    };

    let actor = CommandActor::Ai(country);
    let mut commands = Vec::with_capacity(2);
    if let Some(events) = state.events() {
        if let Some(((_, key), _)) = events
            .pending
            .iter()
            .find(|((subject, _), _)| *subject == country)
        {
            if let Some(option) = events.ai_option(
                country,
                key,
                state.country(country).expect("AI country exists"),
                state.seed(),
                state.time().tick(),
            ) {
                let command = Command::ChooseEvent {
                    country,
                    key: key.clone(),
                    option,
                };
                if state.validate_command(actor, &command).is_ok() {
                    commands.push((command, Reason::EventOption));
                }
            }
        }
    }
    let economy_period = 28;
    if state.time().date().components().2 == 1 {
        let at_war = state.country_at_war(country);
        let economic_need = has_economic_need(
            nation.economy.treasury,
            state
                .preview_monthly_economy(country, nation.economy.policy)
                .ok(),
        );
        if let Some(technology) = state.technologies() {
            let points = state.research_points(country).unwrap_or(0);
            let mut choices: Vec<_> = technology
                .definitions
                .values()
                .filter(|definition| definition.cost.get() <= points)
                .filter(|definition| {
                    state.country(country).is_some_and(|base| {
                        !base.known_technologies().any(|key| key == definition.key)
                            && definition.requires.iter().all(|required| {
                                base.known_technologies().any(|key| key == required)
                            })
                    })
                })
                .collect();
            choices.sort_by(|a, b| {
                let a_weight = technology_priority(
                    profile
                        .tech_branch_weights
                        .get(&a.branch)
                        .copied()
                        .unwrap_or(0),
                    &a.branch,
                    at_war,
                    economic_need,
                );
                let b_weight = technology_priority(
                    profile
                        .tech_branch_weights
                        .get(&b.branch)
                        .copied()
                        .unwrap_or(0),
                    &b.branch,
                    at_war,
                    economic_need,
                );
                b_weight.cmp(&a_weight).then_with(|| a.key.cmp(&b.key))
            });
            let tech = choices.first().map(|definition| {
                (
                    technology_priority(
                        profile
                            .tech_branch_weights
                            .get(&definition.branch)
                            .copied()
                            .unwrap_or(0),
                        &definition.branch,
                        at_war,
                        economic_need,
                    ),
                    definition.key.as_str(),
                )
            });
            let mut ideas: Vec<_> = state
                .ideas()
                .into_iter()
                .flat_map(|ideas| {
                    ideas
                        .definitions
                        .values()
                        .filter(|definition| definition.country == nation.tag)
                        .filter(|definition| {
                            state
                                .validate_command(
                                    actor,
                                    &Command::UnlockIdea {
                                        country,
                                        key: definition.key.clone(),
                                    },
                                )
                                .is_ok()
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            ideas.sort_by(|left, right| {
                let left_weight = idea_priority(
                    profile.idea_priority,
                    &left.modifiers,
                    at_war,
                    economic_need,
                );
                let right_weight = idea_priority(
                    profile.idea_priority,
                    &right.modifiers,
                    at_war,
                    economic_need,
                );
                right_weight
                    .cmp(&left_weight)
                    .then(left.key.cmp(&right.key))
            });
            let idea = ideas.first().copied();
            let mut reforms: Vec<_> = state
                .reforms()
                .into_iter()
                .flat_map(|reforms| reforms.definitions.values())
                .filter(|reform| {
                    state
                        .validate_command(
                            actor,
                            &Command::UnlockReform {
                                country,
                                key: reform.key.clone(),
                            },
                        )
                        .is_ok()
                })
                .collect();
            reforms.sort_by(|left, right| {
                right
                    .ai_weight
                    .cmp(&left.ai_weight)
                    .then(left.key.cmp(&right.key))
            });
            let mut decisions: Vec<(u32, &str, Command, Reason)> = Vec::new();
            if let Some((weight, key)) = tech {
                decisions.push((
                    weight,
                    key,
                    Command::UnlockTechnology {
                        country,
                        key: key.into(),
                    },
                    Reason::Technology,
                ));
            }
            if let Some(idea) = idea {
                decisions.push((
                    idea_priority(
                        profile.idea_priority,
                        &idea.modifiers,
                        at_war,
                        economic_need,
                    ),
                    idea.key.as_str(),
                    Command::UnlockIdea {
                        country,
                        key: idea.key.clone(),
                    },
                    Reason::NationalIdea,
                ));
            }
            if let Some(reform) = reforms.first() {
                decisions.push((
                    u32::from(profile.reform_priority),
                    reform.key.as_str(),
                    Command::UnlockReform {
                        country,
                        key: reform.key.clone(),
                    },
                    Reason::GovernmentReform,
                ));
            }
            decisions.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(right.1)));
            let decision = decisions
                .into_iter()
                .next()
                .map(|(_, _, command, reason)| (command, reason));
            if let Some((command, reason)) = decision {
                if state.validate_command(actor, &command).is_ok() {
                    commands.push((command, reason));
                }
            }
        }
    }
    if (state.time().tick().get() % economy_period + u64::from(country.get())) % economy_period == 0
    {
        if let Some(decision) = fiscal_decision(state, country, profile) {
            commands.push(decision);
        }
    }

    let candidates = state.eligible_heirs(country);
    if !nation
        .succession
        .heir
        .is_some_and(|heir| candidates.contains(&heir))
    {
        let ruler_dynasty = nation
            .succession
            .ruler
            .and_then(|id| state.character(id))
            .map(|character| character.dynasty.as_str());
        let candidate = candidates
            .iter()
            .copied()
            .find(|id| {
                state
                    .character(*id)
                    .is_some_and(|candidate| Some(candidate.dynasty.as_str()) == ruler_dynasty)
            })
            .or_else(|| candidates.first().copied());
        if let Some(heir) = candidate {
            let command = Command::NominateHeir {
                country,
                heir: Some(heir),
            };
            if state.validate_command(actor, &command).is_ok() {
                commands.push((command, Reason::NominateSuccessor));
            }
        }
    }
    if let Some(command) = naval::decide(state, country, nation, profile)
        .or_else(|| military::decide(state, country, nation, profile, ctx))
    {
        commands.push(command);
    }
    if !commands
        .iter()
        .any(|(command, _)| matches!(command, Command::DeclareWar { .. }))
    {
        if let Some(command) = diplomacy::decide(state, country, profile, ctx) {
            commands.push(command);
        }
    }
    if let Some(command) = marriage::decide(state, country, profile) {
        commands.push(command);
    }
    commands
}

fn has_economic_need(treasury: Fixed, forecast: Option<EconomyState>) -> bool {
    let Some(month) = forecast.and_then(|economy| economy.last_month) else {
        return false;
    };
    let monthly_balance = month
        .income
        .checked_sub(month.tribute_paid)
        .and_then(|balance| balance.checked_add(month.tribute_received))
        .and_then(|balance| balance.checked_sub(month.expenses));
    monthly_balance.is_some_and(|balance| balance < Fixed::ZERO) || treasury < month.expenses
}

fn technology_priority(base: u16, branch: &str, at_war: bool, economic_need: bool) -> u32 {
    let matches_need =
        (at_war && branch == "military") || (economic_need && branch == "administration");
    adjusted_priority(base, matches_need)
}

fn idea_priority(base: u16, effects: &[Effect], at_war: bool, economic_need: bool) -> u32 {
    let matches_need = effects.iter().any(|effect| match effect {
        Effect::AddModifier { key, .. } => {
            (at_war && key.as_str() == "army_morale")
                || (economic_need && key.as_str() == "tax_income")
        }
    });
    adjusted_priority(base, matches_need)
}

fn adjusted_priority(base: u16, matches_need: bool) -> u32 {
    u32::from(base) * if matches_need { 2 } else { 1 }
}

pub use diplomacy::{
    alliance_acceptance_factors, vassal_acceptance_factors, AllianceAcceptanceFactor,
};
pub use marriage::{marriage_acceptance_factors, MarriageAcceptanceFactor};

fn fiscal_decision(
    state: &GameState,
    country: CountryId,
    profile: &strategy_sim::FiscalAiProfile,
) -> Option<(Command, Reason)> {
    let rules = &state.polity()?.rules;
    let economy = &state.country_polity(country)?.economy;
    let preferred = FiscalPolicy {
        tax_rate: profile.preferred_tax_rate,
        monthly_spending: profile.preferred_monthly_spending,
    };
    let debt_policy = FiscalPolicy {
        tax_rate: rules.max_tax_rate,
        monthly_spending: Fixed::ZERO,
    };
    let (policy, reason) = if economy.treasury < Fixed::ZERO || economy.bankrupt {
        (debt_policy, Reason::RecoverDebt)
    } else if state
        .preview_monthly_economy(country, preferred)
        .is_ok_and(|economy| economy.treasury >= Fixed::ZERO && !economy.bankrupt)
    {
        (preferred, Reason::ProfileBudget)
    } else {
        (debt_policy, Reason::AvoidDebt)
    };
    let command = Command::SetFiscalPolicy { country, policy };
    (policy != economy.policy
        && state.preview_monthly_economy(country, policy).is_ok()
        && state
            .validate_command(CommandActor::Ai(country), &command)
            .is_ok())
    .then_some((command, reason))
}

#[cfg(test)]
mod technology_need_tests {
    use super::*;
    use strategy_sim::{ModKey, ModifierKind, MonthlyBudget};

    fn forecast(
        income: i64,
        tribute_paid: i64,
        tribute_received: i64,
        expenses: i64,
    ) -> EconomyState {
        EconomyState {
            treasury: Fixed::ZERO,
            policy: FiscalPolicy::default(),
            last_month: Some(MonthlyBudget {
                income: Fixed::from_raw(income),
                tribute_paid: Fixed::from_raw(tribute_paid),
                tribute_received: Fixed::from_raw(tribute_received),
                expenses: Fixed::from_raw(expenses),
            }),
            bankrupt: false,
        }
    }

    #[test]
    fn economic_need_uses_net_monthly_balance_or_cash_reserve() {
        assert!(has_economic_need(
            Fixed::from_raw(5_000),
            Some(forecast(500, 0, 0, 1_000)),
        ));
        assert!(has_economic_need(
            Fixed::from_raw(500),
            Some(forecast(2_000, 0, 0, 1_000)),
        ));
        assert!(!has_economic_need(
            Fixed::from_raw(5_000),
            Some(forecast(2_000, 0, 0, 1_000)),
        ));
    }

    #[test]
    fn war_and_economy_double_only_matching_technology_and_idea_priorities() {
        assert_eq!(technology_priority(500, "military", true, false), 1_000);
        assert_eq!(
            technology_priority(600, "administration", false, true),
            1_200
        );
        assert_eq!(technology_priority(600, "administration", true, false), 600);
        let morale = Effect::AddModifier {
            key: ModKey::new("army_morale"),
            value: Fixed::ZERO,
            kind: ModifierKind::Additive,
            duration_days: None,
        };
        let tax = Effect::AddModifier {
            key: ModKey::new("tax_income"),
            value: Fixed::ZERO,
            kind: ModifierKind::Multiplicative,
            duration_days: None,
        };
        assert_eq!(idea_priority(550, &[morale], true, false), 1_100);
        assert_eq!(idea_priority(550, &[tax], false, true), 1_100);
        assert_eq!(idea_priority(550, &[], true, true), 550);
    }
}
