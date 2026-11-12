use std::num::NonZeroU32;

use strategy_sim::{
    ArmyId, CountryId, EdgeKind, Fleet, FleetId, MapIdentity, NavyError, NavyState, Province,
    ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind, SHIP_CAPACITY,
};

fn sea(id: u32) -> Province {
    Province {
        id: ProvinceId::new(id),
        key: format!("sea_{id}"),
        kind: ProvinceKind::Sea,
        terrain: "sea".into(),
        centroid: [id as i32, 0],
        development: 0,
        neighbors: Vec::new(),
        ports: Vec::new(),
    }
}

fn graph() -> ProvinceGraph {
    let mut a = sea(1);
    let mut b = sea(2);
    a.neighbors.push(ProvinceEdge {
        to: b.id,
        kind: EdgeKind::Normal,
        cost: 2,
    });
    b.neighbors.push(ProvinceEdge {
        to: a.id,
        kind: EdgeKind::Normal,
        cost: 2,
    });
    ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [0; 32],
        },
        [a, b],
    )
    .unwrap()
}

fn fleet() -> (FleetId, NavyState) {
    let id = FleetId::new(1);
    let mut navy = NavyState::new();
    navy.fleets.insert(
        id,
        Fleet {
            country: CountryId::new(1),
            sea: ProvinceId::new(1),
            ships: 4,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 2;
    (id, navy)
}

#[test]
fn sea_movement_uses_whole_daily_cost_and_validates_ownership() {
    let graph = graph();
    let (id, mut navy) = fleet();
    navy.validate(&graph, [CountryId::new(1)], []).unwrap();
    assert_eq!(
        navy.fleet_route(&graph, id, CountryId::new(2), ProvinceId::new(2)),
        Err(NavyError::NotFleetOwner(id))
    );
    let route = navy
        .fleet_route(&graph, id, CountryId::new(1), ProvinceId::new(2))
        .unwrap();
    navy.fleets.get_mut(&id).unwrap().movement = Some(strategy_sim::FleetMovement {
        route,
        elapsed_ticks: 0,
    });
    assert_eq!(navy.advance(|_, _| false), [(id, None)]);
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(1));
    assert_eq!(navy.advance(|_, _| false), [(id, None)]);
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(2));
    assert!(navy.fleets[&id].movement.is_none());
    assert_eq!(navy.advance(|_, _| false), []);
}

#[test]
fn cargo_validation_rejects_over_capacity_and_duplicates() {
    assert_eq!(SHIP_CAPACITY, 1000);
    let graph = graph();
    let (id, mut navy) = fleet();
    navy.fleets.get_mut(&id).unwrap().cargo = Some(ArmyId::new(8));
    let armies = [(ArmyId::new(8), CountryId::new(1), 3500, false)];
    navy.validate(&graph, [CountryId::new(1)], armies).unwrap();
    navy.fleets.get_mut(&id).unwrap().ships = 3;
    assert_eq!(
        navy.validate(&graph, [CountryId::new(1)], armies),
        Err(NavyError::InsufficientCapacity(id))
    );
    navy.fleets.get_mut(&id).unwrap().ships = 4;
    let mut duplicate = navy.fleets[&id].clone();
    duplicate.sea = ProvinceId::new(2);
    navy.fleets.insert(FleetId::new(2), duplicate);
    navy.next_fleet_id = 3;
    assert_eq!(
        navy.validate(&graph, [CountryId::new(1)], armies),
        Err(NavyError::CargoAlreadyAboard(ArmyId::new(8)))
    );
}

#[test]
fn enemy_in_next_sea_zone_blocks_entry_with_explicit_fleet_id() {
    let graph = graph();
    let (id, mut navy) = fleet();
    let enemy = FleetId::new(2);
    navy.fleets.insert(
        enemy,
        Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(2),
            ships: 1,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 3;
    let route = navy
        .fleet_route(&graph, id, CountryId::new(1), ProvinceId::new(2))
        .unwrap();
    navy.fleets.get_mut(&id).unwrap().movement = Some(strategy_sim::FleetMovement {
        route,
        elapsed_ticks: 1,
    });
    assert_eq!(navy.advance(|a, b| a != b), [(id, Some(enemy))]);
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(1));
    assert!(navy.fleets[&id].movement.is_none());
}
