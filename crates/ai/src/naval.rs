use strategy_sim::{
    Command, CommandActor, CountryId, CountryPolity, EdgeKind, Fleet, GameState, NavyState,
    ProvinceGraph, ProvinceId, War, WarSide,
};

use crate::{NavalReason, Reason};

const ORDER_PERIOD: u64 = 3;

/// L3 navy planner prefers an amphibious objective, then an unopposed port
/// blockade. Every order passes through the normal command validator.
pub(super) fn decide(
    state: &GameState,
    country: CountryId,
    nation: &CountryPolity,
) -> Option<(Command, Reason)> {
    if (state.time().tick().get() + u64::from(country.get())) % ORDER_PERIOD != 0 {
        return None;
    }
    let military = state.war_state()?;
    let (_, war) = military.war_for(country)?;
    let navy = state.navy()?;
    if war.side(country) == Some(WarSide::Attacker) {
        if let Some(order) = decide_transport(state, country, nation, military, war, navy) {
            return Some(order);
        }
    }
    decide_blockade(state, country, war, navy)
}

fn decide_transport(
    state: &GameState,
    country: CountryId,
    nation: &CountryPolity,
    military: &strategy_sim::WarState,
    war: &War,
    navy: &NavyState,
) -> Option<(Command, Reason)> {
    let graph = state.province_graph();
    let goal = graph.get(war.goal)?;
    let actor = CommandActor::Ai(country);

    for (&fleet_id, fleet) in &navy.fleets {
        if fleet.country != country || fleet.movement.is_some() {
            continue;
        }
        // Avoid ordering a transport into a sea battle it cannot win. A
        // stronger escort may still enter through the ordinary MoveFleet
        // command and the host resolves the battle on arrival.
        let safe_to_sail = |target| route_is_safe(graph, navy, war, fleet, target);
        let Some(&target_sea) = goal.ports.iter().find(|&&sea| safe_to_sail(sea)) else {
            continue;
        };
        if fleet.cargo.is_some() {
            let (command, reason) = if fleet.sea == target_sea {
                (
                    Command::DisembarkArmy {
                        country,
                        fleet: fleet_id,
                        destination: war.goal,
                    },
                    NavalReason::LandArmy,
                )
            } else {
                (
                    Command::MoveFleet {
                        country,
                        fleet: fleet_id,
                        destination: target_sea,
                    },
                    NavalReason::SailToGoal,
                )
            };
            if state.validate_command(actor, &command).is_ok() {
                return Some((command, Reason::Naval(reason)));
            }
            continue;
        }

        for (&army_id, army) in &military.armies {
            if army.country != country || army.movement.is_some() {
                continue;
            }
            if u64::from(army.soldiers)
                > u64::from(fleet.ships) * u64::from(strategy_sim::SHIP_CAPACITY)
            {
                continue;
            }
            for &port_id in &nation.provinces {
                let port = graph.get(port_id)?;
                let Some(&port_sea) = port.ports.iter().find(|&&sea| safe_to_sail(sea)) else {
                    continue;
                };
                if port
                    .neighbors
                    .iter()
                    .all(|edge| edge.to != port_sea || edge.kind != EdgeKind::Port)
                    || state
                        .find_army_path(country, army.province, port_id)
                        .is_none()
                {
                    continue;
                }
                let (command, reason) = if fleet.sea != port_sea {
                    (
                        Command::MoveFleet {
                            country,
                            fleet: fleet_id,
                            destination: port_sea,
                        },
                        NavalReason::SailToPort,
                    )
                } else if army.province != port_id {
                    (
                        Command::MoveArmy {
                            country,
                            army: army_id,
                            destination: port_id,
                        },
                        NavalReason::MarchToPort,
                    )
                } else {
                    (
                        Command::EmbarkArmy {
                            country,
                            army: army_id,
                            fleet: fleet_id,
                        },
                        NavalReason::EmbarkArmy,
                    )
                };
                if state.validate_command(actor, &command).is_ok() {
                    return Some((command, Reason::Naval(reason)));
                }
            }
        }
    }
    None
}

fn decide_blockade(
    state: &GameState,
    country: CountryId,
    war: &War,
    navy: &NavyState,
) -> Option<(Command, Reason)> {
    let graph = state.province_graph();
    let blocked = state.blockaded_ports();
    let ports: Vec<_> = state
        .polity()?
        .countries
        .iter()
        .filter(|(&owner, _)| war.opposed(country, owner))
        .flat_map(|(_, nation)| &nation.provinces)
        .filter_map(|&id| graph.get(id))
        .filter(|province| !province.ports.is_empty())
        .collect();
    for (&fleet_id, fleet) in &navy.fleets {
        if fleet.country != country || fleet.movement.is_some() || fleet.cargo.is_some() {
            continue;
        }
        // Keep a fleet in place while it already closes at least one port.
        if ports
            .iter()
            .any(|port| blocked.contains(&port.id) && port.ports.contains(&fleet.sea))
        {
            continue;
        }
        for port in &ports {
            if blocked.contains(&port.id) {
                continue;
            }
            for &sea in &port.ports {
                if sea == fleet.sea || !route_is_safe(graph, navy, war, fleet, sea) {
                    continue;
                }
                let command = Command::MoveFleet {
                    country,
                    fleet: fleet_id,
                    destination: sea,
                };
                if state
                    .validate_command(CommandActor::Ai(country), &command)
                    .is_ok()
                {
                    return Some((command, Reason::Naval(NavalReason::BlockadePort)));
                }
            }
        }
    }
    None
}

