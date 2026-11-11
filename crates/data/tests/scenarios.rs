use std::{collections::BTreeSet, fs, io, path::PathBuf};

use serde_json::{json, Value};
use strategy_data::{
    load_game, load_provinces, load_scenario, load_scenario_from_reader, parse_provinces,
    save_game, state_hash, SaveError, ScenarioLoadError,
};
use strategy_sim::{
    Controller, DiplomacyError, DiplomacyState, GameState, Opinion, ProvinceGraph, ProvinceKind,
};

fn load_with_opinions(opinions: Value) -> Result<GameState, ScenarioLoadError> {
    load_scenario_from_reader(graph(), "1440", |logical| {
        let bytes = legacy_read(logical)?;
        if logical != "scenarios/1440.json" {
            return Ok(bytes);
        }
        let mut scenario: Value = serde_json::from_slice(&bytes).unwrap();
        scenario["opinions"] = opinions.clone();
        Ok(serde_json::to_vec(&scenario).unwrap())
    })
}

#[test]
fn scenario_opinions_resolve_country_keys_and_validate_bounds() {
    let entry = json!({"from":"cas","toward":"por","points":35});
    let state = load_with_opinions(json!([entry.clone()])).unwrap();
    let countries = &state.polity().unwrap().countries;
    let castile = countries
        .iter()
        .find(|(_, country)| country.tag == "cas")
        .unwrap()
        .0;
    let portugal = countries
        .iter()
        .find(|(_, country)| country.tag == "por")
        .unwrap()
        .0;
    assert_eq!(
        state.opinion(*castile, *portugal),
        Some(Opinion::new(35).unwrap())
    );
    assert_eq!(state.opinion(*portugal, *castile), Some(Opinion::NEUTRAL));
    let bytes = save_game(&state).unwrap();
    assert_eq!(load_game(&bytes, graph()).unwrap(), state);

    for bad in [
        json!([{"from":"cas","toward":"por","points":101}]),
        json!([{"from":"cas","toward":"missing","points":35}]),
        json!([entry.clone(), entry]),
        json!([{"from":"cas","toward":"cas","points":35}]),
        json!([{"from":"cas","toward":"por","points":0.5}]),
        json!([{"from":"cas","toward":"por","points":"35"}]),
    ] {
        assert!(load_with_opinions(bad).is_err());
    }
}

#[test]
fn pre_marriage_save_without_sex_loads_as_unknown() {
    let mut saved: Value = serde_json::from_slice(&save_game(&m3_state()).unwrap()).unwrap();
    saved["schema_version"] = json!(2);
    for character in saved["state"]["polity"]["characters"]
        .as_array_mut()
        .unwrap()
    {
        character.as_object_mut().unwrap().remove("sex");
        character.as_object_mut().unwrap().remove("spouse");
    }
    for profile in saved["state"]["polity"]["ai_profiles"]
        .as_array_mut()
        .unwrap()
    {
        profile["thresholds"]
            .as_object_mut()
            .unwrap()
            .remove("marriage_min_opinion");
    }
    let loaded = load_game(&serde_json::to_vec(&saved).unwrap(), graph()).unwrap();
    assert!(loaded
        .polity()
        .unwrap()
        .characters
        .values()
        .all(|character| character.sex == strategy_sim::Sex::Unknown));
}

#[test]
fn character_v2_requires_binary_sex_content() {
    for invalid_sex in [Value::Null, json!("unknown"), json!(1)] {
        let loaded = load_scenario_from_reader(graph(), "1440", |logical| {
            let bytes = legacy_read(logical)?;
            if logical != "characters/henry_vi.json" {
                return Ok(bytes);
            }
            let mut character: Value = serde_json::from_slice(&bytes).unwrap();
            character["sex"] = invalid_sex.clone();
            Ok(serde_json::to_vec(&character).unwrap())
        });
        assert!(loaded.is_err());
    }
}

