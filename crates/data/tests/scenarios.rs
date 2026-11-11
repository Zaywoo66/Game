use std::{fs, io, path::PathBuf};

use serde_json::{json, Value};
use strategy_data::{
    load_game, load_provinces, load_scenario, load_scenario_from_reader, save_game, state_hash,
    SaveError, ScenarioLoadError,
};
use strategy_sim::{Controller, GameState, ProvinceGraph, ProvinceKind};

fn data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn graph() -> ProvinceGraph {
    load_provinces(data_root().join("map/western_europe/provinces.json")).unwrap()
}

fn original() -> GameState {
    load_scenario(graph(), data_root().join("scenarios/1440.json")).unwrap()
}

fn edited_file(path: &str, edit: impl FnOnce(&mut Value)) -> Result<GameState, ScenarioLoadError> {
    let root = data_root();
    let mut value: Value = serde_json::from_slice(&fs::read(root.join(path)).unwrap()).unwrap();
    edit(&mut value);
    let replacement = serde_json::to_vec(&value).unwrap();
    load_scenario_from_reader(graph(), "1440", |logical| {
        if logical == path {
            Ok(replacement.clone())
        } else {
            fs::read(root.join(logical))
        }
    })
}

fn assert_invalid(result: Result<GameState, ScenarioLoadError>, expected: &str) {
    let error = result.unwrap_err();
    assert!(error.to_string().contains(expected), "{error}");
}

#[test]
fn western_europe_loads_six_countries_with_neutral_land() {
    let state = original();
    let polity = state.polity().unwrap();
    assert_eq!(polity.countries.len(), 6);
    assert_eq!(polity.characters.len(), 13);
    assert_eq!(polity.ai_profiles.len(), 2);
    assert_eq!(polity.rules.base_taxable_development_per_province, 1);
    assert_eq!(state.time().date().components(), (1440, 1, 1));

    let owned = polity
        .countries
        .values()
        .map(|country| country.provinces.len())
        .sum::<usize>();
    let land = state
        .province_graph()
        .iter()
        .filter(|province| province.kind == ProvinceKind::Land)
        .count();
    assert_eq!((owned, land - owned), (24, 18));

    let (&england, england_polity) = polity
        .countries
        .iter()
        .find(|(_, country)| country.tag == "eng")
        .unwrap();
    assert!(
        matches!(state.country(england).unwrap().controller(), Controller::Human(id) if id.get() == 1)
    );
    assert_eq!(
        state
            .province_graph()
            .get(england_polity.capital)
            .unwrap()
            .key,
        "london"
    );
    assert_eq!(
        state
            .countries()
            .filter(|(_, facts)| matches!(facts.controller(), Controller::Ai(_)))
            .count(),
        5
    );

    let scotland = polity
        .countries
        .values()
        .find(|country| country.tag == "sco")
        .unwrap();
    let ruler = &polity.characters[&scotland.succession.ruler.unwrap()];
    assert_eq!(ruler.birth.components(), (1430, 10, 16));
    assert!(scotland.succession.heir.is_none());
}

#[test]
fn reference_order_does_not_change_ids_and_all_ai_mode_is_available() {
    let shuffled = edited_file("scenarios/1440.json", |scenario| {
        scenario["countries"].as_array_mut().unwrap().reverse();
        scenario["characters"].as_array_mut().unwrap().reverse();
    })
    .unwrap();
    assert_eq!(shuffled, original());
    let all_ai = edited_file("scenarios/1440.json", |scenario| {
        scenario["human_slots"] = json!({});
    })
    .unwrap();
    assert!(all_ai
        .countries()
        .all(|(_, country)| matches!(country.controller(), Controller::Ai(_))));
}

#[test]
fn rejects_shared_unknown_and_water_provinces_and_unowned_capitals() {
    for (province, message) in [
        ("aragon", "claimed more than once"),
        ("nowhere", "unknown province"),
        ("north_sea", "is not land"),
    ] {
        assert_invalid(
            edited_file("countries/eng.json", |country| {
                country["start"]["provinces"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(province));
            }),
            message,
        );
    }
    assert_invalid(
        edited_file("countries/eng.json", |country| {
            country["capital"] = json!("leinster");
        }),
        "capital must be owned",
    );
}

