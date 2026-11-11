use std::collections::BTreeSet;

use strategy_ai::{decide, AiContext};
use strategy_app::load_embedded_scenario;
use strategy_sim::{AiProfileKey, Command, CommandActor, Controller, PlayerId};

#[test]
fn ai_lands_an_army_across_the_sea_without_invalid_naval_commands() {
    let mut state = load_embedded_scenario().unwrap();
    let countries: Vec<_> = state.countries().map(|(id, _)| id).collect();
    let england = countries
        .iter()
        .copied()
        .find(|&id| {
            state
                .country_polity(id)
                .is_some_and(|nation| nation.tag == "eng")
        })
        .unwrap();
    let scotland = countries
        .iter()
        .copied()
        .find(|&id| {
            state
                .country_polity(id)
                .is_some_and(|nation| nation.tag == "sco")
        })
        .unwrap();
    let belfast = state.province_graph().by_key("belfast").unwrap().id;
    let host = CommandActor::Host(PlayerId::new(1));
    state
        .apply(
            host,
            &Command::DeclareWar {
                country: england,
                defender: scotland,
                goal: belfast,
            },
        )
        .unwrap();
    for country in countries {
        let controller = if country == england {
            let key = state.country_polity(country).unwrap().ai_profile.as_str();
            Controller::Ai(AiProfileKey::new(key))
        } else {
            Controller::Human(PlayerId::new(2))
        };
        state
            .apply(
                host,
                &Command::SetController {
                    country,
                    controller,
                },
            )
            .unwrap();
    }
    let mut reasons = BTreeSet::new();
    let mut landed = false;
    for _ in 0..500 {
        let report = state
            .tick(|view, country| {
                let mut context = AiContext::new(view, country);
                decide(view, country, &mut context)
                    .into_iter()
                    .map(|(command, reason)| {
                        reasons.insert(reason.label());
                        command
                    })
                    .collect()
            })
            .unwrap();
        assert!(
            report.ai_rejections.is_empty(),
            "{:?}",
            report.ai_rejections
        );
        landed = state
            .war_state()
            .unwrap()
            .armies
            .values()
            .any(|army| army.country == england && army.province == belfast);
        if landed {
            break;
        }
    }
    assert!(
        landed,
        "AI did not land at Belfast within 500 days: {reasons:?}"
    );
    assert!(reasons.contains("EmbarkArmy"));
    assert!(reasons.contains("SailToGoal"));
    assert!(reasons.contains("LandArmy"));
}