#[test]
fn character_spouse_reference_requires_a_reciprocal_eligible_pair() {
    let loaded = load_scenario_from_reader(graph(), "1440", |logical| {
        let bytes = legacy_read(logical)?;
        if logical != "characters/henry_vi.json" {
            return Ok(bytes);
        }
        let mut character: Value = serde_json::from_slice(&bytes).unwrap();
        character["spouse"] = json!("james_ii_scotland");
        Ok(serde_json::to_vec(&character).unwrap())
    });
    assert!(loaded.is_err());
}

fn data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn graph() -> ProvinceGraph {
    parse_provinces(include_bytes!("fixtures/western_europe_v2.json")).unwrap()
}

// Keep the old graph and its ownership together for tests with fixed IDs and routes.
// Rulers, profiles and the rest of each country record come from the scenario assets.
fn legacy_read(logical: &str) -> io::Result<Vec<u8>> {
    let data = fs::read(data_root().join(logical))?;
    let mut file: Value = serde_json::from_slice(&data).unwrap();
    let legacy: Value =
        serde_json::from_str(include_str!("fixtures/western_europe_v2_ownership.json")).unwrap();
    if logical == "scenarios/1440.json" {
        file.as_object_mut().unwrap().remove("unowned_provinces");
        file["military"]["forts"] = legacy["forts"].clone();
    } else if let Some(tag) = logical
        .strip_prefix("countries/")
        .and_then(|path| path.strip_suffix(".json"))
    {
        if let Some(country) = legacy["countries"].get(tag) {
            file["capital"] = country["capital"].clone();
            file["start"]["provinces"] = country["provinces"].clone();
        }
    }
    Ok(serde_json::to_vec(&file).unwrap())
}

fn original() -> GameState {
    load_scenario_from_reader(graph(), "1440", legacy_read).unwrap()
}

fn m3_state() -> GameState {
    edited_file("scenarios/1440.json", |scenario| {
        scenario.as_object_mut().unwrap().remove("military");
    })
    .unwrap()
}

fn edited_file(path: &str, edit: impl FnOnce(&mut Value)) -> Result<GameState, ScenarioLoadError> {
    let mut value: Value = serde_json::from_slice(&legacy_read(path).unwrap()).unwrap();
    edit(&mut value);
    let replacement = serde_json::to_vec(&value).unwrap();
    load_scenario_from_reader(graph(), "1440", |logical| {
        if logical == path {
            Ok(replacement.clone())
        } else {
            legacy_read(logical)
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

    let bytes = String::from_utf8(legacy_read("scenarios/1440.json").unwrap()).unwrap();
    let duplicate = bytes.replace("\"eng\":1", "\"eng\":1,\"eng\":2");
    assert_ne!(duplicate, bytes);
    assert_invalid(
        load_scenario_from_reader(graph(), "1440", |path| -> io::Result<Vec<u8>> {
            if path == "scenarios/1440.json" {
                Ok(duplicate.as_bytes().to_vec())
            } else {
                legacy_read(path)
            }
        }),
        "duplicate object key",
    );
}

#[test]
fn save_v2_round_trips_country_economy_characters_profiles_and_rules() {
    let mut state = m3_state();
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
    let mut save: Value = serde_json::from_slice(&save_game(&m3_state()).unwrap()).unwrap();
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

#[test]
fn m4_scenario_validates_fort_references_and_rules() {
    let state = original();
    let war = state.war_state().unwrap();
    assert_eq!(war.forts.len(), 7);
    let london = state.province_graph().by_key("london").unwrap().id;
    assert_eq!(war.forts[&london], 2);
    assert_eq!(war.rules.raise_cost_per_soldier.raw(), 8);
    assert_eq!(war.rules.siege_days_per_level, 15);
    for province in ["unknown_province", "north_sea"] {
        assert_invalid(
            edited_file("scenarios/1440.json", |scenario| {
                scenario["military"]["forts"][province] = json!(1);
            }),
            "existing land province",
        );
    }
    assert_invalid(
        edited_file("scenarios/1440.json", |scenario| {
            scenario["military"]["forts"]["london"] = json!(0);
        }),
        "fort level must be positive",
    );
    assert_invalid(
        edited_file("scenarios/1440.json", |scenario| {
            scenario["military"]["rules"]["max_army_size"] = json!(0);
        }),
        "invalid military rules",
    );
}

#[test]
fn save_v3_round_trips_military_rules_and_forts() {
    let state = original();
    let bytes = save_game(&state).unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["schema_version"], 3);
    assert_eq!(
        json["state"]["military"]["forts"].as_array().unwrap().len(),
        7
    );
    let restored = load_game(&bytes, graph()).unwrap();
    assert_eq!(restored, state);
    assert_eq!(state_hash(&restored).unwrap(), state_hash(&state).unwrap());
    assert_eq!(save_game(&restored).unwrap(), bytes);
}

#[test]
fn save_v3_rejects_bad_versions_and_military_records() {
    let bytes = save_game(&original()).unwrap();
    let mut save: Value = serde_json::from_slice(&bytes).unwrap();
    save["schema_version"] = json!(2);
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::VersionMilitaryMismatch(2))
    ));

    save["schema_version"] = json!(3);
    let forts = save["state"]["military"]["forts"].as_array_mut().unwrap();
    forts.insert(0, forts[0].clone());
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::UnorderedRecords("forts"))
    ));

    save = serde_json::from_slice(&bytes).unwrap();
    save["state"]["military"]["forts"][0]["province"] = json!(0);
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::InvalidMilitary(_))
    ));

    save["state"].as_object_mut().unwrap().remove("military");
    assert!(matches!(
        load_game(&serde_json::to_vec(&save).unwrap(), graph()),
        Err(SaveError::VersionMilitaryMismatch(3))
    ));
}

