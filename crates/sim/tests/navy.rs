use std::num::NonZeroU32;

use std::collections::{BTreeMap, BTreeSet};
use strategy_sim::{
    ArmyId, CountryId, EdgeKind, Fleet, FleetId, MapIdentity, NavyError, NavyState, Province,
    ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind, Tick, War, WarId, WarSide,
    SHIP_CAPACITY,
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

fn war() -> War {
    War {
        attacker: CountryId::new(1),
        defender: CountryId::new(2),
        attacker_supporters: BTreeSet::new(),
        defender_supporters: BTreeSet::new(),
        goal: ProvinceId::new(1),
        started_at: Tick::ZERO,
        score: 0,
        occupations: BTreeMap::new(),
        offer: None,
    }
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
    assert_eq!(navy.advance(), [(id, ProvinceId::new(1))].into());
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(1));
    assert_eq!(navy.advance(), [(id, ProvinceId::new(1))].into());
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(2));
    assert!(navy.fleets[&id].movement.is_none());
    assert!(navy.advance().is_empty());
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
fn enemy_in_next_sea_zone_is_reached_for_battle() {
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
    assert_eq!(navy.advance(), [(id, ProvinceId::new(1))].into());
    assert_eq!(navy.fleets[&id].sea, ProvinceId::new(2));
    assert!(navy.fleets[&id].movement.is_none());
}

#[test]
fn stronger_side_wins_with_ceil_losses_and_loser_retreats() {
    let graph = graph();
    let (_, mut navy) = fleet();
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().ships = 20;
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().sea = ProvinceId::new(2);
    navy.fleets.insert(
        FleetId::new(2),
        Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(2),
            ships: 10,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 3;
    let war = war();
    let previous = BTreeMap::from([(FleetId::new(2), ProvinceId::new(1))]);
    let battles = navy.resolve_battles(&graph, [(WarId::new(1), &war)], &previous, |_| None);
    assert_eq!(battles.len(), 1);
    assert_eq!(battles[0].winner, Some(WarSide::Attacker));
    assert_eq!(
        (battles[0].attacker_losses, battles[0].defender_losses),
        (2, 3)
    );
    assert_eq!(navy.fleets[&FleetId::new(1)].ships, 18);
    assert_eq!(navy.fleets[&FleetId::new(2)].ships, 7);
    assert_eq!(navy.fleets[&FleetId::new(2)].sea, ProvinceId::new(1));
}

#[test]
fn sunk_transport_loses_its_full_army() {
    let graph = graph();
    let (_, mut navy) = fleet();
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().ships = 1;
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().cargo = Some(ArmyId::new(1));
    navy.fleets.insert(
        FleetId::new(2),
        Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(1),
            ships: 2,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 3;
    let war = war();
    let battles = navy.resolve_battles(&graph, [(WarId::new(1), &war)], &BTreeMap::new(), |_| {
        Some(500)
    });
    assert_eq!(battles[0].lost_armies, [ArmyId::new(1)]);
    assert_eq!(battles[0].attacker_losses, 1);
    assert!(!navy.fleets.contains_key(&FleetId::new(1)));
}

#[test]
fn tied_fleets_both_lose_ten_percent_without_randomness() {
    let graph = graph();
    let (_, mut navy) = fleet();
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().ships = 10;
    navy.fleets.insert(
        FleetId::new(2),
        Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(1),
            ships: 10,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 3;
    let war = war();
    let battles = navy.resolve_battles(&graph, [(WarId::new(1), &war)], &BTreeMap::new(), |_| None);
    assert_eq!(battles[0].winner, None);
    assert_eq!(
        (battles[0].attacker_losses, battles[0].defender_losses),
        (1, 1)
    );
    assert_eq!(navy.fleets[&FleetId::new(1)].ships, 9);
    assert_eq!(navy.fleets[&FleetId::new(2)].ships, 9);
}

#[test]
fn ship_losses_follow_fleet_id_order_within_each_side() {
    let graph = graph();
    let (_, mut navy) = fleet();
    navy.fleets.get_mut(&FleetId::new(1)).unwrap().ships = 1;
    navy.fleets.insert(
        FleetId::new(2),
        Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(1),
            ships: 10,
            movement: None,
            cargo: None,
        },
    );
    navy.fleets.insert(
        FleetId::new(3),
        Fleet {
            country: CountryId::new(1),
            sea: ProvinceId::new(1),
            ships: 19,
            movement: None,
            cargo: None,
        },
    );
    navy.next_fleet_id = 4;
    let war = war();
    let battle = navy.resolve_battles(&graph, [(WarId::new(1), &war)], &BTreeMap::new(), |_| None);
    assert_eq!(battle[0].attacker_losses, 2);
    assert!(!navy.fleets.contains_key(&FleetId::new(1)));
    assert_eq!(navy.fleets[&FleetId::new(3)].ships, 18);
}
