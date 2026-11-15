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
        for army in self.armies.values_mut() {
            army.recruitment_days_remaining = army.recruitment_days_remaining.saturating_sub(1);
        }

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
            if army.recruitment_days_remaining > 0 {
                continue;
            }
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
            if army.recruitment_days_remaining > 0 {
                return true;
            }
            let capacity = remaining
                .entry((army.country, army.faction_override, army.province))
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
        let wars: Vec<_> = self.wars.keys().copied().collect();
        for war_id in wars {
            let war = self.wars.get(&war_id).expect("active war exists").clone();
            let mut sides_by_location =
                BTreeMap::<ProvinceId, BTreeMap<super::WarSide, Vec<ArmyId>>>::new();
            for (&id, army) in &self.armies {
                if army.recruitment_days_remaining > 0 {
                    continue;
                }
                if let Some(side) = war.side_for_army(army) {
                    sides_by_location
                        .entry(army.province)
                        .or_default()
                        .entry(side)
                        .or_default()
                        .push(id);
                }
            }
            for (province, sides) in sides_by_location {
                if sides.len() < 2 {
                    continue;
                }
                if war.factions.is_empty() {
                    let mut iterator = sides.into_iter();
                    let (first_side, first_ids) = iterator.next().expect("two sides checked");
                    let (second_side, second_ids) = iterator.next().expect("two sides checked");
                    self.resolve_binary_battle(
                        graph,
                        polity,
                        bonuses,
                        war_id,
                        province,
                        first_side,
                        first_ids,
                        second_side,
                        second_ids,
                        changes,
                        failures,
                    );
                } else {
                    self.resolve_multiside_battle(
                        graph, polity, bonuses, war_id, province, sides, changes, failures,
                    );
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "combat inputs mirror the deterministic tick state"
    )]
    fn resolve_binary_battle(
        &mut self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        bonuses: &BTreeMap<CountryId, MilitaryBonuses>,
        war_id: crate::WarId,
        province: ProvinceId,
        first_side: super::WarSide,
        first_ids: Vec<ArmyId>,
        second_side: super::WarSide,
        second_ids: Vec<ArmyId>,
        changes: &mut Vec<StateChange>,
        failures: &mut BTreeSet<CountryId>,
    ) {
        let first_strength = self.strength(&first_ids, bonuses);
        let second_strength = self.strength(&second_ids, bonuses);
        if first_strength.is_none() {
            failures.extend(first_ids.iter().map(|id| self.armies[id].country));
        }
        if second_strength.is_none() {
            failures.extend(second_ids.iter().map(|id| self.armies[id].country));
        }
        let (Some(first_strength), Some(second_strength)) = (first_strength, second_strength)
        else {
            return;
        };
        // Preserve the established binary rule: defenders win ties.
        let (winner_side, winners, losers) = if first_strength > second_strength {
            (first_side, first_ids, second_ids)
        } else {
            (second_side, second_ids, first_ids)
        };
        let winner = self.wars[&war_id].side_leader(winner_side);
        let loser = self.wars[&war_id].side_leader(if winner_side == first_side {
            second_side
        } else {
            first_side
        });
        let winner_casualties =
            self.apply_battle_losses(&winners, self.rules.winner_casualty_rate, false);
        let loser_casualties =
            self.apply_battle_losses(&losers, self.rules.loser_casualty_rate, true);
        let mut loser_casualties = loser_casualties;
        for id in losers {
            self.retreat_or_disband(graph, polity, id, winner_side, &mut loser_casualties);
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

    #[expect(
        clippy::too_many_arguments,
        reason = "multi-side combat keeps the resolved state explicit"
    )]
    fn resolve_multiside_battle(
        &mut self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        bonuses: &BTreeMap<CountryId, MilitaryBonuses>,
        war_id: crate::WarId,
        province: ProvinceId,
        sides: BTreeMap<super::WarSide, Vec<ArmyId>>,
        changes: &mut Vec<StateChange>,
        failures: &mut BTreeSet<CountryId>,
    ) {
        let war = self
            .wars
            .get(&war_id)
            .expect("active battle war exists")
            .clone();
        let strengths: BTreeMap<_, _> = sides
            .iter()
            .map(|(&side, armies)| (side, self.strength(armies, bonuses)))
            .collect();
        if strengths.values().any(Option::is_none) {
            failures.extend(sides.values().flatten().map(|id| self.armies[id].country));
            return;
        }
        let strengths: BTreeMap<_, _> = strengths
            .into_iter()
            .map(|(side, strength)| (side, strength.expect("checked strength")))
            .collect();
        let winner_side = war
            .strongest_side(&strengths)
            .expect("at least two factions are present");
        let winner = war.side_leader(winner_side);
        let winner_ids = sides.get(&winner_side).expect("winning side is present");
        let winner_casualties =
            self.apply_battle_losses(winner_ids, self.rules.winner_casualty_rate, false);
        let mut losers = Vec::new();
        for (&side, army_ids) in &sides {
            if side == winner_side {
                continue;
            }
            let leader = war.side_leader(side);
            let mut casualties =
                self.apply_battle_losses(army_ids, self.rules.loser_casualty_rate, true);
            for &id in army_ids {
                self.retreat_or_disband(graph, polity, id, winner_side, &mut casualties);
            }
            losers.push((leader, casualties));
        }
        changes.push(StateChange::MultiSideBattleResolved {
            war: war_id,
            province,
            winner,
            winner_casualties,
            losers,
        });
    }

    fn retreat_or_disband(
        &mut self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        id: ArmyId,
        winner_side: super::WarSide,
        casualties: &mut u64,
    ) {
        let Some(army) = self.armies.get(&id) else {
            return;
        };
        let retreat = graph
            .neighbors(army.province)
            .into_iter()
            .flatten()
            .find(|edge| self.can_retreat_as(polity, graph, army, winner_side, edge.to))
            .map(|edge| edge.to);
        if let Some(destination) = retreat {
            let army = self.armies.get_mut(&id).expect("losing army exists");
            army.province = destination;
            army.movement = None;
        } else if let Some(disbanded) = self.armies.remove(&id) {
            *casualties += u64::from(disbanded.soldiers);
        }
    }

    fn strength(
        &self,
        ids: &[ArmyId],
        bonuses: &BTreeMap<CountryId, MilitaryBonuses>,
    ) -> Option<u128> {
        ids.iter().try_fold(0_u128, |total, id| {
            let army = &self.armies[id];
            let factor = bonuses.get(&army.country)?.combat?;
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
        let locations: BTreeSet<_> = self.armies.values().map(|army| army.province).collect();
        let mut ongoing = BTreeMap::new();
        for province in locations {
            let Some((war_id, side, country)) = self.wars.iter().find_map(|(&war_id, war)| {
                let stationary: Vec<_> = self
                    .armies
                    .values()
                    .filter(|army| {
                        army.province == province
                            && army.movement.is_none()
                            && army.recruitment_days_remaining == 0
                    })
                    .filter_map(|army| {
                        war.side_for_army(army)
                            .map(|side| (side, army.country, army))
                    })
                    .collect();
                let sides: BTreeSet<_> = stationary.iter().map(|(side, _, _)| *side).collect();
                if sides.len() > 1 {
                    return None;
                }
                let side = *sides.iter().next()?;
                let army = stationary.first()?.2;
                let controller = war
                    .occupations
                    .get(&province)
                    .copied()
                    .or_else(|| owner(polity, province));
                let hostile = controller
                    .and_then(|holder| war.side(holder))
                    .is_some_and(|holder_side| holder_side != side);
                hostile.then_some((war_id, side, army.country))
            }) else {
                continue;
            };
            let occupation_owner = self.wars[&war_id].side_leader(side);
            let territorial_owner = owner(polity, province);
            let has_factions = !self.wars[&war_id].factions.is_empty();
            let previous = self
                .sieges
                .get(&province)
                .filter(|siege| siege.war == war_id && siege.besieger == country);
            let progress_days = previous.map_or(1, |siege| siege.progress_days.saturating_add(1));
            if progress_days >= self.siege_required(province) {
                let war = self.wars.get_mut(&war_id).expect("siege war exists");
                if territorial_owner.is_some_and(|owner| war.side(owner) == Some(side)) {
                    war.occupations.remove(&province);
                } else {
                    war.occupations.insert(province, occupation_owner);
                }
                if !has_factions {
                    add_score(war, occupation_owner, self.rules.occupation_score);
                }
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
    war.score = if war.side(winner) == Some(super::WarSide::Attacker) {
        war.score.saturating_add(amount)
    } else {
        war.score.saturating_sub(amount)
    };
}

#[cfg(test)]
mod multiside_tests {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use crate::{
        CountryId, MapIdentity, ModKey, PolityRules, ProvinceGraph, StateChange, Tick, WarId,
    };

    use super::{MilitaryBonuses, WarState};
    use crate::{Army, Fixed, PolityState, ProvinceId, War, WarFaction, WarRules, WarSide};

    #[test]
    fn strongest_of_three_factions_wins_and_every_other_side_is_reported() {
        let country = CountryId::new(1);
        let first_claimant = CountryId::new(u32::MAX - 1);
        let second_claimant = CountryId::new(u32::MAX);
        let war_id = WarId::new(1);
        let rules = WarRules {
            raise_cost_per_soldier: Fixed::ZERO,
            max_army_size: 1_000,
            max_morale: Fixed::ONE,
            morale_recovery_per_tick: Fixed::ZERO,
            retreat_morale_threshold: Fixed::ZERO,
            winner_casualty_rate: Fixed::from_raw(100),
            loser_casualty_rate: Fixed::from_raw(200),
            battle_morale_loss: Fixed::ZERO,
            base_supply: 10_000,
            supply_per_development: 0,
            attrition_rate: Fixed::ZERO,
            siege_days_per_level: 1,
            battle_score: 10,
            occupation_score: 10,
            peace_score: 10,
            truce_ticks: 1,
            combat_modifier: ModKey::new("army_combat"),
            morale_modifier: ModKey::new("army_morale"),
        };
        let mut military = WarState::new(rules, BTreeMap::new());
        military.wars.insert(
            war_id,
            War {
                attacker: country,
                defender: country,
                succession_country: Some(country),
                attacker_supporters: Default::default(),
                defender_supporters: Default::default(),
                factions: BTreeMap::from([
                    (
                        first_claimant,
                        WarFaction {
                            members: Default::default(),
                            name: Some("First claimant".to_owned()),
                            dynasty: Some("House A".to_owned()),
                            claimant: None,
                            country_backed: false,
                        },
                    ),
                    (
                        second_claimant,
                        WarFaction {
                            members: Default::default(),
                            name: Some("Second claimant".to_owned()),
                            dynasty: Some("House B".to_owned()),
                            claimant: None,
                            country_backed: false,
                        },
                    ),
                ]),
                goal: ProvinceId::new(1),
                independence: false,
                started_at: Tick::new(1),
                score: 0,
                occupations: BTreeMap::new(),
                offer: None,
            },
        );
        for (id, soldiers, faction_override) in [
            (1, 100, None),
            (2, 200, Some(first_claimant)),
            (3, 300, Some(second_claimant)),
        ] {
            military.armies.insert(
                crate::ArmyId::new(id),
                Army {
                    country,
                    faction_override,
                    province: ProvinceId::new(1),
                    soldiers,
                    morale: Fixed::ONE,
                    recruitment_days_remaining: 0,
                    movement: None,
                },
            );
        }
        let polity = PolityState {
            rules: PolityRules {
                adulthood_years: 18,
                male_names: Vec::new(),
                female_names: Vec::new(),
                base_taxable_development_per_province: 0,
                tax_per_development: Fixed::ZERO,
                max_tax_rate: Fixed::ONE,
                max_monthly_spending: Fixed::ZERO,
                debt_limit: Fixed::ZERO,
                income_modifier: ModKey::new("income"),
            },
            countries: BTreeMap::new(),
            characters: BTreeMap::new(),
            ai_profiles: BTreeMap::new(),
        };
        let graph = ProvinceGraph::try_new(
            MapIdentity {
                version: NonZeroU32::new(1).expect("nonzero map version"),
                sha256: [0; 32],
            },
            [],
        )
        .expect("empty graph is sufficient; all retreats disband");
        let bonuses = BTreeMap::from([(
            country,
            MilitaryBonuses {
                morale: Some(Fixed::ONE),
                combat: Some(Fixed::ONE),
            },
        )]);
        let mut changes = Vec::<StateChange>::new();
        let mut failures = Default::default();
        military.resolve_multiside_battle(
            &graph,
            &polity,
            &bonuses,
            war_id,
            ProvinceId::new(1),
            BTreeMap::from([
                (WarSide::Attacker, vec![crate::ArmyId::new(1)]),
                (
                    WarSide::Faction(first_claimant),
                    vec![crate::ArmyId::new(2)],
                ),
                (
                    WarSide::Faction(second_claimant),
                    vec![crate::ArmyId::new(3)],
                ),
            ]),
            &mut changes,
            &mut failures,
        );
        assert!(failures.is_empty());
        assert!(matches!(
            &changes[..],
            [StateChange::MultiSideBattleResolved {
                winner,
                winner_casualties: 30,
                losers,
                ..
            }] if *winner == second_claimant
                && losers == &vec![(country, 100), (first_claimant, 200)]
        ));
        assert_eq!(military.armies[&crate::ArmyId::new(3)].soldiers, 270);
    }
}
