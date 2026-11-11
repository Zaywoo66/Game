use std::collections::{BTreeMap, BTreeSet};

use crate::{ArmyId, CountryId, FleetId, ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind};

/// One transport ship carries this many whole soldiers. The ship type is fixed in M8.2.
pub const SHIP_CAPACITY: u32 = 1_000;

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
}

impl NavyState {
    pub fn new() -> Self {
        Self {
            fleets: BTreeMap::new(),
            next_fleet_id: 1,
        }
    }

    /// Checks scenario/save topology and stable identifiers before accepting state.
    pub fn validate(
        &self,
        graph: &ProvinceGraph,
        countries: impl IntoIterator<Item = CountryId>,
        armies: impl IntoIterator<Item = (ArmyId, CountryId, u32)>,
    ) -> Result<(), NavyError> {
        let countries: BTreeSet<_> = countries.into_iter().collect();
        let armies: BTreeMap<_, _> = armies
            .into_iter()
            .map(|(id, country, soldiers)| (id, (country, soldiers)))
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
                let Some(&(country, soldiers)) = armies.get(&army_id) else {
                    return Err(NavyError::UnknownArmy(army_id));
                };
                if country != fleet.country || !cargo.insert(army_id) {
                    return Err(NavyError::CargoAlreadyAboard(army_id));
                }
                if u64::from(soldiers) > u64::from(fleet.ships) * u64::from(SHIP_CAPACITY) {
                    return Err(NavyError::InsufficientCapacity(id));
                }
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

    /// Advances one game day. Returns IDs of fleets whose position or route changed.
    pub fn advance(&mut self) -> Vec<FleetId> {
        let mut changed = Vec::new();
        for (&id, fleet) in &mut self.fleets {
            let Some(movement) = &mut fleet.movement else {
                continue;
            };
            movement.elapsed_ticks += 1;
            if movement.elapsed_ticks == movement.route[0].cost {
                fleet.sea = movement.route.remove(0).to;
                movement.elapsed_ticks = 0;
                if movement.route.is_empty() {
                    fleet.movement = None;
                }
            }
            changed.push(id);
        }
        changed
    }
}
