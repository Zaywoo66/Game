use std::{fs, path::PathBuf};

use strategy_data::{load_scenario, parse_provinces};
use strategy_sim::{Command, CommandActor, CommandError, CountryId, IdeaError, PlayerId};

fn scenario() -> strategy_sim::GameState {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let map = fs::read(root.join("map/western_europe/provinces.json")).unwrap();
    let graph = parse_provinces(&map).unwrap();
    load_scenario(graph, root.join("scenarios/1440.json")).unwrap()
}

#[test]
fn english_idea_requires_unlock_and_spends_shared_points_once() {
    let mut state = scenario();
    let england = state
        .polity()
        .unwrap()
        .countries
        .iter()
        .find(|(_, polity)| polity.tag == "eng")
        .map(|(&id, _)| id)
        .unwrap();
    let actor = CommandActor::Host(PlayerId::new(1));
    let purchase = Command::UnlockIdea {
        country: england,
        key: "english_longbow".into(),
    };
    assert!(matches!(
        state.validate_command(actor, &purchase),
        Err(CommandError::Idea(IdeaError::Locked(_)))
    ));
    for _ in 0..1100 {
        state.tick(|_, _| vec![]).unwrap();
    }
    for key in ["recordkeeping", "professional_officers"] {
        state
            .apply(
                actor,
                &Command::UnlockTechnology {
                    country: england,
                    key: key.into(),
                },
            )
            .unwrap();
    }
    assert!(state.research_points(england).unwrap() < 100);
    assert!(matches!(
        state.validate_command(actor, &purchase),
        Err(CommandError::Idea(IdeaError::InsufficientPoints { .. }))
    ));
    for _ in 0..310 {
        state.tick(|_, _| vec![]).unwrap();
    }
    let points = state.research_points(england).unwrap();
    let delta = state.apply(actor, &purchase).unwrap();
    assert_eq!(state.research_points(england), Some(points - 100));
    assert!(delta.changes().iter().any(|change| matches!(change, strategy_sim::StateChange::IdeaUnlocked { country, key, .. } if *country == england && key == "english_longbow")));
    assert!(matches!(
        state.validate_command(actor, &purchase),
        Err(CommandError::Idea(IdeaError::AlreadyKnown(_)))
    ));
    assert!(matches!(
        state.validate_command(
            actor,
            &Command::UnlockIdea {
                country: CountryId::new(1),
                key: "english_longbow".into()
            }
        ),
        Err(CommandError::NotCountryController(_))
            | Err(CommandError::Idea(IdeaError::WrongCountry(_)))
    ));
}
