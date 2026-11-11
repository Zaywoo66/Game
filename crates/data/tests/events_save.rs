use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use serde_json::{json, Value};
use strategy_data::{
    load_game, parse_event_yaml, parse_provinces, save_game, state_hash, SaveError,
};
use strategy_sim::{
    Command, CommandActor, Controller, CountryId, CountryState, Date, EventError, GameState,
    ModifierStore, PlayerId, SimTime, Speed, StateChange,
};

const MAP: &[u8] = include_bytes!("fixtures/provinces.json");
const COUNTRY: CountryId = CountryId::new(1);

fn event_game() -> GameState {
    let mut event = parse_event_yaml(include_str!(
        "../../../tools/tests/fixtures/good_event/events/harvest.yaml"
    ))
    .unwrap();
    event.mean_time_to_happen = NonZeroU64::new(1);
    let graph = parse_provinces(MAP).unwrap();
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        1234,
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
    .with_events(BTreeMap::from([(event.key.clone(), event)]))
    .unwrap()
}

#[test]
fn pending_and_resolved_choices_survive_canonical_save_and_replay() {
    let mut state = event_game();
    let fired = state.tick(|_, _| Vec::new()).unwrap();
    assert!(fired
        .delta
        .changes()
        .iter()
        .any(|change| matches!(change, StateChange::EventFired { .. })));
    let pending = save_game(&state).unwrap();
    let document: Value = serde_json::from_slice(&pending).unwrap();
    assert_eq!(document["schema_version"], 8);
    assert_eq!(
        document["state"]["events"]["pending"][0]["key"],
        "harvest_festival"
    );
    let mut replay = load_game(&pending, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(state, replay);
    assert_eq!(save_game(&replay).unwrap(), pending);
    assert_eq!(state_hash(&replay).unwrap(), state_hash(&state).unwrap());

    let command = Command::ChooseEvent {
        country: COUNTRY,
        key: "harvest_festival".into(),
        option: 0,
    };
    state
        .apply(CommandActor::Host(PlayerId::new(1)), &command)
        .unwrap();
    replay
        .apply(CommandActor::Host(PlayerId::new(1)), &command)
        .unwrap();
    assert_eq!(state, replay);
    let resolved = save_game(&state).unwrap();
    assert_eq!(
        load_game(&resolved, parse_provinces(MAP).unwrap()).unwrap(),
        state
    );
}

#[test]
fn malformed_event_snapshot_is_rejected_without_reinterpretation() {
    let mut state = event_game();
    state.tick(|_, _| Vec::new()).unwrap();
    let mut document: Value = serde_json::from_slice(&save_game(&state).unwrap()).unwrap();
    document["state"]["events"]["pending"][0]["country"] = json!(999);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::InvalidEvent(EventError::InvalidSavedState))
    ));

    document["state"]["events"]["pending"][0]["country"] = json!(1);
    document["state"]["events"]["next_source_id"] = json!(0);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::InvalidEvent(_))
    ));
}
