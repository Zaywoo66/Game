use std::collections::{BTreeMap, BTreeSet};

use crate::{ArmyId, CountryId, Fixed, PolityState, ProvinceGraph, ProvinceId, StateChange};

use super::{owner, MilitaryTickFailure, Siege, WarError, WarState};

#[derive(Clone, Copy)]
pub(crate) struct MilitaryBonuses {
    pub morale: Option<Fixed>,
    pub combat: Option<Fixed>,
}

impl WarState {
    pub(crate) fn advance(
        &mut self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        bonuses: &BTreeMap<CountryId, MilitaryBonuses>,
    ) -> (Vec<StateChange>, Vec<MilitaryTickFailure>) {
        let old_armies = self.armies.clone();
        let old_wars = self.wars.clone();
        let old_sieges = self.sieges.clone();
        let mut changes = Vec::new();
        let mut failures = BTreeSet::new();

        self.move_armies();
        self.apply_supply(graph);
        for army in self.armies.values_mut() {
            if let Some(limit) = bonuses[&army.country].morale {
                let restored = army
                    .morale
                    .raw()
                    .saturating_add(self.rules.morale_recovery_per_tick.raw());
                army.morale = Fixed::from_raw(restored.min(limit.raw()));
            } else {
                failures.insert(army.country);
            }
        }
        self.resolve_battles(graph, polity, bonuses, &mut changes, &mut failures);
        self.advance_sieges(polity);

        for &id in old_armies
            .keys()
            .chain(self.armies.keys())
            .collect::<BTreeSet<_>>()
        {
            if old_armies.get(&id) != self.armies.get(&id) {
                changes.push(StateChange::ArmyChanged {
                    army: id,
                    state: self.armies.get(&id).cloned(),
                });
            }
        }
        for &id in old_wars
            .keys()
            .chain(self.wars.keys())
            .collect::<BTreeSet<_>>()
        {
            if old_wars.get(&id) != self.wars.get(&id) {
                changes.push(StateChange::WarChanged {
                    war: id,
                    state: self.wars.get(&id).cloned(),
                });
            }
        }
        for &province in old_sieges
            .keys()
            .chain(self.sieges.keys())
            .collect::<BTreeSet<_>>()
        {
            if old_sieges.get(&province) != self.sieges.get(&province) {
                changes.push(StateChange::SiegeChanged {
                    province,
                    state: self.sieges.get(&province).copied(),
                });
            }
        }

        let failures = failures
            .into_iter()
            .map(|country| MilitaryTickFailure {
                country,
                error: WarError::ArithmeticOverflow,
            })
            .collect();
        (changes, failures)
    }

    fn move_armies(&mut self) {
        for army in self.armies.values_mut() {
            let Some(movement) = &mut army.movement else {
                continue;
            };
            let leg = movement.route[0];
            movement.elapsed_ticks += 1;
            if movement.elapsed_ticks == leg.cost {
                army.province = leg.to;
                movement.route.remove(0);
                movement.elapsed_ticks = 0;
                if movement.route.is_empty() {
                    army.movement = None;
                }
            }
        }
    }

    fn apply_supply(&mut self, graph: &ProvinceGraph) {
        let mut remaining = BTreeMap::new();
        self.armies.retain(|_, army| {
            let capacity = remaining
                .entry((army.country, army.province))
                .or_insert_with(|| {
                    let development = graph
                        .get(army.province)
                        .expect("army province validated")
                        .development;
                    u64::from(self.rules.base_supply)
                        + u64::from(development) * u64::from(self.rules.supply_per_development)
                });
            let supplied = (*capacity).min(u64::from(army.soldiers)) as u32;
            *capacity -= u64::from(supplied);
            army.soldiers -= losses(army.soldiers - supplied, self.rules.attrition_rate);
            army.soldiers > 0
        });
    }

