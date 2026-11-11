use strategy_sim::{CharacterId, Command, CommandActor, CountryId, FiscalAiProfile, GameState};

use crate::Reason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarriageAcceptanceFactor {
    pub key: &'static str,
    pub value: i32,
}

/// Identical numeric factors for AI decisions and the UI preview.
pub fn marriage_acceptance_factors(
    state: &GameState,
    from: CountryId,
    character: CharacterId,
    toward: CountryId,
    partner: CharacterId,
    profile: &FiscalAiProfile,
) -> Vec<MarriageAcceptanceFactor> {
    vec![
        MarriageAcceptanceFactor {
            key: "effective_opinion",
            value: state
                .effective_opinion(toward, from)
                .map_or(0, |value| i32::from(value.points())),
        },
        MarriageAcceptanceFactor {
            key: "profile_minimum",
            value: profile.thresholds.marriage_min_opinion,
        },
        MarriageAcceptanceFactor {
            key: "legal_pair",
            value: i32::from(
                state
                    .marriage_eligibility(from, character, toward, partner)
                    .is_ok(),
            ),
        },
    ]
}

pub(super) fn decide(
    state: &GameState,
    country: CountryId,
    profile: &FiscalAiProfile,
) -> Option<(Command, Reason)> {
    let actor = CommandActor::Ai(country);
    let polity = state.polity()?;
    if let Some(offer) = state.diplomacy().and_then(|diplomacy| {
        diplomacy
            .marriage_offers_to(&polity.characters, country)
            .next()
    }) {
        let from = state.character(offer.from)?.country;
        let factors =
            marriage_acceptance_factors(state, from, offer.from, country, offer.toward, profile);
        let accept = factors[2].value == 1 && factors[0].value >= factors[1].value;
        let command = if accept {
            Command::AcceptRoyalMarriage {
                country,
                from,
                character: offer.from,
                partner: offer.toward,
            }
        } else {
            Command::RejectRoyalMarriage {
                country,
                from,
                character: offer.from,
                partner: offer.toward,
            }
        };
        return state.validate_command(actor, &command).is_ok().then_some((
            command,
            if accept {
                Reason::MarriageAccepted
            } else {
                Reason::MarriageRejected
            },
        ));
    }
    if (state.time().tick().get() + u64::from(country.get())) % 31 != 0 {
        return None;
    }
    let mut candidates = Vec::new();
    for (&character, person) in &polity.characters {
        if person.country != country {
            continue;
        }
        for (&partner, other_person) in &polity.characters {
            let toward = other_person.country;
            if toward == country {
                continue;
            }
            if state
                .marriage_eligibility(country, character, toward, partner)
                .is_err()
            {
                continue;
            }
            let opinion = state
                .effective_opinion(country, toward)
                .map_or(0, |value| i32::from(value.points()));
            if opinion < profile.thresholds.marriage_min_opinion {
                continue;
            }
            let command = Command::OfferRoyalMarriage {
                country,
                character,
                toward,
                partner,
            };
            if state.validate_command(actor, &command).is_ok() {
                candidates.push((opinion, command));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .into_iter()
        .next()
        .map(|(_, command)| (command, Reason::MarriageProposed))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::Path};

    use strategy_data::{load_scenario, parse_provinces};
    use strategy_sim::{CharacterId, CountryId, DiplomacyState, GameState, Opinion, Sex};

    use super::*;

    fn scenario(points: i32) -> (GameState, CountryId, CharacterId, CountryId, CharacterId) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let graph = parse_provinces(
            &std::fs::read(root.join("data/map/western_europe/provinces.json")).unwrap(),
        )
        .unwrap();
        let original = load_scenario(graph.clone(), root.join("data/scenarios/1440.json")).unwrap();
        let find = |tag: &str| {
            original
                .polity()
                .unwrap()
                .countries
                .iter()
                .find_map(|(&id, nation)| (nation.tag == tag).then_some(id))
                .unwrap()
        };
        let from = find("fra");
        let toward = find("cas");
        let mut polity = original.polity().unwrap().clone();
        let character = polity.countries[&from].succession.ruler.unwrap();
        let partner = polity.countries[&toward].succession.ruler.unwrap();
        polity.characters.get_mut(&partner).unwrap().sex = Sex::Female;
        let state = GameState::new(
            *original.time(),
            original.seed(),
            graph,
            original
                .countries()
                .map(|(id, country)| (id, country.clone()))
                .collect::<BTreeMap<_, _>>(),
            [],
        )
        .unwrap()
        .with_polity(polity)
        .unwrap()
        .with_opinions([(toward, from, Opinion::new(points).unwrap())])
        .unwrap();
        let mut diplomacy = DiplomacyState::default();
        diplomacy.marriage_offers.insert(
            (character, partner),
            state.time().date().checked_add_days(30).unwrap(),
        );
        (
            state.with_diplomacy(diplomacy).unwrap(),
            from,
            character,
            toward,
            partner,
        )
    }

    #[test]
    fn ai_uses_explainable_opinion_threshold_for_marriage() {
        for (points, accepted) in [(25, true), (-25, false)] {
            let (state, from, character, toward, partner) = scenario(points);
            let profile_key = state.country_polity(toward).unwrap().ai_profile.as_str();
            let profile = &state.polity().unwrap().ai_profiles[profile_key];
            let factors =
                marriage_acceptance_factors(&state, from, character, toward, partner, profile);
            assert_eq!(factors[0].value, points);
            assert_eq!(factors[1].value, 20);
            assert_eq!(factors[2].value, 1);
            let (command, _) = decide(&state, toward, profile).unwrap();
            assert_eq!(
                matches!(command, Command::AcceptRoyalMarriage { .. }),
                accepted
            );
            assert!(state
                .validate_command(CommandActor::Ai(toward), &command)
                .is_ok());
        }
    }

    #[test]
    fn ai_proposes_to_highest_opinion_legal_country_on_l2_cadence() {
        let (mut state, from, character, toward, partner) = scenario(25);
        state
            .apply(
                CommandActor::Ai(toward),
                &Command::RejectRoyalMarriage {
                    country: toward,
                    from,
                    character,
                    partner,
                },
            )
            .unwrap();
        while (state.time().tick().get() + u64::from(toward.get())) % 31 != 0 {
            state.tick(|_, _| Vec::new()).unwrap();
        }
        let profile_key = state.country_polity(toward).unwrap().ai_profile.as_str();
        let profile = &state.polity().unwrap().ai_profiles[profile_key];
        let (command, reason) = decide(&state, toward, profile).unwrap();
        assert_eq!(
            command,
            Command::OfferRoyalMarriage {
                country: toward,
                character: partner,
                toward: from,
                partner: character
            }
        );
        assert_eq!(reason, Reason::MarriageProposed);
        assert!(state
            .validate_command(CommandActor::Ai(toward), &command)
            .is_ok());
    }
}
