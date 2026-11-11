use std::path::Path;

use strategy_data::{load_events_from_dir, parse_event_yaml, EventLoadError};
use strategy_sim::{Effect, Fixed, ModifierKind, Trigger};

const VALID: &str = include_str!("../../../tools/tests/fixtures/good_event/events/harvest.yaml");

fn fixture_dir(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/tests/fixtures")
        .join(name)
        .join("events")
}

#[test]
fn parses_event_contract_into_sim_types_and_key_references() {
    let event = parse_event_yaml(VALID).unwrap();
    assert_eq!(event.key, "harvest_festival");
    assert_eq!(event.title_key, "event_harvest_title");
    assert_eq!(event.desc_key, "event_harvest_desc");
    assert_eq!(event.mean_time_to_happen.unwrap().get(), 365);
    assert!(event.repeatable);
    assert_eq!(event.cooldown_days.unwrap().get(), 365);
    assert_eq!(event.trigger, Trigger::Not(Box::new(Trigger::AtWar(true))));
    assert_eq!(event.options[0].text_key, "event_harvest_accept");
    assert_eq!(event.options[0].ai_weight, 10);
    assert_eq!(event.options[0].ai_weight_if.as_ref().unwrap().add, 5);
    assert_eq!(
        event.options[0].ai_weight_if.as_ref().unwrap().when,
        Trigger::CountryHasTech("agrarian_reform".into())
    );
    assert!(matches!(
        &event.options[0].effects[0],
        Effect::AddModifier {
            value,
            kind: ModifierKind::Multiplicative,
            duration_days: Some(days),
            ..
        } if *value == Fixed::from_raw(100) && days.get() == 30
    ));
    assert!(event.options[1].effects.is_empty());
}

#[test]
fn loads_files_in_key_order_and_rejects_duplicate_keys() {
    let loaded = load_events_from_dir(&fixture_dir("good_event")).unwrap();
    assert_eq!(
        loaded.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["harvest_festival"]
    );

    let error = load_events_from_dir(&fixture_dir("duplicate_events")).unwrap_err();
    assert!(matches!(error, EventLoadError::DuplicateKey { key, .. } if key == "duplicate_event"));

    let invalid = load_events_from_dir(&fixture_dir("bad_event")).unwrap_err();
    assert!(
        matches!(invalid, EventLoadError::InFile { path, .. } if path.ends_with("invalid.yaml"))
    );
}

#[test]
fn rejects_unknown_trigger_effect_and_missing_ai_weight() {
    let unknown_trigger = VALID.replace("not: {at_war: true}", "future_trigger: true");
    assert!(matches!(
        parse_event_yaml(&unknown_trigger),
        Err(EventLoadError::Invalid { reason, .. }) if reason.contains("unknown trigger")
    ));

    let unknown_effect = VALID.replace("add_modifier:", "future_effect:");
    assert!(matches!(
        parse_event_yaml(&unknown_effect),
        Err(EventLoadError::Invalid { reason, .. }) if reason.contains("unknown effect")
    ));

    let missing_weight = VALID
        .lines()
        .filter(|line| !line.contains("ai_weight: 10"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        parse_event_yaml(&missing_weight),
        Err(EventLoadError::Yaml(_))
    ));
}

#[test]
fn rejects_invalid_units_keys_and_repeat_rules() {
    let fractional = VALID.replace("mean_time_to_happen: 365", "mean_time_to_happen: 0.5");
    assert!(parse_event_yaml(&fractional).is_err());

    let bad_key = VALID.replace("key: harvest_festival", "key: harvest__festival");
    assert!(matches!(
        parse_event_yaml(&bad_key),
        Err(EventLoadError::Invalid { field, .. }) if field == "key"
    ));

    let missing_cooldown = VALID
        .lines()
        .filter(|line| !line.contains("cooldown_days: 365"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        parse_event_yaml(&missing_cooldown),
        Err(EventLoadError::Invalid { field, .. }) if field == "cooldown_days"
    ));

    let all_zero = VALID
        .replace("ai_weight: 10", "ai_weight: 0")
        .replace("ai_weight: 1", "ai_weight: 0");
    assert!(matches!(
        parse_event_yaml(&all_zero),
        Err(EventLoadError::Invalid { field, .. }) if field == "options"
    ));
}