#[test]
fn save_v3_preserves_active_armies_movement_sieges_and_treaties() {
    use std::collections::BTreeMap;

    use strategy_sim::{
        Army, ArmyId, ArmyMovement, Fixed, MovementLeg, PeaceOffer, PeaceTreaty, Siege, Tick, War,
        WarId,
    };

    let mut military = original().war_state().unwrap().clone();
    let state = m3_state();
    let polity = state.polity().unwrap();
    let country = |tag| {
        polity
            .countries
            .iter()
            .find(|(_, country)| country.tag == tag)
            .unwrap()
            .0
    };
    let province = |key| state.province_graph().by_key(key).unwrap().id;
    let attacker = *country("eng");
    let defender = *country("sco");
    let castile = *country("cas");
    let portugal = *country("por");
    let from = province("northumbria");
    let first_leg = province("lowlands");
    let goal = province("highlands");
    let siege_province = goal;
    let edge = state
        .province_graph()
        .neighbors(from)
        .unwrap()
        .iter()
        .find(|edge| edge.to == first_leg)
        .unwrap();
    military.armies = BTreeMap::from([
        (
            ArmyId::new(1),
            Army {
                country: attacker,
                province: from,
                soldiers: 400,
                morale: Fixed::ONE,
                movement: Some(ArmyMovement {
                    route: vec![MovementLeg {
                        to: first_leg,
                        kind: edge.kind,
                        cost: edge.cost,
                    }],
                    elapsed_ticks: 1,
                }),
            },
        ),
        (
            ArmyId::new(2),
            Army {
                country: attacker,
                province: siege_province,
                soldiers: 250,
                morale: Fixed::from_raw(600),
                movement: None,
            },
        ),
    ]);
    let active = WarId::new(2);
    military.wars.insert(
        active,
        War {
            attacker,
            defender,
            goal,
            started_at: Tick::ZERO,
            score: -15,
            occupations: BTreeMap::from([(province("yorkshire"), defender)]),
            offer: Some(PeaceOffer {
                offered_by: defender,
                cede_goal: false,
            }),
        },
    );
    military.sieges.insert(
        siege_province,
        Siege {
            war: active,
            besieger: attacker,
            progress_days: 1,
        },
    );
    military.treaties.insert(
        WarId::new(1),
        PeaceTreaty {
            attacker: *country("cas"),
            defender: *country("por"),
            ceded: None,
            concluded_at: Tick::ZERO,
        },
    );
    military.next_army_id = 3;
    military.next_war_id = 3;

    let state = state.with_war_state(military).unwrap();
    let bytes = save_game(&state).unwrap();
    let restored = load_game(&bytes, graph()).unwrap();
    assert_eq!(restored, state);
    assert_eq!(save_game(&restored).unwrap(), bytes);
    assert_eq!(state_hash(&restored).unwrap(), state_hash(&state).unwrap());

    let mut invalid: Value = serde_json::from_slice(&bytes).unwrap();
    invalid["state"]["military"]["armies"][0]["movement"]["route"][0]["cost"] = json!(999);
    assert!(matches!(
        load_game(&serde_json::to_vec(&invalid).unwrap(), graph()),
        Err(SaveError::InvalidMilitary(_))
    ));

    let mut invalid: Value = serde_json::from_slice(&bytes).unwrap();
    invalid["state"]["military"]["wars"][0]["started_at"] = json!(1);
    assert!(matches!(
        load_game(&serde_json::to_vec(&invalid).unwrap(), graph()),
        Err(SaveError::InvalidMilitary(_))
    ));

    let mut diplomacy = DiplomacyState::default();
    diplomacy
        .alliances
        .insert((castile.min(portugal), castile.max(portugal)));
    let valid_v5 = state.clone().with_diplomacy(diplomacy).unwrap();
    let mut forged: Value = serde_json::from_slice(&save_game(&valid_v5).unwrap()).unwrap();
    forged["state"]["diplomacy"]["alliances"][0]["a"] = json!(attacker.min(defender).get());
    forged["state"]["diplomacy"]["alliances"][0]["b"] = json!(attacker.max(defender).get());
    assert!(matches!(
        load_game(&serde_json::to_vec(&forged).unwrap(), graph()),
        Err(SaveError::InvalidDiplomacy(DiplomacyError::DirectWar))
    ));
}

