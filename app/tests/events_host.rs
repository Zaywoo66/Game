use std::collections::BTreeMap;
use std::num::NonZeroU64;

use strategy_app::{load_embedded_scenario, tick_host};
use strategy_data::{load_game, load_scenario, parse_event_yaml, parse_provinces, save_game};
use strategy_sim::{Controller, Trigger};

#[test]
fn host_ai_resolves_pending_event_by_legal_command() {
    let mut event = parse_event_yaml(include_str!(
        "../../tools/tests/fixtures/good_event/events/harvest.yaml"
    ))
    .unwrap();
    event.trigger = Trigger::All(Vec::new());
    event.mean_time_to_happen = NonZeroU64::new(1);
    let graph = parse_provinces(include_bytes!("../../data/map/europe/provinces.json")).unwrap();
    let scenario =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/scenarios/1440.json");
    let mut state = load_scenario(graph, scenario)
        .unwrap()
        .with_events(BTreeMap::from([(event.key.clone(), event)]))
        .unwrap();
    let ai = state
        .countries()
        .filter(|(_, country)| matches!(country.controller(), Controller::Ai(_)))
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    assert!(!ai.is_empty());
    let first = tick_host(&mut state).unwrap();
    assert!(first.ai_rejections.is_empty());
    assert!(ai.iter().all(|country| state
        .events()
        .unwrap()
        .pending
        .contains_key(&(*country, "harvest_festival".into()))));
    let snapshot = save_game(&state).unwrap();
    let version: serde_json::Value = serde_json::from_slice(&snapshot).unwrap();
    // The 1440 scenario includes year-precision historical births, introduced
    // with save schema v16; this event-only state has no v17 call-to-arms data.
    assert_eq!(version["schema_version"], 16);
    assert_eq!(
        load_game(&snapshot, state.province_graph().clone()).unwrap(),
        state
    );
    let second = tick_host(&mut state).unwrap();
    assert!(
        second.ai_rejections.is_empty(),
        "{:?}",
        second.ai_rejections
    );
    assert!(ai.iter().all(|country| !state
        .events()
        .unwrap()
        .pending
        .contains_key(&(*country, "harvest_festival".into()))));
    assert!(ai.iter().all(|country| state
        .events()
        .unwrap()
        .resolved
        .contains_key(&(*country, "harvest_festival".into()))));
}

#[test]
fn embedded_scenario_contains_production_events() {
    let state = load_embedded_scenario().unwrap();
    let events = state.events().unwrap();
    assert_eq!(events.definitions.len(), 20);
    for key in [
        "court_auditors",
        "frontier_patrol",
        "harvest_surplus",
        "market_charter",
        "veteran_captains",
        "storehouse_audit",
    ] {
        assert!(events.definitions.contains_key(key), "missing {key}");
    }
}
