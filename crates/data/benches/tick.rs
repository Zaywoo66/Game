use std::{collections::BTreeMap, hint::black_box, num::NonZeroU32};

use criterion::{criterion_group, criterion_main, Criterion};
use strategy_sim::{
    AiProfileKey, Controller, CountryId, CountryState, Date, EdgeKind, Fixed, GameState,
    MapIdentity, ModKey, Modifier, ModifierKind, ModifierSourceId, ModifierStore, Province,
    ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind, SimTime, Speed,
};

const PROVINCES: u32 = 400;
const COUNTRIES: u32 = 8;

fn synthetic_state() -> GameState {
    let graph = ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::MIN,
            sha256: [0; 32],
        },
        (1..=PROVINCES).map(|number| {
            let previous = if number == 1 { PROVINCES } else { number - 1 };
            let next = if number == PROVINCES { 1 } else { number + 1 };
            Province {
                id: ProvinceId::new(number),
                key: format!("province_{number}"),
                kind: ProvinceKind::Land,
                terrain: "grassland".into(),
                centroid: [number as i32, 0],
                development: 3,
                neighbors: [previous, next]
                    .into_iter()
                    .map(|neighbor| ProvinceEdge {
                        to: ProvinceId::new(neighbor),
                        kind: EdgeKind::Normal,
                        cost: 1,
                    })
                    .collect(),
                ports: Vec::new(),
            }
        }),
    )
    .unwrap();
    let countries = (1..=COUNTRIES)
        .map(|number| {
            let mut modifiers = ModifierStore::default();
            let source = ModifierSourceId::new(1);
            modifiers
                .insert_source(
                    source,
                    None,
                    vec![Modifier {
                        source,
                        key: ModKey::new("bench_stat"),
                        value: Fixed::ONE,
                        kind: ModifierKind::Additive,
                    }],
                )
                .unwrap();
            (
                CountryId::new(number),
                CountryState::new(
                    Controller::Ai(AiProfileKey::new("bench_stub")),
                    Default::default(),
                    false,
                    modifiers,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        1_440,
        graph,
        countries,
        [],
    )
    .unwrap()
}

fn tick(c: &mut Criterion) {
    let mut state = synthetic_state();
    c.bench_function("tick/synthetic_400_provinces_8_countries", |b| {
        b.iter(|| black_box(state.tick(|_, _| Vec::new()).unwrap()));
    });
}

criterion_group!(benches, tick);
criterion_main!(benches);
