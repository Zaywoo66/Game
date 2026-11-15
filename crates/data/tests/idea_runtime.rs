use std::{fs, path::PathBuf};

use strategy_data::{load_game, load_scenario, parse_provinces, save_game, state_hash};
use strategy_sim::{Command, CommandActor, CommandError, CountryId, IdeaError, PlayerId};

fn scenario() -> strategy_sim::GameState {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let map = fs::read(root.join("map/europe/provinces.json")).unwrap();
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
    for key in [
        "digital_government",
        "professional_forces",
        "industrial_productivity",
    ] {
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
    let bytes = save_game(&state).unwrap();
    let version: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(version["schema_version"].as_u64().unwrap() >= 22);
    let restored = load_game(&bytes, state.province_graph().clone()).unwrap();
    assert_eq!(state_hash(&restored).unwrap(), state_hash(&state).unwrap());
    assert!(restored
        .ideas()
        .unwrap()
        .sources
        .contains_key(&(england, "english_longbow".into())));
}

#[test]
fn common_ideas_are_fallback_only_for_countries_without_unique_catalogs() {
    let state = scenario();
    let find = |tag: &str| {
        state
            .polity()
            .unwrap()
            .countries
            .iter()
            .find(|(_, country)| country.tag == tag)
            .map(|(&id, _)| id)
            .unwrap()
    };
    let portugal = find("por");
    let france = find("fra");
    let common_purchase = |country| Command::UnlockIdea {
        country,
        key: "common_infrastructure".into(),
    };
    assert!(matches!(
        state.validate_command(CommandActor::Ai(portugal), &common_purchase(portugal)),
        Err(CommandError::Idea(IdeaError::Locked(_)))
    ));
    assert!(matches!(
        state.validate_command(CommandActor::Ai(france), &common_purchase(france)),
        Err(CommandError::Idea(IdeaError::WrongCountry(_)))
    ));
}
