use strategy_sim::{Command, CommandActor, CountryId, FiscalAiProfile, GameState, SeededRng};

use crate::{AiContext, Reason};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllianceAcceptanceFactor {
    pub key: &'static str,
    pub value: i32,
}

/// The same explainable inputs used by AI acceptance and the human tooltip.
pub fn alliance_acceptance_factors(
    state: &GameState,
    from: CountryId,
    toward: CountryId,
    profile: &FiscalAiProfile,
) -> Vec<AllianceAcceptanceFactor> {
    vec![
        AllianceAcceptanceFactor {
            key: "opinion",
            value: state
                .opinion(toward, from)
                .map_or(0, |o| i32::from(o.points())),
        },
        AllianceAcceptanceFactor {
            key: "profile_minimum",
            value: profile.thresholds.alliance_min_opinion,
        },
        AllianceAcceptanceFactor {
            key: "common_stronger_neighbor",
            value: i32::from(state.common_stronger_neighbor(from, toward)),
        },
        AllianceAcceptanceFactor {
            key: "direct_war",
            value: i32::from(state.directly_at_war(from, toward)),
        },
        AllianceAcceptanceFactor {
            key: "negative_opinion_accept_percent",
            value: if state.opinion(toward, from).is_some_and(|o| o.points() < 0)
                && state.common_stronger_neighbor(from, toward)
            {
                10
            } else {
                0
            },
        },
    ]
}

pub(super) fn decide(
    state: &GameState,
    country: CountryId,
    profile: &FiscalAiProfile,
    ctx: &mut AiContext,
) -> Option<(Command, Reason)> {
    let actor = CommandActor::Ai(country);
    if let Some(offer) = state
        .diplomacy()
        .and_then(|d| d.call_to_arms_offers_to(country).next())
    {
        let opinion = state
            .opinion(country, offer.from)
            .map_or(0, |opinion| i32::from(opinion.points()));
        let accepts = opinion >= profile.thresholds.alliance_min_opinion;
        let command = if accepts {
            Command::AcceptCallToArms {
                country,
                war: offer.war,
                from: offer.from,
            }
        } else {
            Command::RejectCallToArms {
                country,
                war: offer.war,
                from: offer.from,
            }
        };
        let reason = if accepts {
            Reason::CallToArmsAccepted
        } else {
            Reason::CallToArmsRejected
        };
        return state
            .validate_command(actor, &command)
            .is_ok()
            .then_some((command, reason));
    }
    if let Some(offer) = state
        .diplomacy()
        .and_then(|d| d.vassal_offers_to(country).next())
    {
        let factors = vassal_acceptance_factors(state, offer.from, country, profile);
        let accepts = factors[0].value >= factors[1].value && factors[2].value > factors[3].value;
        let (command, reason) = if accepts {
            (
                Command::AcceptVassalage {
                    country,
                    from: offer.from,
                },
                Reason::VassalageAccepted,
            )
        } else {
            (
                Command::RejectVassalage {
                    country,
                    from: offer.from,
                },
                Reason::VassalageRejected,
            )
        };
        return state
            .validate_command(actor, &command)
            .is_ok()
            .then_some((command, reason));
    }
    if let Some(offer) = state.diplomacy().and_then(|d| d.offers_to(country).next()) {
        let factors = alliance_acceptance_factors(state, offer.from, country, profile);
        let opinion = factors[0].value;
        let positive = opinion >= 0 && opinion >= profile.thresholds.alliance_min_opinion;
        let threatened = opinion < 0
            && accepts_negative(
                state.common_stronger_neighbor(offer.from, country),
                ctx.rng(),
            );
        let command = if !state.directly_at_war(offer.from, country) && (positive || threatened) {
            Command::AcceptAlliance {
                country,
                from: offer.from,
            }
        } else {
            Command::RejectAlliance {
                country,
                from: offer.from,
            }
        };
        let reason = if matches!(command, Command::AcceptAlliance { .. }) {
            Reason::AllianceAccepted
        } else {
            Reason::AllianceRejected
        };
        return state
            .validate_command(actor, &command)
            .is_ok()
            .then_some((command, reason));
    }

    // Strategic diplomacy is staggered by country and uses stable candidate order.
    if (state.time().tick().get() + u64::from(country.get())) % 31 != 0 {
        return None;
    }
    for (other, _) in state.countries() {
        if other == country
            || state.allied(country, other)
            || state
                .diplomacy()
                .is_some_and(|diplomacy| diplomacy.guarantees(country, other))
            || !state.common_stronger_neighbor(country, other)
            || state
                .opinion(country, other)
                .map_or(0, |opinion| i32::from(opinion.points()))
                < profile.thresholds.alliance_min_opinion
        {
            continue;
        }
        let command = Command::Guarantee {
            country,
            protected: other,
        };
        if state.validate_command(actor, &command).is_ok() {
            return Some((command, Reason::GuaranteeIssued));
        }
    }
    for (other, _) in state.countries() {
        if other == country || state.allied(country, other) {
            continue;
        }
        let opinion = state
            .opinion(country, other)
            .map_or(0, |o| i32::from(o.points()));
        let willing = if opinion < 0 {
            accepts_negative(state.common_stronger_neighbor(country, other), ctx.rng())
        } else {
            opinion >= profile.thresholds.alliance_min_opinion
        };
        if !willing {
            continue;
        }
        let command = Command::OfferAlliance {
            country,
            toward: other,
        };
        if state.validate_command(actor, &command).is_ok() {
            return Some((command, Reason::AllianceProposed));
        }
    }
    None
}