#[test]
fn rejects_cross_country_unborn_and_duplicate_ruler_roles() {
    for (heir, message) in [
        ("charles_vii", "belongs to another country"),
        ("edward_iv", "not alive"),
        ("henry_vi", "same character"),
        ("absent_person", "unknown ruler/heir"),
    ] {
        assert_invalid(
            edited_file("countries/eng.json", |country| {
                country["start"]["heir"] = json!(heir);
            }),
            message,
        );
    }
    assert_invalid(
        edited_file("characters/henry_vi.json", |character| {
            character["death"] = json!([1400, 1, 1]);
        }),
        "death must be after birth",
    );
}

#[test]
fn rejects_missing_profiles_unknown_traits_and_out_of_bound_values() {
    assert_invalid(
        edited_file("countries/eng.json", |country| {
            country["ai_profile"] = json!("missing");
        }),
        "ai/profiles/missing.json",
    );
    assert_invalid(
        edited_file("characters/henry_vi.json", |character| {
            character["traits"] = json!(["absent_trait"]);
        }),
        "unknown trait",
    );
    assert_invalid(
        edited_file("ai/profiles/balanced.json", |profile| {
            profile["weights"]["economy"] = json!(1001);
        }),
        "InvalidProfile",
    );
    assert_invalid(
        edited_file("countries/eng.json", |country| {
            country["start"]["tax_rate"] = json!(501);
        }),
        "InvalidFiscalPolicy",
    );
}

#[test]
fn rejects_duplicate_human_slots_and_paths_outside_the_resolver_contract() {
    assert_invalid(
        edited_file("scenarios/1440.json", |scenario| {
            scenario["human_slots"]["fra"] = json!(1);
        }),
        "duplicate human player slot",
    );

    assert_invalid(
        load_scenario_from_reader(graph(), "../1440", |_| {
            panic!("invalid scenario key must not reach the resolver");
        }),
        "positive decimal year",
    );
    assert_invalid(
        edited_file("scenarios/1440.json", |scenario| {
            scenario["countries"] = json!(["../eng"]);
        }),
        "invalid country key",
    );

    let root = data_root();
    let bytes = fs::read_to_string(root.join("scenarios/1440.json")).unwrap();
    let duplicate = bytes.replace("\"eng\": 1", "\"eng\": 1, \"eng\": 2");
    assert_ne!(duplicate, bytes);
    assert_invalid(
        load_scenario_from_reader(graph(), "1440", |path| -> io::Result<Vec<u8>> {
            if path == "scenarios/1440.json" {
                Ok(duplicate.as_bytes().to_vec())
            } else {
                fs::read(root.join(path))
            }
        }),
        "duplicate object key",
    );
}

#[test]
fn save_v2_round_trips_country_economy_characters_profiles_and_rules() {
    let mut state = original();
    for _ in 0..62 {
        state.tick(|_, _| Vec::new()).unwrap();
    }
    assert!(state
        .polity()
        .unwrap()
        .countries
        .values()
        .all(|country| country.economy.last_month.is_some()));
    let bytes = save_game(&state).unwrap();
    let fields: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(fields["schema_version"], 2);
    assert!(fields["state"]["polity"].is_object());

    let restored = load_game(&bytes, graph()).unwrap();
    assert_eq!(restored, state);
    assert_eq!(state_hash(&restored).unwrap(), state_hash(&state).unwrap());
    assert_eq!(save_game(&restored).unwrap(), bytes);
}

#[test]
fn v2_rejects_wrong_envelope_missing_polity_and_invalid_capital() {
    let mut save: Value = serde_json::from_slice(&save_game(&original()).unwrap()).unwrap();
    save["schema_version"] = json!(1);
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::VersionPolityMismatch(1))
    ));

    save["schema_version"] = json!(2);
    save["state"]["polity"]["countries"][0]["capital"] = json!(0);
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::InvalidPolity(_))
    ));

    save["state"].as_object_mut().unwrap().remove("polity");
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::VersionPolityMismatch(2))
    ));
}
