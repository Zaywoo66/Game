use std::{num::NonZeroU32, path::Path, process::Command};

use serde_json::{json, Value};
use strategy_data::{parse_provinces, ProvinceLoadError};
use strategy_sim::{EdgeKind, MapIdentity, ProvinceId, ProvinceKind};

const FIXTURE: &[u8] = include_bytes!("fixtures/provinces.json");
const GOLDEN_HASH: &str = "ef610c0ac68bf9e1c0c84fb75f5a03a69dc761f1efad9790e41aa7e5414f347a";

fn fixture() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}

fn parse(value: &Value) -> Result<strategy_sim::ProvinceGraph, ProvinceLoadError> {
    parse_provinces(&serde_json::to_vec(value).unwrap())
}

fn province(map: &mut Value, id: u32) -> &mut Value {
    map["provinces"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["id"] == id)
        .unwrap()
}

fn assert_graph_error(map: &Value, expected: &str) {
    let error = parse(map).unwrap_err();
    assert!(matches!(error, ProvinceLoadError::Graph(_)), "{error}");
    assert!(error.to_string().contains(expected), "{error}");
}

#[test]
fn loads_sparse_ids_and_preserves_distinct_edge_types() {
    let graph = parse_provinces(FIXTURE).unwrap();
    let ids: Vec<_> = graph.iter().map(|entry| entry.id.get()).collect();
    assert_eq!(ids, [1, 2, 7, 9, 11]);
    assert_eq!(graph.by_key("test_lake").unwrap().kind, ProvinceKind::Lake);
    assert_eq!(graph.by_key("test_sea").unwrap().kind, ProvinceKind::Sea);
    let land = graph.by_key("test_land").unwrap();
    assert_eq!(land.kind, ProvinceKind::Land);
    assert_eq!(land.centroid, [12, 34]);
    assert_eq!(land.development, 3);
    assert_eq!(land.ports, [ProvinceId::new(7), ProvinceId::new(9)]);
    let edges: Vec<_> = graph
        .neighbors(land.id)
        .unwrap()
        .iter()
        .map(|edge| (edge.to.get(), edge.kind))
        .collect();
    assert_eq!(
        edges,
        [
            (2, EdgeKind::Normal),
            (2, EdgeKind::River),
            (7, EdgeKind::Strait),
            (9, EdgeKind::Port),
        ]
    );
    assert_eq!(graph.neighbors(ProvinceId::new(99)), None);
    assert!(graph
        .iter()
        .flat_map(|province| &province.neighbors)
        .all(|edge| edge.cost == 1));
}

#[test]
fn reordered_and_reformatted_json_has_the_same_golden_hash() {
    let graph = parse_provinces(FIXTURE).unwrap();
    assert_eq!(
        graph.identity().to_string(),
        format!("map version 4, SHA-256 {GOLDEN_HASH}")
    );
    let mut map = fixture();
    let entries = map["provinces"].as_array_mut().unwrap();
    entries.reverse();
    for entry in entries {
        entry["neighbors"].as_array_mut().unwrap().reverse();
        entry["ports"].as_array_mut().unwrap().reverse();
    }
    assert_eq!(parse(&map).unwrap(), graph);
}

#[test]
fn rejects_invalid_schema_fields_and_versions() {
    let mut map = fixture();
    map["schema_version"] = json!(3);
    assert!(matches!(
        parse(&map),
        Err(ProvinceLoadError::UnsupportedSchemaVersion(3))
    ));
    map = fixture();
    map.as_object_mut().unwrap().remove("schema_version");
    assert!(matches!(parse(&map), Err(ProvinceLoadError::Json(_))));

    for (field, invalid) in [
        ("kind", json!("ocean")),
        ("centroid", json!([1, 2, 3])),
        ("development", json!(-1)),
        ("neighbors", json!([{ "id": 2, "edge": "bridge" }])),
        ("cost", json!(2)),
    ] {
        map = fixture();
        province(&mut map, 1)[field] = invalid;
        assert!(matches!(parse(&map), Err(ProvinceLoadError::Json(_))));
    }

    map = fixture();
    map["map_version"] = json!(0);
    assert!(matches!(parse(&map), Err(ProvinceLoadError::Json(_))));
}

#[test]
fn rejects_reserved_ids_duplicate_ids_and_invalid_or_duplicate_keys() {
    let mut map = fixture();
    province(&mut map, 2)["id"] = json!(0);
    assert_graph_error(&map, "ID 0 is reserved");
    map = fixture();
    province(&mut map, 2)["id"] = json!(1);
    assert_graph_error(&map, "duplicate province ID 1");
    map = fixture();
    province(&mut map, 2)["key"] = json!("test_land");
    assert_graph_error(&map, "duplicate province key");
    map = fixture();
    province(&mut map, 2)["key"] = json!("Invalid key");
    assert_graph_error(&map, "invalid key");
}

#[test]
fn rejects_dangling_unpaired_and_duplicate_edges() {
    let mut map = fixture();
    province(&mut map, 1)["neighbors"][0]["id"] = json!(99);
    assert_graph_error(&map, "edge to unknown province 99");

    map = fixture();
    province(&mut map, 2)["neighbors"][1]["edge"] = json!("strait");
    assert_graph_error(&map, "reverse River edge");

    map = fixture();
    let edges = province(&mut map, 1)["neighbors"].as_array_mut().unwrap();
    edges.push(edges[0].clone());
    assert_graph_error(&map, "duplicate typed edge");
}

