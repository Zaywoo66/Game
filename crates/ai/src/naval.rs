use strategy_sim::{
    Command, CommandActor, CountryId, CountryPolity, EdgeKind, GameState, ProvinceId, WarSide,
};

use crate::{NavalReason, Reason};

const ORDER_PERIOD: u64 = 3;

/// The first L3 transport planner follows a current war goal across sea.
/// It uses only read-only state and returns a normally validated command.
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
    if war.side(country) != Some(WarSide::Attacker) {
        return None;
    }
    let navy = state.navy()?;
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
        let safe_to_sail = |target: ProvinceId| {
            graph.sea_route(fleet.sea, target).is_some_and(|route| {
                route.iter().all(|edge| {
                    let enemy_ships: u64 = navy
                        .fleets
                        .values()
                        .filter(|other| other.sea == edge.to && war.opposed(country, other.country))
                        .map(|other| u64::from(other.ships))
                        .sum();
                    u64::from(fleet.ships) > enemy_ships
                })
            })
        };
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
