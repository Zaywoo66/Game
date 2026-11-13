use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    ArmyId, CountryId, EdgeKind, Fleet, FleetId, FleetMovement, NavyState, ProvinceEdge,
    ProvinceId, ShipBuildOrder,
};

use super::{require_sorted, required_optional, SaveError};
use crate::provinces::EdgeKindDef;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NavyStateFile {
    fleets: Vec<FleetFile>,
    next_fleet_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shipyards: Vec<ShipyardFile>,
}

impl From<&NavyState> for NavyStateFile {
    fn from(state: &NavyState) -> Self {
        Self {
            fleets: state
                .fleets
                .iter()
                .map(|(&id, fleet)| FleetFile::snapshot(id, fleet))
                .collect(),
            next_fleet_id: state.next_fleet_id,
            shipyards: state
                .shipyards
                .iter()
                .map(|(&port, order)| ShipyardFile {
                    port: port.get(),
                    country: order.country.get(),
                    sea: order.sea.get(),
                    days_remaining: order.days_remaining,
                })
                .collect(),
        }
    }
}

impl NavyStateFile {
    pub(super) fn restore(self) -> Result<NavyState, SaveError> {
        require_sorted(&self.fleets, "fleets", |fleet| fleet.id)?;
        require_sorted(&self.shipyards, "shipyards", |order| order.port)?;
        Ok(NavyState {
            fleets: self
                .fleets
                .into_iter()
                .map(FleetFile::restore)
                .collect::<BTreeMap<_, _>>(),
            next_fleet_id: self.next_fleet_id,
            shipyards: self
                .shipyards
                .into_iter()
                .map(|order| {
                    (
                        ProvinceId::new(order.port),
                        ShipBuildOrder {
                            country: CountryId::new(order.country),
                            sea: ProvinceId::new(order.sea),
                            days_remaining: order.days_remaining,
                        },
                    )
                })
                .collect(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShipyardFile {
    port: u32,
    country: u32,
    sea: u32,
    days_remaining: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FleetFile {
    id: u32,
    country: u32,
    sea: u32,
    ships: u32,
    #[serde(deserialize_with = "required_optional")]
    movement: Option<FleetMovementFile>,
    #[serde(deserialize_with = "required_optional")]
    cargo: Option<u32>,
}

impl FleetFile {
    fn snapshot(id: FleetId, fleet: &Fleet) -> Self {
        Self {
            id: id.get(),
            country: fleet.country.get(),
            sea: fleet.sea.get(),
            ships: fleet.ships,
            movement: fleet.movement.as_ref().map(|movement| FleetMovementFile {
                route: movement.route.iter().map(LegFile::from).collect(),
                elapsed_ticks: movement.elapsed_ticks,
            }),
            cargo: fleet.cargo.map(ArmyId::get),
        }
    }

    fn restore(self) -> (FleetId, Fleet) {
        (
            FleetId::new(self.id),
            Fleet {
                country: CountryId::new(self.country),
                sea: ProvinceId::new(self.sea),
                ships: self.ships,
                movement: self.movement.map(|movement| FleetMovement {
                    route: movement.route.into_iter().map(LegFile::restore).collect(),
                    elapsed_ticks: movement.elapsed_ticks,
                }),
                cargo: self.cargo.map(ArmyId::new),
            },
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FleetMovementFile {
    route: Vec<LegFile>,
    elapsed_ticks: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegFile {
    to: u32,
    #[serde(with = "EdgeKindDef")]
    kind: EdgeKind,
    cost: u32,
}

impl From<&ProvinceEdge> for LegFile {
    fn from(edge: &ProvinceEdge) -> Self {
        Self {
            to: edge.to.get(),
            kind: edge.kind,
            cost: edge.cost,
        }
    }
}

impl LegFile {
    fn restore(self) -> ProvinceEdge {
        ProvinceEdge {
            to: ProvinceId::new(self.to),
            kind: self.kind,
            cost: self.cost,
        }
    }
}
