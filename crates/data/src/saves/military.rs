use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strategy_sim::{
    Army, ArmyId, ArmyMovement, CountryId, EdgeKind, Fixed, MovementLeg, PeaceOffer, PeaceTreaty,
    ProvinceId, Siege, Tick, War, WarId, WarState,
};

use super::{require_sorted, required_optional, SaveError};
use crate::{provinces::EdgeKindDef, war_format::WarRulesFile};

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WarStateFile {
    rules: WarRulesFile,
    armies: Vec<ArmyFile>,
    wars: Vec<WarFile>,
    forts: Vec<FortFile>,
    sieges: Vec<SiegeFile>,
    treaties: Vec<TreatyFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    coalition_truces: Vec<CoalitionTruceFile>,
    next_army_id: u32,
    next_war_id: u32,
}

impl From<&WarState> for WarStateFile {
    fn from(state: &WarState) -> Self {
        Self {
            rules: (&state.rules).into(),
            armies: state
                .armies
                .iter()
                .map(|(&id, army)| ArmyFile::snapshot(id, army))
                .collect(),
            wars: state
                .wars
                .iter()
                .map(|(&id, war)| WarFile::snapshot(id, war))
                .collect(),
            forts: state
                .forts
                .iter()
                .map(|(province, &level)| FortFile {
                    province: province.get(),
                    level,
                })
                .collect(),
            sieges: state
                .sieges
                .iter()
                .map(|(province, siege)| SiegeFile {
                    province: province.get(),
                    war: siege.war.get(),
                    besieger: siege.besieger.get(),
                    progress_days: siege.progress_days,
                })
                .collect(),
            treaties: state
                .treaties
                .iter()
                .map(|(war, treaty)| TreatyFile {
                    war: war.get(),
                    attacker: treaty.attacker.get(),
                    defender: treaty.defender.get(),
                    ceded: treaty.ceded.map(ProvinceId::get),
                    concluded_at: treaty.concluded_at.get(),
                })
                .collect(),
            coalition_truces: state
                .coalition_truces
                .iter()
                .map(|(&(a, b), &concluded_at)| CoalitionTruceFile {
                    a: a.get(),
                    b: b.get(),
                    concluded_at: concluded_at.get(),
                })
                .collect(),
            next_army_id: state.next_army_id,
            next_war_id: state.next_war_id,
        }
    }
}

impl WarStateFile {
    pub(super) fn has_coalition_history(&self) -> bool {
        !self.coalition_truces.is_empty()
            || self.wars.iter().any(|war| {
                !war.attacker_supporters.is_empty() || !war.defender_supporters.is_empty()
            })
    }

