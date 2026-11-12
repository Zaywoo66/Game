use std::fs;
use std::path::PathBuf;

use strategy_ai::{decide, AiContext, Reason};
use strategy_data::{load_scenario, parse_provinces, state_hash};
use strategy_sim::{Command, Controller};

fn scenario() -> strategy_sim::GameState {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let map = fs::read(root.join("map/western_europe/provinces.json")).unwrap();
    let graph = parse_provinces(&map).unwrap();
    load_scenario(graph, root.join("scenarios/1440.json")).unwrap()
}

#[test]
fn ai_buys_technologies_by_profile_through_validated_commands() {
    let mut a = scenario();
    let mut b = a.clone();
    assert!(a.technologies().is_some());
    for _ in 0..400 {
        for state in [&mut a, &mut b] {
            let report = state
                .tick(|state, country| {
                    decide(state, country, &mut AiContext::new(state, country))
                        .into_iter()
                        .filter(|(command, reason)| {
                            !matches!(reason, Reason::Technology)
                                || matches!(command, Command::UnlockTechnology { .. })
                        })
                        .map(|(command, _)| command)
                        .collect()
                })
                .unwrap();
            assert!(report
                .ai_rejections
                .iter()
                .all(|rejection| !matches!(rejection.command, Command::UnlockTechnology { .. })));
        }
    }
    assert_eq!(state_hash(&a).unwrap(), state_hash(&b).unwrap());
    let ai_countries: Vec<_> = a
        .countries()
        .filter(|(_, state)| matches!(state.controller(), Controller::Ai(_)))
        .collect();
    assert!(!ai_countries.is_empty());
    assert!(ai_countries
        .iter()
        .all(|(_, state)| state.known_technologies().any(|key| key == "recordkeeping")));
}