fn accepts_negative(common_threat: bool, rng: &mut SeededRng) -> bool {
    common_threat && rng.next_u64() % 10 == 0
}

/// Explainable factors for an AI country's response to an overlord offer.
pub fn vassal_acceptance_factors(
    state: &GameState,
    overlord: CountryId,
    subject: CountryId,
    profile: &FiscalAiProfile,
) -> Vec<AllianceAcceptanceFactor> {
    let army_strength = |country| -> i32 {
        let soldiers: u64 = state
            .war_state()
            .into_iter()
            .flat_map(|war| war.armies.values())
            .filter(|army| army.country == country)
            .map(|army| u64::from(army.soldiers))
            .sum();
        i32::try_from(soldiers).unwrap_or(i32::MAX)
    };
    vec![
        AllianceAcceptanceFactor {
            key: "effective_opinion",
            value: state
                .effective_opinion(subject, overlord)
                .map_or(0, |o| i32::from(o.points())),
        },
        AllianceAcceptanceFactor {
            key: "profile_minimum",
            value: profile.thresholds.vassal_min_opinion,
        },
        AllianceAcceptanceFactor {
            key: "overlord_soldiers",
            value: army_strength(overlord),
        },
        AllianceAcceptanceFactor {
            key: "subject_soldiers",
            value: army_strength(subject),
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use strategy_data::{load_scenario, parse_provinces};
    use strategy_sim::{
        derive_ai_rng, Army, ArmyId, Controller, DiplomacyState, EdgeKind, Opinion,
    };

    use super::*;

    fn scenario(opinion: i32) -> (GameState, CountryId, CountryId) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let graph =
            parse_provinces(&std::fs::read(root.join("data/map/europe/provinces.json")).unwrap())
                .unwrap();
        let state = load_scenario(graph, root.join("data/scenarios/1440.json")).unwrap();
        let ai: Vec<_> = state
            .countries()
            .filter_map(|(id, nation)| {
                matches!(nation.controller(), Controller::Ai(_)).then_some(id)
            })
            .collect();
        let (from, toward) = (ai[0], ai[1]);
        let state = state
            .with_opinions([(toward, from, Opinion::new(opinion).unwrap())])
            .unwrap();
        let mut diplomacy = DiplomacyState::default();
        diplomacy.offers.insert(
            (from, toward),
            state.time().date().checked_add_days(30).unwrap(),
        );
        (state.with_diplomacy(diplomacy).unwrap(), from, toward)
    }

    #[test]
    fn accepts_beneficial_offer_and_rejects_hostile_one_without_common_threat() {
        for (opinion, expected_accept) in [(60, true), (-60, false)] {
            let (state, from, toward) = scenario(opinion);
            let profile_key = match state.country(toward).unwrap().controller() {
                Controller::Ai(key) => key,
                _ => unreachable!(),
            };
            let profile = &state.polity().unwrap().ai_profiles[profile_key.as_str()];
            let factors = alliance_acceptance_factors(&state, from, toward, profile);
            assert_eq!(factors[0].value, opinion);
            let mut ctx = AiContext::new(&state, toward);
            let (command, _) = decide(&state, toward, profile, &mut ctx).unwrap();
            assert_eq!(
                matches!(command, Command::AcceptAlliance { .. }),
                expected_accept
            );
            assert!(state
                .validate_command(CommandActor::Ai(toward), &command)
                .is_ok());
        }
    }

    #[test]
    fn vassal_response_uses_profile_opinion_and_strict_army_advantage() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let graph =
            parse_provinces(&std::fs::read(root.join("data/map/europe/provinces.json")).unwrap())
                .unwrap();
        let scenario = load_scenario(graph, root.join("data/scenarios/1440.json")).unwrap();
        let ai: Vec<_> = scenario
            .countries()
            .filter_map(|(id, state)| matches!(state.controller(), Controller::Ai(_)).then_some(id))
            .collect();
        let (overlord, subject) = (ai[0], ai[1]);
        for (opinion, overlord_soldiers, subject_soldiers, accepts) in [
            (20, 100, 50, true),
            (19, 100, 50, false),
            (20, 50, 50, false),
        ] {
            let mut military = scenario.war_state().unwrap().clone();
            military.armies.clear();
            military.next_army_id = 3;
            for (id, country, soldiers) in [
                (1, overlord, overlord_soldiers),
                (2, subject, subject_soldiers),
            ] {
                military.armies.insert(
                    ArmyId::new(id),
                    Army {
                        country,
                        province: scenario.country_polity(country).unwrap().capital,
                        soldiers,
                        morale: military.rules.max_morale,
                        movement: None,
                    },
                );
            }
            let mut diplomacy = DiplomacyState::default();
            diplomacy.vassal_offers.insert(
                (overlord, subject),
                scenario.time().date().checked_add_days(30).unwrap(),
            );
            let state = GameState::new(
                *scenario.time(),
                scenario.seed(),
                scenario.province_graph().clone(),
                scenario
                    .countries()
                    .map(|(id, country)| (id, country.clone()))
                    .collect(),
                [],
            )
            .unwrap()
            .with_polity(scenario.polity().unwrap().clone())
            .unwrap()
            .with_war_state(military)
            .unwrap()
            .with_opinions([(subject, overlord, Opinion::new(opinion).unwrap())])
            .unwrap()
            .with_diplomacy(diplomacy)
            .unwrap();
            let profile_key = match state.country(subject).unwrap().controller() {
                Controller::Ai(key) => key,
                _ => unreachable!(),
            };
            let profile = &state.polity().unwrap().ai_profiles[profile_key.as_str()];
            assert_eq!(profile.thresholds.vassal_min_opinion, 20);
            let factors = vassal_acceptance_factors(&state, overlord, subject, profile);
            assert_eq!(factors[0].value, opinion);
            let mut context = AiContext::new(&state, subject);
            let (command, reason) = decide(&state, subject, profile, &mut context).unwrap();
            assert_eq!(matches!(command, Command::AcceptVassalage { .. }), accepts);
            assert_eq!(
                reason,
                if accepts {
                    Reason::VassalageAccepted
                } else {
                    Reason::VassalageRejected
                }
            );
        }
    }

    #[test]
    fn hostile_acceptance_is_seeded_and_requires_the_shared_threat() {
        let mut first = SeededRng::new(1440);
        let mut second = SeededRng::new(1440);
        let outcomes: Vec<_> = (0..1000)
            .map(|_| accepts_negative(true, &mut first))
            .collect();
        assert_eq!(
            outcomes,
            (0..1000)
                .map(|_| accepts_negative(true, &mut second))
                .collect::<Vec<_>>()
        );
        let accepted = outcomes.into_iter().filter(|&accepted| accepted).count();
        assert!(
            (50..=150).contains(&accepted),
            "expected about ten percent, observed {accepted}"
        );
        let mut absent = SeededRng::new(1440);
        assert!(!accepts_negative(false, &mut absent));
        assert_eq!(absent, SeededRng::new(1440));
    }

    #[test]
    fn ai_reaches_ten_percent_acceptance_when_both_border_a_stronger_country() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let graph =
            parse_provinces(&std::fs::read(root.join("data/map/europe/provinces.json")).unwrap())
                .unwrap();
        let scenario = load_scenario(graph, root.join("data/scenarios/1440.json")).unwrap();
        let ai: Vec<_> = scenario
            .countries()
            .filter_map(|(id, nation)| {
                matches!(nation.controller(), Controller::Ai(_)).then_some(id)
            })
            .collect();
        let border = |a, b| {
            let a_provinces = &scenario.country_polity(a).unwrap().provinces;
            let b_provinces = &scenario.country_polity(b).unwrap().provinces;
            a_provinces.iter().any(|&province| {
                scenario
                    .province_graph()
                    .neighbors(province)
                    .is_some_and(|edges| {
                        edges.iter().any(|edge| {
                            matches!(edge.kind, EdgeKind::Normal | EdgeKind::River)
                                && b_provinces.contains(&edge.to)
                        })
                    })
            })
        };
        let triple = ai.iter().copied().find_map(|from| {
            ai.iter()
                .copied()
                .filter(|&toward| toward != from)
                .find_map(|toward| {
                    scenario
                        .countries()
                        .map(|(id, _)| id)
                        .find(|&third| {
                            third != from
                                && third != toward
                                && border(from, third)
                                && border(toward, third)
                        })
                        .map(|third| (from, toward, third))
                })
        });
        let (from, toward, third) =
            triple.expect("1440 slice needs a shared land neighbor for the threat scenario");

        let mut military = scenario.war_state().unwrap().clone();
        military.armies.clear();
        military.wars.clear();
        military.sieges.clear();
        military.treaties.clear();
        military.next_army_id = 2;
        military.next_war_id = 1;
        military.armies.insert(
            ArmyId::new(1),
            Army {
                country: third,
                province: scenario.country_polity(third).unwrap().capital,
                soldiers: 100,
                morale: military.rules.max_morale,
                movement: None,
            },
        );
        let seed = (0..1000)
            .find(|&seed| {
                derive_ai_rng(seed, scenario.time().tick().get(), toward).next_u64() % 10 == 0
            })
            .unwrap();
        let base = GameState::new(
            *scenario.time(),
            seed,
            scenario.province_graph().clone(),
            scenario
                .countries()
                .map(|(id, country)| (id, country.clone()))
                .collect(),
            [],
        )
        .unwrap();
        let base = base
            .with_polity(scenario.polity().unwrap().clone())
            .unwrap()
            .with_war_state(military)
            .unwrap()
            .with_opinions([
                (toward, from, Opinion::new(-60).unwrap()),
                (from, toward, Opinion::new(60).unwrap()),
            ])
            .unwrap();
        let mut guarantee_state = base.clone();
        for _ in 0..31 {
            if (guarantee_state.time().tick().get() + u64::from(from.get())) % 31 == 0 {
                break;
            }
            guarantee_state.tick(|_, _| Vec::new()).unwrap();
        }
        let guarantee_key = match guarantee_state.country(from).unwrap().controller() {
            Controller::Ai(key) => key,
            _ => unreachable!(),
        };
        let guarantee_profile =
            &guarantee_state.polity().unwrap().ai_profiles[guarantee_key.as_str()];
        let mut guarantee_ctx = AiContext::new(&guarantee_state, from);
        let (guarantee_command, guarantee_reason) = decide(
            &guarantee_state,
            from,
            guarantee_profile,
            &mut guarantee_ctx,
        )
        .unwrap();
        assert!(
            matches!(&guarantee_command, Command::Guarantee { country, protected } if *country == from && guarantee_state.common_stronger_neighbor(*country, *protected))
        );
        assert_eq!(guarantee_reason, Reason::GuaranteeIssued);
        assert!(guarantee_state
            .validate_command(CommandActor::Ai(from), &guarantee_command)
            .is_ok());
        let mut diplomacy = DiplomacyState::default();
        diplomacy.offers.insert(
            (from, toward),
            base.time().date().checked_add_days(30).unwrap(),
        );
        let state = base.with_diplomacy(diplomacy).unwrap();
        assert!(state.common_stronger_neighbor(from, toward));
        let profile_key = match state.country(toward).unwrap().controller() {
            Controller::Ai(key) => key,
            _ => unreachable!(),
        };
        let profile = &state.polity().unwrap().ai_profiles[profile_key.as_str()];
        let mut ctx = AiContext::new(&state, toward);
        let (command, reason) = decide(&state, toward, profile, &mut ctx).unwrap();
        assert_eq!(
            command,
            Command::AcceptAlliance {
                country: toward,
                from
            }
        );
        assert_eq!(reason, Reason::AllianceAccepted);
        assert!(state
            .validate_command(CommandActor::Ai(toward), &command)
            .is_ok());
    }
}