    pub(super) fn restore(self) -> Result<WarState, SaveError> {
        require_sorted(&self.armies, "armies", |army| army.id)?;
        require_sorted(&self.wars, "wars", |war| war.id)?;
        require_sorted(&self.forts, "forts", |fort| fort.province)?;
        require_sorted(&self.sieges, "sieges", |siege| siege.province)?;
        require_sorted(&self.treaties, "treaties", |treaty| treaty.war)?;
        require_sorted(&self.coalition_truces, "coalition truces", |truce| {
            (truce.a, truce.b)
        })?;
        Ok(WarState {
            rules: self.rules.into(),
            armies: self.armies.into_iter().map(ArmyFile::restore).collect(),
            wars: self
                .wars
                .into_iter()
                .map(WarFile::restore)
                .collect::<Result<BTreeMap<_, _>, _>>()?,
            forts: self
                .forts
                .into_iter()
                .map(|fort| (ProvinceId::new(fort.province), fort.level))
                .collect(),
            sieges: self
                .sieges
                .into_iter()
                .map(|siege| {
                    (
                        ProvinceId::new(siege.province),
                        Siege {
                            war: WarId::new(siege.war),
                            besieger: CountryId::new(siege.besieger),
                            progress_days: siege.progress_days,
                        },
                    )
                })
                .collect(),
            treaties: self
                .treaties
                .into_iter()
                .map(|treaty| {
                    (
                        WarId::new(treaty.war),
                        PeaceTreaty {
                            attacker: CountryId::new(treaty.attacker),
                            defender: CountryId::new(treaty.defender),
                            ceded: treaty.ceded.map(ProvinceId::new),
                            concluded_at: Tick::new(treaty.concluded_at),
                        },
                    )
                })
                .collect(),
            coalition_truces: self
                .coalition_truces
                .into_iter()
                .map(|truce| {
                    (
                        (CountryId::new(truce.a), CountryId::new(truce.b)),
                        Tick::new(truce.concluded_at),
                    )
                })
                .collect(),
            next_army_id: self.next_army_id,
            next_war_id: self.next_war_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArmyFile {
    id: u32,
    country: u32,
    province: u32,
    soldiers: u32,
    morale: i64,
    #[serde(deserialize_with = "required_optional")]
    movement: Option<MovementFile>,
}

impl ArmyFile {
    fn snapshot(id: ArmyId, army: &Army) -> Self {
        Self {
            id: id.get(),
            country: army.country.get(),
            province: army.province.get(),
            soldiers: army.soldiers,
            morale: army.morale.raw(),
            movement: army.movement.as_ref().map(|movement| MovementFile {
                route: movement
                    .route
                    .iter()
                    .map(|leg| LegFile {
                        to: leg.to.get(),
                        kind: leg.kind,
                        cost: leg.cost,
                    })
                    .collect(),
                elapsed_ticks: movement.elapsed_ticks,
            }),
        }
    }

    fn restore(self) -> (ArmyId, Army) {
        (
            ArmyId::new(self.id),
            Army {
                country: CountryId::new(self.country),
                province: ProvinceId::new(self.province),
                soldiers: self.soldiers,
                morale: Fixed::from_raw(self.morale),
                movement: self.movement.map(|movement| ArmyMovement {
                    route: movement
                        .route
                        .into_iter()
                        .map(|leg| MovementLeg {
                            to: ProvinceId::new(leg.to),
                            kind: leg.kind,
                            cost: leg.cost,
                        })
                        .collect(),
                    elapsed_ticks: movement.elapsed_ticks,
                }),
            },
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MovementFile {
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WarFile {
    id: u32,
    attacker: u32,
    defender: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attacker_supporters: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    defender_supporters: Vec<u32>,
    goal: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    independence: bool,
    started_at: u64,
    score: i32,
    occupations: Vec<OccupationFile>,
    #[serde(deserialize_with = "required_optional")]
    offer: Option<OfferFile>,
}

impl WarFile {
    fn snapshot(id: WarId, war: &War) -> Self {
        Self {
            id: id.get(),
            attacker: war.attacker.get(),
            defender: war.defender.get(),
            attacker_supporters: war.attacker_supporters.iter().map(|id| id.get()).collect(),
            defender_supporters: war.defender_supporters.iter().map(|id| id.get()).collect(),
            goal: war.goal.get(),
            independence: war.independence,
            started_at: war.started_at.get(),
            score: war.score,
            occupations: war
                .occupations
                .iter()
                .map(|(province, country)| OccupationFile {
                    province: province.get(),
                    country: country.get(),
                })
                .collect(),
            offer: war.offer.as_ref().map(|offer| OfferFile {
                offered_by: offer.offered_by.get(),
                cede_goal: offer.cede_goal,
            }),
        }
    }

    fn restore(self) -> Result<(WarId, War), SaveError> {
        require_sorted(&self.attacker_supporters, "attacker supporters", |&id| id)?;
        require_sorted(&self.defender_supporters, "defender supporters", |&id| id)?;
        require_sorted(&self.occupations, "war occupations", |record| {
            record.province
        })?;
        Ok((
            WarId::new(self.id),
            War {
                attacker: CountryId::new(self.attacker),
                defender: CountryId::new(self.defender),
                attacker_supporters: self
                    .attacker_supporters
                    .into_iter()
                    .map(CountryId::new)
                    .collect(),
                defender_supporters: self
                    .defender_supporters
                    .into_iter()
                    .map(CountryId::new)
                    .collect(),
                goal: ProvinceId::new(self.goal),
                independence: self.independence,
                started_at: Tick::new(self.started_at),
                score: self.score,
                occupations: self
                    .occupations
                    .into_iter()
                    .map(|record| {
                        (
                            ProvinceId::new(record.province),
                            CountryId::new(record.country),
                        )
                    })
                    .collect(),
                offer: self.offer.map(|offer| PeaceOffer {
                    offered_by: CountryId::new(offer.offered_by),
                    cede_goal: offer.cede_goal,
                }),
            },
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OccupationFile {
    province: u32,
    country: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfferFile {
    offered_by: u32,
    cede_goal: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FortFile {
    province: u32,
    level: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SiegeFile {
    province: u32,
    war: u32,
    besieger: u32,
    progress_days: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreatyFile {
    war: u32,
    attacker: u32,
    defender: u32,
    #[serde(deserialize_with = "required_optional")]
    ceded: Option<u32>,
    concluded_at: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoalitionTruceFile {
    a: u32,
    b: u32,
    concluded_at: u64,
}
