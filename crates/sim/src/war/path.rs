use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

use crate::{CountryId, EdgeKind, PolityState, ProvinceGraph, ProvinceId, ProvinceKind};

use super::{owner, MovementLeg, WarState};

fn distance(a: [i32; 2], b: [i32; 2]) -> u64 {
    i64::from(a[0]).abs_diff(i64::from(b[0])) + i64::from(a[1]).abs_diff(i64::from(b[1]))
}

impl WarState {
    pub(crate) fn find_path(
        &self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        country: CountryId,
        from: ProvinceId,
        to: ProvinceId,
    ) -> Option<Vec<MovementLeg>> {
        find_path(graph, from, to, |province| {
            self.can_enter(polity, country, province)
        })
    }

    pub(super) fn can_reach_goal(
        &self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        attacker: CountryId,
        defender: CountryId,
        goal: ProvinceId,
    ) -> bool {
        goal != polity.countries[&defender].capital
            && owner(polity, goal) == Some(defender)
            && polity.countries[&attacker].provinces.iter().any(|&from| {
                let frontier = graph.neighbors(from).is_some_and(|edges| {
                    edges.iter().any(|edge| {
                        edge.kind != EdgeKind::Port && owner(polity, edge.to) == Some(defender)
                    })
                });
                frontier
                    && find_path(graph, from, goal, |province| {
                        owner(polity, province).is_some_and(|id| id == attacker || id == defender)
                    })
                    .is_some()
            })
    }
}

fn find_path(
    graph: &ProvinceGraph,
    from: ProvinceId,
    to: ProvinceId,
    can_enter: impl Fn(ProvinceId) -> bool,
) -> Option<Vec<MovementLeg>> {
    if !can_enter(from) || !can_enter(to) {
        return None;
    }
    if from == to {
        return Some(Vec::new());
    }

    let target = graph.get(to)?.centroid;
    // No one-day edge can cover more geometric distance than this.
    // The resulting lower bound is admissible for positive tick costs.
    let max_span = graph
        .iter()
        .flat_map(|province| {
            province.neighbors.iter().filter_map(|edge| {
                graph
                    .get(edge.to)
                    .map(|other| distance(province.centroid, other.centroid))
            })
        })
        .max()
        .unwrap_or(0)
        .max(1);
    let heuristic = |province| {
        graph
            .get(province)
            .map_or(0, |point| distance(point.centroid, target) / max_span)
    };

    let mut best = BTreeMap::from([(from, 0_u64)]);
    let mut previous = BTreeMap::new();
    let mut queue = BinaryHeap::from([Reverse((heuristic(from), 0_u64, from))]);
    while let Some(Reverse((_, cost, province))) = queue.pop() {
        if best.get(&province) != Some(&cost) {
            continue;
        }
        if province == to {
            let mut route = Vec::new();
            let mut current = to;
            while current != from {
                let &(before, leg) = previous.get(&current)?;
                route.push(leg);
                current = before;
            }
            route.reverse();
            return Some(route);
        }
        for edge in graph.neighbors(province)? {
            if edge.kind == EdgeKind::Port
                || !graph
                    .get(edge.to)
                    .is_some_and(|province| province.kind == ProvinceKind::Land)
                || !can_enter(edge.to)
            {
                continue;
            }
            let next = cost.saturating_add(u64::from(edge.cost));
            if best.get(&edge.to).is_none_or(|&known| next < known) {
                best.insert(edge.to, next);
                previous.insert(
                    edge.to,
                    (
                        province,
                        MovementLeg {
                            to: edge.to,
                            kind: edge.kind,
                            cost: edge.cost,
                        },
                    ),
                );
                queue.push(Reverse((
                    next.saturating_add(heuristic(edge.to)),
                    next,
                    edge.to,
                )));
            }
        }
    }
    None
}