#[test]
fn rejects_unknown_non_sea_and_duplicate_ports() {
    for (ports, expected) in [
        (json!([99]), "port to unknown province 99"),
        (json!([11]), "port to non-sea province 11"),
        (json!([7, 7]), "duplicate port"),
    ] {
        let mut map = fixture();
        province(&mut map, 1)["ports"] = ports;
        assert_graph_error(&map, expected);
    }
}

#[test]
fn validates_declared_hash_and_checks_both_parts_of_save_identity() {
    let mut map = fixture();
    map["map_hash"] = json!(GOLDEN_HASH.to_uppercase());
    assert!(matches!(
        parse(&map),
        Err(ProvinceLoadError::InvalidMapHash)
    ));

    map = fixture();
    province(&mut map, 1)["development"] = json!(100);
    assert!(matches!(
        parse(&map),
        Err(ProvinceLoadError::MapHashMismatch { .. })
    ));

    let identity = parse_provinces(FIXTURE).unwrap().identity();
    assert!(identity.verify_save(identity).is_ok());
    map = fixture();
    map["map_version"] = json!(5);
    let newer = parse(&map).unwrap().identity();
    assert_eq!(newer.sha256, identity.sha256);
    assert_eq!(newer.version, NonZeroU32::new(5).unwrap());
    assert!(identity.verify_save(newer).is_err());

    let other_hash = MapIdentity {
        sha256: [0; 32],
        ..identity
    };
    let error = identity.verify_save(other_hash).unwrap_err();
    assert_eq!(error.loaded, identity);
    assert_eq!(error.saved, other_hash);
}

#[test]
fn lint_cli_uses_the_same_graph_validator() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let run = |filename| {
        Command::new(env!("CARGO_BIN_EXE_lint-provinces"))
            .arg(fixtures.join(filename))
            .output()
            .unwrap()
    };
    let valid = run("provinces.json");
    assert!(valid.status.success());
    let invalid = run("dangling-province.json");
    assert_eq!(invalid.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("edge to unknown province"));
}

const V2_FIXTURE: &[u8] = include_bytes!("fixtures/provinces_v2.json");
const V2_GOLDEN_HASH: &str = "c163abb511bb665266aaa0aa619f4a1203afa0f4d7fcf8ce7212a5b80e57b4dc";

#[test]
fn weighted_map_preserves_costs_and_the_canonical_v2_hash() {
    let graph = parse_provinces(V2_FIXTURE).unwrap();
    assert_eq!(
        graph.identity().to_string(),
        format!("map version 5, SHA-256 {V2_GOLDEN_HASH}")
    );
    let edges: Vec<_> = graph
        .neighbors(ProvinceId::new(1))
        .unwrap()
        .iter()
        .map(|edge| (edge.to.get(), edge.kind, edge.cost))
        .collect();
    assert_eq!(
        edges,
        [
            (2, EdgeKind::Normal, 2),
            (2, EdgeKind::River, 3),
            (7, EdgeKind::Strait, 5),
            (9, EdgeKind::Port, 2),
        ]
    );
    let mut map: Value = serde_json::from_slice(V2_FIXTURE).unwrap();
    let entries = map["provinces"].as_array_mut().unwrap();
    entries.reverse();
    for entry in entries {
        entry["neighbors"].as_array_mut().unwrap().reverse();
    }
    assert_eq!(parse(&map).unwrap(), graph);
}

#[test]
fn rejects_missing_or_nonpositive_costs_and_costs_on_legacy_maps() {
    let mut map: Value = serde_json::from_slice(V2_FIXTURE).unwrap();
    province(&mut map, 1)["neighbors"][0]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    assert!(matches!(
        parse(&map),
        Err(ProvinceLoadError::EdgeCostVersionMismatch { version: 2, .. })
    ));
    for bad in [json!(0), json!(-1), json!(1.5), json!(null)] {
        province(&mut map, 1)["neighbors"][0]["cost"] = bad;
        assert!(matches!(parse(&map), Err(ProvinceLoadError::Json(_))));
    }
    let mut legacy = fixture();
    province(&mut legacy, 1)["neighbors"][0]["cost"] = json!(1);
    assert!(matches!(
        parse(&legacy),
        Err(ProvinceLoadError::EdgeCostVersionMismatch { version: 1, .. })
    ));
}

#[test]
fn reverse_costs_match_and_cost_changes_are_hashed() {
    let mut map: Value = serde_json::from_slice(V2_FIXTURE).unwrap();
    let change = |record: &mut Value, target: u32| {
        let edge = record["neighbors"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|edge| edge["id"] == target && edge["edge"] == "normal")
            .unwrap();
        edge["cost"] = json!(9);
    };
    change(province(&mut map, 1), 2);
    assert!(matches!(parse(&map), Err(ProvinceLoadError::Graph(_))));
    change(province(&mut map, 2), 1);
    assert!(matches!(
        parse(&map),
        Err(ProvinceLoadError::MapHashMismatch { .. })
    ));
}
