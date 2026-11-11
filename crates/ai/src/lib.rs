//! Host-side AI decision entry point.

use strategy_sim::{
    derive_ai_rng, Command, CommandActor, Controller, CountryId, FiscalPolicy, Fixed, GameState,
    SeededRng,
};

mod diplomacy;
mod marriage;
mod military;

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
    ProfileBudget,
    AvoidDebt,
    RecoverDebt,
    NominateSuccessor,
    Military { action: MilitaryReason, score: i32 },
    AllianceAccepted,
    AllianceRejected,
    AllianceProposed,
    EventOption,
    MarriageAccepted,
    MarriageRejected,
    MarriageProposed,
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

impl Reason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Technology => "Technology",
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
                let a_weight = profile
                    .tech_branch_weights
                    .get(&a.branch)
                    .copied()
                    .unwrap_or(0);
                let b_weight = profile
                    .tech_branch_weights
                    .get(&b.branch)
                    .copied()
                    .unwrap_or(0);
                b_weight.cmp(&a_weight).then_with(|| a.key.cmp(&b.key))
            });
            if let Some(definition) = choices.first() {
                let command = Command::UnlockTechnology {
                    country,
                    key: definition.key.clone(),
                };
                if state.validate_command(actor, &command).is_ok() {
                    commands.push((command, Reason::Technology));
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
    if let Some(command) = military::decide(state, country, nation, profile, ctx) {
        commands.push(command);
    }
    if let Some(command) = diplomacy::decide(state, country, profile, ctx) {
        commands.push(command);
    }
    if let Some(command) = marriage::decide(state, country, profile) {
        commands.push(command);
    }
    commands
}

pub use diplomacy::{alliance_acceptance_factors, AllianceAcceptanceFactor};
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