    fn resolve_battles(
        &mut self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        bonuses: &BTreeMap<CountryId, MilitaryBonuses>,
        changes: &mut Vec<StateChange>,
        failures: &mut BTreeSet<CountryId>,
    ) {
        let wars: Vec<_> = self
            .wars
            .iter()
            .map(|(&id, war)| (id, war.attacker, war.defender))
            .collect();
        for (war_id, attacker, defender) in wars {
            let mut locations: BTreeMap<ProvinceId, (Vec<ArmyId>, Vec<ArmyId>)> = BTreeMap::new();
            for (&id, army) in &self.armies {
                if army.country == attacker {
                    locations.entry(army.province).or_default().0.push(id);
                } else if army.country == defender {
                    locations.entry(army.province).or_default().1.push(id);
                }
            }
            for (province, (attackers, defenders)) in locations {
                if attackers.is_empty() || defenders.is_empty() {
                    continue;
                }
                let attack = self.strength(&attackers, bonuses[&attacker].combat);
                let defense = self.strength(&defenders, bonuses[&defender].combat);
                if attack.is_none() {
                    failures.insert(attacker);
                }
                if defense.is_none() {
                    failures.insert(defender);
                }
                let (Some(attack), Some(defense)) = (attack, defense) else {
                    continue;
                };
                // The defending war party wins ties.
                let (winner, won, loser, lost) = if attack > defense {
                    (attacker, attackers, defender, defenders)
                } else {
                    (defender, defenders, attacker, attackers)
                };
                let winner_casualties =
                    self.apply_battle_losses(&won, self.rules.winner_casualty_rate, false);
                let mut loser_casualties =
                    self.apply_battle_losses(&lost, self.rules.loser_casualty_rate, true);
                for id in lost {
                    let Some(army) = self.armies.get(&id) else {
                        continue;
                    };
                    let retreat = graph
                        .neighbors(army.province)
                        .into_iter()
                        .flatten()
                        .find(|edge| self.can_retreat(polity, graph, loser, army.province, edge.to))
                        .map(|edge| edge.to);
                    if let Some(destination) = retreat {
                        let army = self.armies.get_mut(&id).expect("losing army exists");
                        army.province = destination;
                        army.movement = None;
                    } else if let Some(disbanded) = self.armies.remove(&id) {
                        loser_casualties += u64::from(disbanded.soldiers);
                    }
                }
                let war = self
                    .wars
                    .get_mut(&war_id)
                    .expect("active battle war exists");
                add_score(war, winner, self.rules.battle_score);
                changes.push(StateChange::BattleResolved {
                    war: war_id,
                    province,
                    winner,
                    loser,
                    winner_casualties,
                    loser_casualties,
                });
            }
        }
    }

    fn strength(&self, ids: &[ArmyId], factor: Option<Fixed>) -> Option<u128> {
        let factor = factor?;
        ids.iter().try_fold(0_u128, |total, id| {
            let army = &self.armies[id];
            let morale = army.morale.checked_mul(factor)?.raw();
            total.checked_add(u128::from(army.soldiers) * u128::try_from(morale).ok()?)
        })
    }

    fn apply_battle_losses(&mut self, ids: &[ArmyId], rate: Fixed, losing: bool) -> u64 {
        let mut casualties = 0;
        for id in ids {
            let army = self.armies.get_mut(id).expect("battle army exists");
            let dead = losses(army.soldiers, rate);
            casualties += u64::from(dead);
            army.soldiers -= dead;
            if losing {
                army.morale = Fixed::from_raw(
                    army.morale
                        .raw()
                        .saturating_sub(self.rules.battle_morale_loss.raw())
                        .max(0),
                );
            }
            army.movement = None;
            if army.soldiers == 0 {
                self.armies.remove(id);
            }
        }
        casualties
    }

    fn advance_sieges(&mut self, polity: &PolityState) {
        let mut locations: BTreeMap<ProvinceId, BTreeSet<CountryId>> = BTreeMap::new();
        for army in self.armies.values() {
            locations
                .entry(army.province)
                .or_default()
                .insert(army.country);
        }

        let mut ongoing = BTreeMap::new();
        for (&province, countries) in &locations {
            if countries.len() != 1 {
                continue;
            }
            let country = *countries.first().expect("one army country");
            if !self.armies.values().any(|army| {
                army.province == province && army.country == country && army.movement.is_none()
            }) {
                continue;
            }
            let Some((war_id, war)) = self.war_for(country) else {
                continue;
            };
            let territorial_owner = owner(polity, province);
            let controller = war
                .occupations
                .get(&province)
                .copied()
                .or(territorial_owner);
            if controller != war.opponent(country) {
                continue;
            }
            let previous = self
                .sieges
                .get(&province)
                .filter(|siege| siege.war == war_id && siege.besieger == country);
            let progress_days = previous.map_or(1, |siege| siege.progress_days.saturating_add(1));
            if progress_days >= self.siege_required(province) {
                let war = self.wars.get_mut(&war_id).expect("siege war exists");
                if territorial_owner == Some(country) {
                    war.occupations.remove(&province);
                } else {
                    war.occupations.insert(province, country);
                }
                add_score(war, country, self.rules.occupation_score);
            } else {
                ongoing.insert(
                    province,
                    Siege {
                        war: war_id,
                        besieger: country,
                        progress_days,
                    },
                );
            }
        }
        self.sieges = ongoing;
    }
}

fn losses(soldiers: u32, rate: Fixed) -> u32 {
    // Rule validation caps casualty rates at 100%.
    (u64::from(soldiers) * rate.raw() as u64).div_ceil(Fixed::SCALE as u64) as u32
}

fn add_score(war: &mut super::War, winner: CountryId, amount: i32) {
    war.score = if winner == war.attacker {
        war.score.saturating_add(amount)
    } else {
        war.score.saturating_sub(amount)
    };
}
