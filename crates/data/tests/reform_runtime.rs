use std::{fs, path::PathBuf};

use strategy_data::{load_game, load_scenario, parse_provinces, save_game, state_hash};
use strategy_sim::{Command, CommandActor, CommandError, PlayerId, ReformError};

#[test]
fn reform_purchase_is_one_per_tier_and_roundtrips() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let graph =
        parse_provinces(&fs::read(root.join("map/europe/provinces.json")).unwrap()).unwrap();
    let mut state = load_scenario(graph, root.join("scenarios/1440.json")).unwrap();
    let england = state
        .polity()
        .unwrap()
        .countries
        .iter()
        .find(|(_, country)| country.tag == "eng")
        .map(|(&id, _)| id)
        .unwrap();
    for _ in 0..520 {
        state.tick(|_, _| vec![]).unwrap();
    }
    let actor = CommandActor::Host(PlayerId::new(1));
    state
        .apply(
            actor,
            &Command::UnlockReform {
                country: england,
                key: "royal_council".into(),
            },
        )
        .unwrap();
    assert!(matches!(
        state.validate_command(
            actor,
            &Command::UnlockReform {
                country: england,
                key: "noble_privileges".into(),
            }
        ),
        Err(CommandError::Reform(ReformError::TierAlreadyChosen(1)))
    ));
    let bytes = save_game(&state).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("\"schema_version\":14"));
    let restored = load_game(&bytes, state.province_graph().clone()).unwrap();
    assert_eq!(state_hash(&state).unwrap(), state_hash(&restored).unwrap());
    assert_eq!(
        restored
            .reforms()
            .unwrap()
            .choices
            .get(&(england, 1))
            .map(String::as_str),
        Some("royal_council")
    );
}
