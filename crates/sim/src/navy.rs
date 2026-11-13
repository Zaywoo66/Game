use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ArmyId, CountryId, EdgeKind, FleetId, ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind,
    War, WarId, WarSide,
};

/// One transport ship carries this many whole soldiers. The ship type is fixed in M8.2.
pub const SHIP_CAPACITY: u32 = 1_000;
pub const SHIP_BUILD_COST_RAW: i64 = 5_000;
pub const SHIP_BUILD_DAYS: u8 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fleet {
    pub country: CountryId,
    /// A fleet is always in a sea province, including while a movement leg is underway.
    pub sea: ProvinceId,
    pub ships: u32,
    pub movement: Option<FleetMovement>,
    pub cargo: Option<ArmyId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetMovement {
    /// Remaining sea-to-sea legs, starting with the leg in progress.
    pub route: Vec<ProvinceEdge>,
    pub elapsed_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavyState {
    pub fleets: BTreeMap<FleetId, Fleet>,
    pub next_fleet_id: u32,
    pub shipyards: BTreeMap<ProvinceId, ShipBuildOrder>,
}

/// One ship under construction at an owned port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShipBuildOrder {
    pub country: CountryId,
    pub sea: ProvinceId,
    pub days_remaining: u8,
}

/// The result of one deterministic battle in one sea zone on one day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavalBattle {
    pub war: WarId,
    pub sea: ProvinceId,
    pub winner: Option<WarSide>,
    pub attacker_losses: u64,
    pub defender_losses: u64,
    pub changed_fleets: Vec<FleetId>,
    pub lost_armies: Vec<ArmyId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FleetAttrition {
    pub fleet: FleetId,
    pub lost_army: Option<ArmyId>,
}

impl Default for NavyState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavyError {
    AlreadyConfigured,
    Unavailable,
    UnknownFleet(FleetId),
    NotFleetOwner(FleetId),
    InvalidFleet(FleetId),
    InvalidFleetIdSequence,
    InvalidSea(ProvinceId),
    Unreachable(ProvinceId),
    UnknownCountry(CountryId),
    UnknownArmy(ArmyId),
    CargoAlreadyAboard(ArmyId),
    InsufficientCapacity(FleetId),
    FleetInMotion(FleetId),
    FleetHasCargo(FleetId),
    NoCargo(FleetId),
    ArmyInMotion(ArmyId),
    InvalidPort(ProvinceId),
    NotAtWar,
    HostileFleet(FleetId),
    ShipyardOccupied(ProvinceId),
    InvalidShipyard(ProvinceId),
    InsufficientFunds,
}

impl NavyState {
    pub fn new() -> Self {
        Self {
            fleets: BTreeMap::new(),
            next_fleet_id: 1,
            shipyards: BTreeMap::new(),
        }
    }

    pub(crate) fn enqueue_ship(&mut self, port: ProvinceId, order: ShipBuildOrder) {
        self.shipyards.insert(port, order);
    }

    /// Advances projects one game day and returns completed fleets and canceled projects.
    pub(crate) fn advance_shipyards(
        &mut self,
        owner_of: impl Fn(ProvinceId) -> Option<CountryId>,
    ) -> (Vec<(ProvinceId, FleetId)>, Vec<ProvinceId>) {
        let mut completed = Vec::new();
        let mut cancelled = Vec::new();
        let ports: Vec<_> = self.shipyards.keys().copied().collect();
        for port in ports {
            let country = self.shipyards.get(&port).expect("port collected").country;
            if owner_of(port) != Some(country) {
                self.shipyards.remove(&port);
                cancelled.push(port);
                continue;
            }
            let completed_order = {
                let order = self.shipyards.get_mut(&port).expect("port collected");
                order.days_remaining = order.days_remaining.saturating_sub(1);
                (order.days_remaining == 0).then(|| order.clone())
            };
            let Some(order) = completed_order else {
                continue;
            };
            let Some(next_id) = self.next_fleet_id.checked_add(1) else {
                self.shipyards
                    .get_mut(&port)
                    .expect("order retained")
                    .days_remaining = 1;
                continue;
            };
            self.shipyards.remove(&port);
            let fleet_id = FleetId::new(self.next_fleet_id);
            self.next_fleet_id = next_id;
            self.fleets.insert(
                fleet_id,
                Fleet {
                    country: order.country,
                    sea: order.sea,
                    ships: 1,
                    movement: None,
                    cargo: None,
                },
            );
            completed.push((port, fleet_id));
        }
        (completed, cancelled)
    }

    /// Checks scenario/save topology and stable identifiers before accepting state.
    pub fn validate(
        &self,
        graph: &ProvinceGraph,
        countries: impl IntoIterator<Item = CountryId>,
        armies: impl IntoIterator<Item = (ArmyId, CountryId, u32, bool)>,
    ) -> Result<(), NavyError> {
        let countries: BTreeSet<_> = countries.into_iter().collect();
        let armies: BTreeMap<_, _> = armies
            .into_iter()
            .map(|(id, country, soldiers, moving)| (id, (country, soldiers, moving)))
            .collect();
        let mut cargo = BTreeSet::new();
        let mut last_id = 0;
        for (&id, fleet) in &self.fleets {
            if id.get() == 0 || id.get() <= last_id || id.get() >= self.next_fleet_id {
                return Err(NavyError::InvalidFleetIdSequence);
            }
            last_id = id.get();
            if !countries.contains(&fleet.country) {
                return Err(NavyError::UnknownCountry(fleet.country));
            }
            if fleet.ships == 0
                || graph
                    .get(fleet.sea)
                    .is_none_or(|p| p.kind != ProvinceKind::Sea)
            {
                return Err(NavyError::InvalidFleet(id));
            }
            if let Some(movement) = &fleet.movement {
                if movement.route.is_empty() || movement.elapsed_ticks >= movement.route[0].cost {
                    return Err(NavyError::InvalidFleet(id));
                }
                let mut at = fleet.sea;
                for &edge in &movement.route {
                    if graph
                        .neighbors(at)
                        .is_none_or(|neighbors| !neighbors.contains(&edge))
                        || graph
                            .get(edge.to)
                            .is_none_or(|p| p.kind != ProvinceKind::Sea)
                        || !matches!(edge.kind, crate::EdgeKind::Normal | crate::EdgeKind::Strait)
                    {
                        return Err(NavyError::InvalidFleet(id));
                    }
                    at = edge.to;
                }
            }
            if let Some(army_id) = fleet.cargo {
                let Some(&(country, soldiers, moving)) = armies.get(&army_id) else {
                    return Err(NavyError::UnknownArmy(army_id));
                };
                if moving {
                    return Err(NavyError::ArmyInMotion(army_id));
                }
                if country != fleet.country || !cargo.insert(army_id) {
                    return Err(NavyError::CargoAlreadyAboard(army_id));
                }
                if u64::from(soldiers) > u64::from(fleet.ships) * u64::from(SHIP_CAPACITY) {
                    return Err(NavyError::InsufficientCapacity(id));
                }
            }
        }
        for (&port, order) in &self.shipyards {
            if !countries.contains(&order.country) {
                return Err(NavyError::UnknownCountry(order.country));
            }
            let valid_port = graph.get(port).is_some_and(|province| {
                province.kind == ProvinceKind::Land
                    && province.ports.contains(&order.sea)
                    && province
                        .neighbors
                        .iter()
                        .any(|edge| edge.to == order.sea && edge.kind == EdgeKind::Port)
            });
            if !valid_port || order.days_remaining == 0 || order.days_remaining > SHIP_BUILD_DAYS {
                return Err(NavyError::InvalidShipyard(port));
            }
        }
        if self.next_fleet_id == 0 || self.next_fleet_id <= last_id {
            return Err(NavyError::InvalidFleetIdSequence);
        }
        Ok(())
    }

    pub fn fleet_route(
        &self,
        graph: &ProvinceGraph,
        fleet_id: FleetId,
        country: CountryId,
        destination: ProvinceId,
    ) -> Result<Vec<ProvinceEdge>, NavyError> {
        let fleet = self
            .fleets
            .get(&fleet_id)
            .ok_or(NavyError::UnknownFleet(fleet_id))?;
        if fleet.country != country {
            return Err(NavyError::NotFleetOwner(fleet_id));
        }
        if fleet.movement.is_some() {
            return Err(NavyError::FleetInMotion(fleet_id));
        }
        if fleet.sea == destination
            || graph
                .get(destination)
                .is_none_or(|p| p.kind != ProvinceKind::Sea)
        {
            return Err(NavyError::InvalidSea(destination));
        }
        graph
            .sea_route(fleet.sea, destination)
            .filter(|route| !route.is_empty())
            .ok_or(NavyError::Unreachable(destination))
    }

    /// Advances sea routes and returns each moved fleet's previous sea zone.
    pub fn advance(&mut self) -> BTreeMap<FleetId, ProvinceId> {
        let mut previous = BTreeMap::new();
        for (&id, fleet) in &mut self.fleets {
            let Some(movement) = &mut fleet.movement else {
                continue;
            };
            previous.insert(id, fleet.sea);
            movement.elapsed_ticks += 1;
            if movement.elapsed_ticks == movement.route[0].cost {
                fleet.sea = movement.route.remove(0).to;
                movement.elapsed_ticks = 0;
                if movement.route.is_empty() {
                    fleet.movement = None;
                }
            }
        }
        previous
    }

    /// Applies monthly at-sea losses after movement and battles. A stationary
    /// fleet in a sea zone adjacent to an accessible port is sheltered.
    pub(crate) fn apply_monthly_attrition(
        &mut self,
        graph: &ProvinceGraph,
        sheltered_ports: &BTreeSet<(CountryId, ProvinceId)>,
        mut army_strength: impl FnMut(ArmyId) -> Option<u32>,
    ) -> Vec<FleetAttrition> {
        let mut changed = Vec::new();
        let ids: Vec<_> = self.fleets.keys().copied().collect();
        for id in ids {
            let fleet = self.fleets.get_mut(&id).expect("fleet ID came from map");
            let sheltered = fleet.movement.is_none()
                && graph.neighbors(fleet.sea).is_some_and(|edges| {
                    edges.iter().any(|edge| {
                        edge.kind == EdgeKind::Port
                            && sheltered_ports.contains(&(fleet.country, edge.to))
                    })
                });
            if sheltered {
                continue;
            }

            fleet.ships -= fleet.ships.div_ceil(100);
            let capacity_loss = fleet.cargo.filter(|army| {
                army_strength(*army).is_none_or(|soldiers| {
                    u64::from(soldiers) > u64::from(fleet.ships) * u64::from(SHIP_CAPACITY)
                })
            });
            let destroyed = fleet.ships == 0;
            let lost_army = capacity_loss.or_else(|| destroyed.then_some(fleet.cargo).flatten());
            if lost_army.is_some() {
                fleet.cargo = None;
            }
            if destroyed {
                self.fleets.remove(&id);
            }
            changed.push(FleetAttrition {
                fleet: id,
                lost_army,
            });
        }
        changed
    }

    /// Resolves at most one battle for each contested zone and war, in sorted
    /// war/zone/fleet order. Peaceful fleets never participate.
    pub fn resolve_battles<'a>(
        &mut self,
        graph: &ProvinceGraph,
        wars: impl IntoIterator<Item = (WarId, &'a War)>,
        previous: &BTreeMap<FleetId, ProvinceId>,
        army_strength: impl Fn(ArmyId) -> Option<u32>,
    ) -> Vec<NavalBattle> {
        let mut battles = Vec::new();
        let mut engaged = BTreeSet::new();
        for (war_id, war) in wars {
            let zones: BTreeSet<_> = self.fleets.values().map(|fleet| fleet.sea).collect();
            for sea in zones {
                let attacker: Vec<_> = self
                    .fleets
                    .iter()
                    .filter(|(id, fleet)| {
                        !engaged.contains(*id)
                            && fleet.sea == sea
                            && war.side(fleet.country) == Some(WarSide::Attacker)
                    })
                    .map(|(&id, _)| id)
                    .collect();
                let defender: Vec<_> = self
                    .fleets
                    .iter()
                    .filter(|(id, fleet)| {
                        !engaged.contains(*id)
                            && fleet.sea == sea
                            && war.side(fleet.country) == Some(WarSide::Defender)
                    })
                    .map(|(&id, _)| id)
                    .collect();
                if attacker.is_empty() || defender.is_empty() {
                    continue;
                }
                engaged.extend(attacker.iter().chain(&defender).copied());
                let a_total: u64 = attacker
                    .iter()
                    .map(|id| u64::from(self.fleets[id].ships))
                    .sum();
                let d_total: u64 = defender
                    .iter()
                    .map(|id| u64::from(self.fleets[id].ships))
                    .sum();
                let winner = match a_total.cmp(&d_total) {
                    std::cmp::Ordering::Greater => Some(WarSide::Attacker),
                    std::cmp::Ordering::Less => Some(WarSide::Defender),
                    std::cmp::Ordering::Equal => None,
                };
                let a_pct = if winner == Some(WarSide::Defender) {
                    25
                } else {
                    10
                };
                let d_pct = if winner == Some(WarSide::Attacker) {
                    25
                } else {
                    10
                };
                let a_losses = a_total.saturating_mul(a_pct).div_ceil(100).min(a_total);
                let d_losses = d_total.saturating_mul(d_pct).div_ceil(100).min(d_total);
                let mut changed = Vec::new();
                let mut lost_armies = Vec::new();
                for (side, ids, losses) in [
                    (WarSide::Attacker, &attacker, a_losses),
                    (WarSide::Defender, &defender, d_losses),
                ] {
                    let mut remaining = losses;
                    for &id in ids {
                        let fleet = self.fleets.get_mut(&id).expect("battle fleet exists");
                        let removed = u64::from(fleet.ships).min(remaining) as u32;
                        fleet.ships -= removed;
                        remaining -= u64::from(removed);
                        changed.push(id);
                        if let Some(army) = fleet.cargo {
                            if army_strength(army).is_none_or(|soldiers| {
                                u64::from(soldiers)
                                    > u64::from(fleet.ships) * u64::from(SHIP_CAPACITY)
                            }) {
                                lost_armies.push(army);
                                fleet.cargo = None;
                            }
                        }
                    }
                    if winner != Some(side) {
                        for &id in ids {
                            let fleet = self.fleets.get(&id).expect("battle fleet exists");
                            let country = fleet.country;
                            let alive = fleet.ships > 0;
                            let retreat = if alive {
                                let occupied = |target: ProvinceId| {
                                    self.fleets.iter().any(|(&other_id, other)| {
                                        other_id != id
                                            && other.sea == target
                                            && war.opposed(country, other.country)
                                    })
                                };
                                let origin = previous
                                    .get(&id)
                                    .copied()
                                    .filter(|&target| target != sea && !occupied(target));
                                let alternate = graph
                                    .neighbors(sea)
                                    .into_iter()
                                    .flatten()
                                    .filter(|edge| {
                                        matches!(edge.kind, EdgeKind::Normal | EdgeKind::Strait)
                                    })
                                    .filter(|edge| {
                                        graph
                                            .get(edge.to)
                                            .is_some_and(|p| p.kind == ProvinceKind::Sea)
                                            && !occupied(edge.to)
                                    })
                                    .map(|edge| edge.to)
                                    .min();
                                origin.or(alternate).unwrap_or(sea)
                            } else {
                                sea
                            };
                            let fleet = self.fleets.get_mut(&id).expect("battle fleet exists");
                            fleet.movement = None;
                            fleet.sea = retreat;
                        }
                    }
                }
                for &id in attacker.iter().chain(&defender) {
                    if self.fleets.get(&id).is_some_and(|fleet| fleet.ships == 0) {
                        self.fleets.remove(&id);
                    }
                }
                battles.push(NavalBattle {
                    war: war_id,
                    sea,
                    winner,
                    attacker_losses: a_losses,
                    defender_losses: d_losses,
                    changed_fleets: changed,
                    lost_armies,
                });
            }
        }
        battles
    }
}