#[test]
fn explicit_unowned_land_must_partition_the_map() {
    let state = original();
    let unowned: Vec<_> = state
        .province_graph()
        .iter()
        .filter(|province| province.kind == ProvinceKind::Land)
        .filter(|province| state.owner_of_province(province.id).is_none())
        .map(|province| province.key.clone())
        .collect();
    assert_eq!(unowned.len(), 18);
    let with_unowned = |provinces: Value| {
        edited_file("scenarios/1440.json", |scenario| {
            scenario["unowned_provinces"] = provinces;
        })
    };
    assert_eq!(with_unowned(json!(unowned)).unwrap(), original());
    assert_invalid(
        with_unowned(json!([])),
        "neither owned nor declared unowned",
    );

    for (key, expected) in [
        ("london", "both owned and declared unowned"),
        ("north_sea", "is not land"),
        ("nonexistent", "unknown province"),
        (unowned[0].as_str(), "duplicate unowned province"),
    ] {
        let mut declared = unowned.clone();
        declared.push(key.into());
        assert_invalid(with_unowned(json!(declared)), expected);
    }
    assert_invalid(with_unowned(Value::Null), "invalid type: null");
}

#[test]
fn city_map_scenario_loads_with_explicit_land_partition_and_round_trips() {
    let data = data_root();
    let map = load_provinces(data.join("map/western_europe/provinces.json")).unwrap();
    let scenario_path = data.join("scenarios/1440.json");
    let state = load_scenario(map.clone(), &scenario_path).unwrap();
    let scenario: Value = serde_json::from_slice(&fs::read(scenario_path).unwrap()).unwrap();
    let declared: BTreeSet<_> = scenario["unowned_provinces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry.as_str().unwrap())
        .collect();
    let neutral: BTreeSet<_> = map
        .iter()
        .filter(|province| province.kind == ProvinceKind::Land)
        .filter(|province| state.owner_of_province(province.id).is_none())
        .map(|province| province.key.as_str())
        .collect();
    assert_eq!(declared, neutral);
    assert_eq!(map.identity().version.get(), 3);
    assert_eq!(state.polity().unwrap().countries.len(), 6);
    assert_eq!(state.polity().unwrap().characters.len(), 13);
    let bytes = save_game(&state).unwrap();
    assert_eq!(load_game(&bytes, map).unwrap(), state);
    assert!(matches!(
        load_game(&bytes, graph()),
        Err(SaveError::MapMismatch(_))
    ));
}
