use std::path::PathBuf;

use strategy_data::{load_ideas_from_dir, parse_ideas_yaml};

#[test]
fn packaged_ideas_have_unique_keys_and_permanent_modifiers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/ideas");
    let catalog = load_ideas_from_dir(&root).unwrap();
    assert!(catalog.contains_key("english_longbow"));
    assert!(catalog.contains_key("french_crown_lands"));
    assert!(catalog
        .values()
        .all(|idea| idea.modifiers.iter().all(|effect| {
            matches!(
                effect,
                strategy_sim::Effect::AddModifier {
                    duration_days: None,
                    ..
                }
            )
        })));
}

#[test]
fn invalid_idea_documents_fail_loading() {
    let valid = "schema_version: 1\ncountry: eng\nideas:\n  - key: longbow\n    name: longbow\n    unlock: {at_war: false}\n    modifiers:\n      - {key: army_morale, value: 100, kind: multiplicative}\n";
    assert_eq!(parse_ideas_yaml(valid, "eng").unwrap().len(), 1);
    for bad in [
        valid.replace("country: eng", "country: fra"),
        valid.replace("value: 100", "value: 0.1"),
        valid.replace("value: 100", "value: 100\n        duration_days: 30"),
        valid.replace("at_war: false", "at_war: yes"),
        valid.replace("kind: multiplicative", "kind: unknown"),
    ] {
        assert!(
            parse_ideas_yaml(&bad, "eng").is_err(),
            "accepted invalid idea YAML: {bad}"
        );
    }
}