fn route_is_safe(
    graph: &ProvinceGraph,
    navy: &NavyState,
    war: &War,
    fleet: &Fleet,
    target: ProvinceId,
) -> bool {
    graph.sea_route(fleet.sea, target).is_some_and(|route| {
        route.iter().all(|edge| {
            let enemy_ships: u64 = navy
                .fleets
                .values()
                .filter(|other| other.sea == edge.to && war.opposed(fleet.country, other.country))
                .map(|other| u64::from(other.ships))
                .sum();
            u64::from(fleet.ships) > enemy_ships
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU32,
    };
    use strategy_sim::{
        AiProfileKey, AiThresholds, AiWeights, Controller, CountryState, Date, EconomyState,
        FiscalAiProfile, Fixed, FleetId, GameState, MapIdentity, ModKey, ModifierStore, PlayerId,
        PolityRules, PolityState, Province, ProvinceEdge, ProvinceKind, SimTime, Speed,
        SuccessionState, Tick, WarId, WarRules, WarState,
    };

    #[test]
    fn ai_avoids_equal_or_stronger_wartime_fleet_on_its_sea_route() {
        let province = |id, target| Province {
            id: ProvinceId::new(id),
            key: format!("sea_{id}"),
            kind: ProvinceKind::Sea,
            terrain: "sea".into(),
            centroid: [id as i32, 0],
            development: 0,
            neighbors: vec![ProvinceEdge {
                to: ProvinceId::new(target),
                kind: EdgeKind::Normal,
                cost: 1,
            }],
            ports: vec![],
        };
        let graph = ProvinceGraph::try_new(
            MapIdentity {
                version: NonZeroU32::new(1).unwrap(),
                sha256: [0; 32],
            },
            [province(1, 2), province(2, 1)],
        )
        .unwrap();
        let own = Fleet {
            country: CountryId::new(1),
            sea: ProvinceId::new(1),
            ships: 2,
            movement: None,
            cargo: None,
        };
        let mut enemy = Fleet {
            country: CountryId::new(2),
            sea: ProvinceId::new(2),
            ships: 2,
            movement: None,
            cargo: None,
        };
        let war = War {
            attacker: own.country,
            defender: enemy.country,
            attacker_supporters: BTreeSet::new(),
            defender_supporters: BTreeSet::new(),
            goal: ProvinceId::new(1),
            started_at: Tick::ZERO,
            score: 0,
            occupations: BTreeMap::new(),
            offer: None,
            independence: false,
        };
        let mut navy = NavyState {
            fleets: BTreeMap::from([
                (FleetId::new(1), own.clone()),
                (FleetId::new(2), enemy.clone()),
            ]),
            next_fleet_id: 3,
        };
        assert!(!route_is_safe(
            &graph,
            &navy,
            &war,
            &own,
            ProvinceId::new(2)
        ));
        enemy.ships = 1;
        navy.fleets.insert(FleetId::new(2), enemy);
        assert!(route_is_safe(&graph, &navy, &war, &own, ProvinceId::new(2)));
    }

    #[test]
    fn defender_ai_sails_to_blockade_a_port_then_holds_the_zone() {
        let a = CountryId::new(1);
        let b = CountryId::new(3);
        let province = |id, kind, neighbors: &[(u32, EdgeKind)], ports: &[u32]| Province {
            id: ProvinceId::new(id),
            key: format!("province_{id}"),
            kind,
            terrain: "plains".into(),
            centroid: [id as i32, 0],
            development: 1,
            neighbors: neighbors
                .iter()
                .map(|&(to, kind)| ProvinceEdge {
                    to: ProvinceId::new(to),
                    kind,
                    cost: 1,
                })
                .collect(),
            ports: ports.iter().copied().map(ProvinceId::new).collect(),
        };
        let graph = ProvinceGraph::try_new(
            MapIdentity {
                version: NonZeroU32::new(3).unwrap(),
                sha256: [7; 32],
            },
            [
                province(1, ProvinceKind::Land, &[(3, EdgeKind::Port)], &[3]),
                province(
                    2,
                    ProvinceKind::Land,
                    &[(4, EdgeKind::Port), (5, EdgeKind::Normal)],
                    &[4],
                ),
                province(
                    3,
                    ProvinceKind::Sea,
                    &[(1, EdgeKind::Port), (4, EdgeKind::Normal)],
                    &[],
                ),
                province(
                    4,
                    ProvinceKind::Sea,
                    &[(2, EdgeKind::Port), (3, EdgeKind::Normal)],
                    &[],
                ),
                province(5, ProvinceKind::Land, &[(2, EdgeKind::Normal)], &[]),
            ],
        )
        .unwrap();
        let nation = |tag: &str, capital, provinces: &[u32]| CountryPolity {
            tag: tag.into(),
            government: "monarchy".into(),
            name: tag.into(),
            color: [0, 0, 0],
            capital: ProvinceId::new(capital),
            provinces: provinces.iter().copied().map(ProvinceId::new).collect(),
            ai_profile: AiProfileKey::new("test_profile"),
            economy: EconomyState::default(),
            succession: SuccessionState::default(),
        };
        let polity = PolityState {
            rules: PolityRules {
                adulthood_years: 18,
                base_taxable_development_per_province: 0,
                tax_per_development: Fixed::ONE,
                max_tax_rate: Fixed::ONE,
                max_monthly_spending: Fixed::ZERO,
                debt_limit: Fixed::ZERO,
                income_modifier: ModKey::new("income"),
            },
            countries: BTreeMap::from([
                (a, nation("first", 1, &[1])),
                (b, nation("second", 5, &[2, 5])),
            ]),
            characters: BTreeMap::new(),
            ai_profiles: BTreeMap::from([(
                "test_profile".into(),
                FiscalAiProfile {
                    idea_priority: 0,
                    reform_priority: 0,
                    weights: AiWeights::default(),
                    thresholds: AiThresholds::default(),
                    preferred_tax_rate: Fixed::ZERO,
                    preferred_monthly_spending: Fixed::ZERO,
                    tech_branch_weights: BTreeMap::new(),
                },
            )]),
        };
        let mut wars = WarState::new(
            WarRules {
                raise_cost_per_soldier: Fixed::ZERO,
                max_army_size: 5000,
                max_morale: Fixed::ONE,
                morale_recovery_per_tick: Fixed::ZERO,
                retreat_morale_threshold: Fixed::ZERO,
                winner_casualty_rate: Fixed::ZERO,
                loser_casualty_rate: Fixed::ZERO,
                battle_morale_loss: Fixed::ZERO,
                base_supply: 5000,
                supply_per_development: 0,
                attrition_rate: Fixed::ZERO,
                siege_days_per_level: 2,
                battle_score: 10,
                occupation_score: 30,
                peace_score: 40,
                truce_ticks: 180,
                combat_modifier: ModKey::new("army_combat"),
                morale_modifier: ModKey::new("army_morale"),
            },
            BTreeMap::new(),
        );
        wars.wars.insert(
            WarId::new(1),
            War {
                attacker: a,
                defender: b,
                attacker_supporters: BTreeSet::new(),
                defender_supporters: BTreeSet::new(),
                goal: ProvinceId::new(2),
                started_at: Tick::ZERO,
                score: 0,
                occupations: BTreeMap::new(),
                offer: None,
                independence: false,
            },
        );
        wars.next_war_id = 2;
        let navy = NavyState {
            fleets: BTreeMap::from([(
                FleetId::new(1),
                Fleet {
                    country: b,
                    sea: ProvinceId::new(4),
                    ships: 2,
                    movement: None,
                    cargo: None,
                },
            )]),
            next_fleet_id: 2,
        };
        let mut state = GameState::new(
            SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
            42,
            graph,
            BTreeMap::from([
                (
                    a,
                    CountryState::new(
                        Controller::Human(PlayerId::new(1)),
                        BTreeSet::new(),
                        false,
                        ModifierStore::default(),
                    ),
                ),
                (
                    b,
                    CountryState::new(
                        Controller::Ai(AiProfileKey::new("test_profile")),
                        BTreeSet::new(),
                        false,
                        ModifierStore::default(),
                    ),
                ),
            ]),
            [],
        )
        .unwrap()
        .with_polity(polity)
        .unwrap()
        .with_war_state(wars)
        .unwrap()
        .with_navy(navy)
        .unwrap();
        let mut context = crate::AiContext::new(&state, b);
        let (command, reason) = crate::decide(&state, b, &mut context)
            .into_iter()
            .find(|(_, reason)| reason.label() == "BlockadePort")
            .expect("defender AI should sail to an unguarded enemy port");
        assert_eq!(
            command,
            Command::MoveFleet {
                country: b,
                fleet: FleetId::new(1),
                destination: ProvinceId::new(3),
            }
        );
        assert_eq!(reason.label(), "BlockadePort");
        state.apply(CommandActor::Ai(b), &command).unwrap();
        state.tick(|_, _| vec![]).unwrap();
        assert!(state.is_port_blockaded(ProvinceId::new(1)));
        for _ in 0..2 {
            state.tick(|_, _| vec![]).unwrap();
        }
        let mut context = crate::AiContext::new(&state, b);
        assert!(crate::decide(&state, b, &mut context)
            .into_iter()
            .all(|(_, reason)| reason.label() != "BlockadePort"));
    }
}
