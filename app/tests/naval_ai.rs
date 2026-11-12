use std::collections::BTreeSet;

use strategy_ai::{decide, AiContext};
use strategy_app::load_embedded_scenario;
use strategy_sim::{AiProfileKey, Command, CommandActor, Controller, PlayerId};

#[test]
fn ai_lands_an_army_across_the_sea_without_invalid_naval_commands() {
    let mut state = load_embedded_scenario().unwrap();
    let countries: Vec<_> = state.countries().map(|(id, _)| id).collect();
    let portugal = countries
        .iter()
        .copied()
        .find(|&id| {
            state
                .country_polity(id)
                .is_some_and(|nation| nation.tag == "por")
        })
        .unwrap();
    let aragon = countries
        .iter()
        .copied()
        .find(|&id| {
            state
                .country_polity(id)
                .is_some_and(|nation| nation.tag == "ara")
        })
        .unwrap();
    let valencia = state.province_graph().by_key("valencia").unwrap().id;
    let host = CommandActor::Host(PlayerId::new(1));
    state
        .apply(
            CommandActor::Ai(portugal),
            &Command::DeclareWar {
                country: portugal,
                defender: aragon,
                goal: valencia,
            },
        )
        .unwrap();
    for country in countries {
        let controller = if country == portugal {
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
            .any(|army| army.country == portugal && army.province == valencia);
        if landed {
            break;
        }
    }
    assert!(
        landed,
        "AI did not land at Valencia within 500 days: {reasons:?}"
    );
    assert!(reasons.contains("EmbarkArmy"));
    assert!(reasons.contains("SailToGoal"));
    assert!(reasons.contains("LandArmy"));
}
