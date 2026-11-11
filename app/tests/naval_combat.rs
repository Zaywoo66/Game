use strategy_app::load_embedded_scenario;
use strategy_sim::{Command, CommandActor, StateChange};

#[test]
fn wartime_fleets_meet_at_sea_in_the_1440_scenario() {
    let mut state = load_embedded_scenario().unwrap();
    let portugal = state
        .countries()
        .find_map(|(id, _)| (state.country_polity(id)?.tag == "por").then_some(id))
        .unwrap();
    let aragon = state
        .countries()
        .find_map(|(id, _)| (state.country_polity(id)?.tag == "ara").then_some(id))
        .unwrap();
    let goal = state.province_graph().by_key("valencia").unwrap().id;
    let attacker = state
        .navy()
        .unwrap()
        .fleets
        .iter()
        .find_map(|(&id, fleet)| (fleet.country == portugal).then_some(id))
        .unwrap();
    let defender_sea = state
        .navy()
        .unwrap()
        .fleets
        .values()
        .find_map(|fleet| (fleet.country == aragon).then_some(fleet.sea))
        .unwrap();
    state
        .apply(
            CommandActor::Ai(portugal),
            &Command::DeclareWar {
                country: portugal,
                defender: aragon,
                goal,
            },
        )
        .unwrap();
    state
        .apply(
            CommandActor::Ai(portugal),
            &Command::MoveFleet {
                country: portugal,
                fleet: attacker,
                destination: defender_sea,
            },
        )
        .unwrap();

    let mut battle = None;
    for _ in 0..100 {
        let report = state.tick(|_, _| Vec::new()).unwrap();
        battle = report.delta.changes.into_iter().find_map(|change| {
            if let StateChange::NavalBattleResolved(result) = change {
                Some(result)
            } else {
                None
            }
        });
        if battle.is_some() {
            break;
        }
    }
    let battle = battle.expect("moving fleet never engaged the wartime opponent");
    assert_eq!(battle.sea, defender_sea);
    assert_eq!((battle.attacker_losses, battle.defender_losses), (1, 1));
}
