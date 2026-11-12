use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use strategy_data::{
    load_game, parse_provinces, parse_technology_branch_yaml, save_game, state_hash, SaveError,
};
use strategy_sim::{
    Command, CommandActor, Controller, CountryId, CountryState, Date, GameState, ModifierStore,
    PlayerId, SimTime, Speed, TechnologyError,
};

const MAP: &[u8] = include_bytes!("fixtures/provinces.json");
const COUNTRY: CountryId = CountryId::new(1);

fn game() -> GameState {
    let graph = parse_provinces(MAP).unwrap();
    let definitions = parse_technology_branch_yaml(
        "schema_version: 1\nbranch: administration\ntechnologies:\n  - key: recordkeeping\n    cost: 10\n    requires: []\n    pos: {x: 0, y: 0}\n    effects:\n      - add_modifier: {key: tax_income, value: 100, kind: multiplicative}\n",
        "administration",
    ).unwrap().into_iter().map(|definition| (definition.key.clone(), definition.into())).collect();
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        44,
        graph,
        BTreeMap::from([(
            COUNTRY,
            CountryState::new(
                Controller::Human(PlayerId::new(1)),
                BTreeSet::new(),
                false,
                ModifierStore::default(),
            ),
        )]),
        [],
    )
    .unwrap()
    .with_technologies(definitions)
    .unwrap()
}

#[test]
fn purchase_save_roundtrip_and_replay() {
    let mut state = game();
    for _ in 0..31 {
        state.tick(|_, _| vec![]).unwrap();
    }
    state
        .apply(
            CommandActor::Host(PlayerId::new(1)),
            &Command::UnlockTechnology {
                country: COUNTRY,
                key: "recordkeeping".into(),
            },
        )
        .unwrap();
    let bytes = save_game(&state).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["schema_version"], 11);
    assert_eq!(
        document["state"]["technology"]["sources"][0]["key"],
        "recordkeeping"
    );
    let mut replay = load_game(&bytes, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(state, replay);
    assert_eq!(bytes, save_game(&replay).unwrap());
    for _ in 0..65 {
        assert_eq!(
            state.tick(|_, _| vec![]).unwrap(),
            replay.tick(|_, _| vec![]).unwrap()
        );
    }
    assert_eq!(state_hash(&state).unwrap(), state_hash(&replay).unwrap());
}

#[test]
fn rejects_invalid_saved_research_points_and_wrong_version() {
    let bytes = save_game(&game()).unwrap();
    let mut document: Value = serde_json::from_slice(&bytes).unwrap();
    document["state"]["technology"]["points"][0]["points"] = json!(1001);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::InvalidTechnology(
            TechnologyError::InvalidSavedState
        ))
    ));
    document["state"]["technology"]["points"][0]["points"] = json!(0);
    document["schema_version"] = json!(1);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::VersionTechnologyMismatch(1))
    ));
}
