use std::num::NonZeroU32;

use strategy_sim::{
    EdgeKind, MapIdentity, Province, ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind,
};

fn province(id: u32, kind: ProvinceKind) -> Province {
    Province {
        id: ProvinceId::new(id),
        key: format!("province_{id}"),
        kind,
        terrain: "plains".into(),
        centroid: [id as i32, 0],
        development: 0,
        neighbors: Vec::new(),
        ports: Vec::new(),
    }
}

fn connect(provinces: &mut [Province], a: u32, b: u32, kind: EdgeKind, cost: u32) {
    for (from, to) in [(a, b), (b, a)] {
        provinces
            .iter_mut()
            .find(|province| province.id.get() == from)
            .unwrap()
            .neighbors
            .push(ProvinceEdge {
                to: ProvinceId::new(to),
                kind,
                cost,
            });
    }
}

fn graph() -> ProvinceGraph {
    let mut provinces = vec![
        province(8, ProvinceKind::Sea),
        province(7, ProvinceKind::Lake),
        province(6, ProvinceKind::Sea),
        province(5, ProvinceKind::Sea),
        province(4, ProvinceKind::Sea),
        province(3, ProvinceKind::Sea),
        province(2, ProvinceKind::Sea),
        province(1, ProvinceKind::Land),
    ];
    connect(&mut provinces, 1, 2, EdgeKind::Port, 1);
    connect(&mut provinces, 1, 5, EdgeKind::Normal, 1);
    connect(&mut provinces, 2, 3, EdgeKind::Normal, 2);
    connect(&mut provinces, 3, 5, EdgeKind::Strait, 3);
    connect(&mut provinces, 2, 4, EdgeKind::Normal, 2);
    connect(&mut provinces, 4, 5, EdgeKind::Normal, 3);
    connect(&mut provinces, 2, 6, EdgeKind::Normal, 20);
    connect(&mut provinces, 6, 5, EdgeKind::Normal, 1);
    connect(&mut provinces, 2, 7, EdgeKind::River, 1);
    connect(&mut provinces, 2, 8, EdgeKind::Port, 1);
    connect(&mut provinces, 8, 5, EdgeKind::Normal, 1);
    ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [0; 32],
        },
        provinces,
    )
    .unwrap()
}

#[test]
fn weighted_route_uses_only_sea_edges_and_stable_id_tie_break() {
    let graph = graph();
    let route = graph
        .sea_route(ProvinceId::new(2), ProvinceId::new(5))
        .unwrap();
    assert_eq!(
        route.iter().map(|edge| edge.to.get()).collect::<Vec<_>>(),
        [3, 5]
    );
    assert_eq!(route[0].kind, EdgeKind::Normal);
    assert_eq!(route[1].kind, EdgeKind::Strait);
    assert_eq!(route.iter().map(|edge| edge.cost).sum::<u32>(), 5);
}

#[test]
fn coast_lake_and_unknown_endpoints_do_not_become_sea_routes() {
    let graph = graph();
    let sea = ProvinceId::new(2);
    assert_eq!(graph.sea_route(sea, sea), Some(Vec::new()));
    for invalid in [1, 7, 99] {
        assert_eq!(graph.sea_route(ProvinceId::new(invalid), sea), None);
        assert_eq!(graph.sea_route(sea, ProvinceId::new(invalid)), None);
    }
}

#[test]
fn ported_island_needs_future_embark_command_for_land_access() {
    let mut provinces = vec![
        province(10, ProvinceKind::Land),
        province(11, ProvinceKind::Sea),
        province(12, ProvinceKind::Sea),
    ];
    provinces[0].ports.push(ProvinceId::new(11));
    connect(&mut provinces, 10, 11, EdgeKind::Port, 1);
    connect(&mut provinces, 11, 12, EdgeKind::Normal, 4);
    let graph = ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [0; 32],
        },
        provinces,
    )
    .unwrap();
    assert_eq!(
        graph
            .sea_route(ProvinceId::new(11), ProvinceId::new(12))
            .unwrap()[0]
            .cost,
        4
    );
    assert_eq!(
        graph.sea_route(ProvinceId::new(10), ProvinceId::new(12)),
        None
    );
}

#[test]
fn sea_to_sea_edges_require_a_matching_reverse_edge() {
    let mut provinces = vec![
        province(2, ProvinceKind::Sea),
        province(3, ProvinceKind::Sea),
    ];
    connect(&mut provinces, 2, 3, EdgeKind::Strait, 4);
    provinces[1].neighbors.clear();
    assert!(ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [0; 32],
        },
        provinces,
    )
    .is_err());
}
