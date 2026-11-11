use std::collections::BTreeMap;
use std::num::NonZeroU64;

use strategy_app::{load_embedded_scenario, tick_host};
use strategy_data::parse_event_yaml;
use strategy_sim::{Controller, Trigger};

#[test]
fn host_ai_resolves_pending_event_by_legal_command() {
    let mut event = parse_event_yaml(include_str!(
        "../../tools/tests/fixtures/good_event/events/harvest.yaml"
    ))
    .unwrap();
    event.trigger = Trigger::All(Vec::new());
    event.mean_time_to_happen = NonZeroU64::new(1);
    let mut state = load_embedded_scenario()
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
    assert!(ai.iter().all(|country| state.events().unwrap().pending.contains_key(&(*country, "harvest_festival".into()))));
    let second = tick_host(&mut state).unwrap();
    assert!(second.ai_rejections.is_empty(), "{:?}", second.ai_rejections);
    assert!(ai.iter().all(|country| !state.events().unwrap().pending.contains_key(&(*country, "harvest_festival".into()))));
    assert!(ai.iter().all(|country| state.events().unwrap().resolved.contains_key(&(*country, "harvest_festival".into()))));
}
