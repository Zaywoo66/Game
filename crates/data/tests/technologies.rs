use std::path::{Path, PathBuf};

use strategy_data::{
    load_technologies_from_dir, parse_technology_branch_yaml, TechnologyLoadError,
};
use strategy_sim::{Effect, Fixed, ModifierKind};

const MILITARY: &str = include_str!("../../../tools/tests/fixtures/good_tech/techs/military.yaml");

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn loads_cross_branch_prerequisites_and_typed_raw_fixed_effects() {
    let catalog = load_technologies_from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tests/fixtures/good_tech/techs"),
    )
    .unwrap();
    assert_eq!(
        catalog.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["professional_officers", "recordkeeping"]
    );
    let officers = &catalog["professional_officers"];
    assert_eq!(officers.branch, "military");
    assert_eq!(officers.cost.get(), 250);
    assert_eq!(officers.requires, ["recordkeeping"]);
    assert_eq!((officers.position.x, officers.position.y), (-2, 1));
    assert!(
        matches!(&officers.effects[0], Effect::AddModifier {value, kind: ModifierKind::Additive, ..} if *value == Fixed::from_raw(50))
    );
    assert!(
        matches!(&catalog["recordkeeping"].effects[0], Effect::AddModifier {value, kind: ModifierKind::Multiplicative, ..} if *value == Fixed::from_raw(100))
    );
}

#[test]
fn parser_rejects_wrong_branch_schema_keys_and_units() {
    assert!(
        matches!(parse_technology_branch_yaml(MILITARY, "commerce"), Err(TechnologyLoadError::Invalid { field, .. }) if field == "branch")
    );
    for (from, to) in [
        ("schema_version: 1", "schema_version: 2"),
        ("professional_officers", "professional__officers"),
        ("cost: 250", "cost: 0"),
        ("cost: 250", "cost: 0.5"),
        ("x: -2", "x: 2147483648"),
        ("value: 50", "value: 0.1"),
        ("value: 50", "value: \"10%\""),
        ("kind: additive", "kind: future"),
        ("add_modifier:", "future_effect:"),
    ] {
        let source = MILITARY.replace(from, to);
        assert!(
            parse_technology_branch_yaml(&source, "military").is_err(),
            "accepted {to}"
        );
    }
}

#[test]
fn parser_rejects_duplicate_fields_requirements_and_noncanonical_numbers() {
    for (from, to) in [
        ("cost: 250", "cost: 250\n    cost: 100"),
        ("[recordkeeping]", "[recordkeeping, recordkeeping]"),
        ("cost: 250", "cost: 0250"),
        ("cost: 250", "cost: 2_50"),
        ("cost: 250", "cost: +250"),
        ("cost: 250", "cost: 0xfa"),
    ] {
        let source = MILITARY.replace(from, to);
        assert!(
            parse_technology_branch_yaml(&source, "military").is_err(),
            "accepted {to}"
        );
    }
}

#[test]
fn loader_rejects_duplicate_branches_and_keys_missing_refs_and_cycles() {
    assert!(
        matches!(load_technologies_from_dir(&fixture_dir("tech_duplicate_branch")), Err(TechnologyLoadError::DuplicateBranch { branch, .. }) if branch == "military")
    );
    assert!(
        matches!(load_technologies_from_dir(&fixture_dir("tech_duplicate_key")), Err(TechnologyLoadError::DuplicateKey { key, .. }) if key == "recordkeeping")
    );
    assert!(
        matches!(load_technologies_from_dir(&fixture_dir("tech_missing")), Err(TechnologyLoadError::MissingRequirement { key, required }) if key == "military_drill" && required == "unknown_tech")
    );
    assert!(
        matches!(load_technologies_from_dir(&fixture_dir("tech_cycle")), Err(TechnologyLoadError::Cycle { keys }) if keys.len() == 3 && keys.first() == keys.last() && keys.contains(&"recordkeeping".to_owned()) && keys.contains(&"military_drill".to_owned()))
    );
}

#[test]
fn loader_accepts_empty_branch_but_rejects_malformed_yaml() {
    let catalog = load_technologies_from_dir(&fixture_dir("tech_empty")).unwrap();
    assert!(catalog.is_empty());
    assert!(matches!(
        load_technologies_from_dir(&fixture_dir("tech_invalid")),
        Err(TechnologyLoadError::InFile { .. })
    ));
    assert!(matches!(
        load_technologies_from_dir(&fixture_dir("tech_json")),
        Err(TechnologyLoadError::UnsupportedFormat { path }) if path.ends_with("military.json")
    ));
}
