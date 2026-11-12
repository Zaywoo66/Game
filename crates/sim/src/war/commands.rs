use crate::{
    ArmyId, CountryId, Fixed, PolityState, ProvinceGraph, ProvinceId, StateChange, Tick, WarId,
};

use super::{Army, ArmyMovement, PeaceOffer, PeaceTreaty, War, WarError, WarState};

#[derive(Clone, Copy)]
pub(crate) enum MilitaryOrder {
    Raise(ProvinceId, u32),
    Merge(ArmyId, ArmyId),
    Split(ArmyId, u32),
    Move(ArmyId, ProvinceId),
    Retreat(ArmyId, ProvinceId),
    Declare(CountryId, ProvinceId),
    DeclareIndependence(CountryId, ProvinceId),
    Offer(WarId, bool),
    Accept(WarId),
}

pub(crate) enum MilitaryAction {
    Raise(ArmyId, Army, Fixed),
    Update(ArmyId, Army),
    Merge(ArmyId, Army, ArmyId),
    Split(ArmyId, Army, ArmyId, Army),
    Declare(WarId, War),
    Offer(WarId, PeaceOffer),
    Accept(WarId, PeaceTreaty),
}

impl WarState {
    pub(crate) fn prepare(
        &self,
        graph: &ProvinceGraph,
        polity: &PolityState,
        tick: Tick,
        country: CountryId,
        order: MilitaryOrder,
        raise_morale: Option<Fixed>,
    ) -> Result<MilitaryAction, WarError> {
        let state = polity
            .countries
            .get(&country)
            .ok_or(WarError::UnknownCountry(country))?;
        match order {
            MilitaryOrder::Raise(province, soldiers) => {
                if soldiers == 0 || soldiers > self.rules.max_army_size {
                    return Err(WarError::InvalidArmySize);
                }
                if !state.provinces.contains(&province)
                    || self
                        .occupation(province)
                        .is_some_and(|occupier| occupier != country)
                    || self.war_for(country).is_some_and(|(_, war)| {
                        self.armies.values().any(|army| {
                            army.province == province && war.opposed(country, army.country)
                        })
                    })
                {
                    return Err(WarError::InvalidProvince(province));
                }
                let id = self.allocate_army_id()?;
                let cost = self
                    .rules
                    .raise_cost_per_soldier
                    .checked_mul(Fixed::from_integer(i64::from(soldiers)).expect("u32 fits Fixed"))
                    .ok_or(WarError::ArithmeticOverflow)?;
                let treasury = state
                    .economy
                    .treasury
                    .checked_sub(cost)
                    .filter(|result| *result >= Fixed::ZERO)
                    .ok_or(WarError::InsufficientTreasury)?;
                let morale = raise_morale.ok_or(WarError::ArithmeticOverflow)?;
                Ok(MilitaryAction::Raise(
                    id,
                    Army {
                        country,
                        province,
                        soldiers,
                        morale,
                        movement: None,
                    },
                    treasury,
                ))
            }
            MilitaryOrder::Merge(into, from) => {
                let mut army = self.idle_army(country, into)?.clone();
                let other = self.idle_army(country, from)?;
                if into == from || army.province != other.province {
                    return Err(WarError::ArmiesNotCoLocated);
                }
                let total = army
                    .soldiers
                    .checked_add(other.soldiers)
                    .filter(|&n| n <= self.rules.max_army_size)
                    .ok_or(WarError::InvalidArmySize)?;
                let morale = (i128::from(army.morale.raw()) * i128::from(army.soldiers)
                    + i128::from(other.morale.raw()) * i128::from(other.soldiers))
                    / i128::from(total);
                army.morale = Fixed::from_raw(
                    i64::try_from(morale).map_err(|_| WarError::ArithmeticOverflow)?,
                );
                army.soldiers = total;
                Ok(MilitaryAction::Merge(into, army, from))
            }
            MilitaryOrder::Split(id, soldiers) => {
                let mut army = self.idle_army(country, id)?.clone();
                if soldiers == 0 || soldiers >= army.soldiers {
                    return Err(WarError::InvalidArmySize);
                }
                let new_id = self.allocate_army_id()?;
                let child = Army {
                    soldiers,
                    ..army.clone()
                };
                army.soldiers -= soldiers;
                Ok(MilitaryAction::Split(id, army, new_id, child))
            }
            MilitaryOrder::Move(id, destination) => {
                let mut army = self.owned_army(country, id)?.clone();
                if army
                    .movement
                    .as_ref()
                    .and_then(|movement| movement.route.last())
                    .is_some_and(|leg| leg.to == destination)
                {
                    return Ok(MilitaryAction::Update(id, army));
                }
                let route = self
                    .find_path(graph, polity, country, army.province, destination)
                    .ok_or(WarError::Unreachable(destination))?;
                army.movement = (!route.is_empty()).then_some(ArmyMovement {
                    route,
                    elapsed_ticks: 0,
                });
                Ok(MilitaryAction::Update(id, army))
            }
            MilitaryOrder::Retreat(id, destination) => {
                let mut army = self.owned_army(country, id)?.clone();
                let war = self.war_for(country).map(|(_, war)| war);
                let contact = war.is_some_and(|war| {
                    self.armies.values().any(|other| {
                        war.opposed(country, other.country) && other.province == army.province
                    })
                });
                let outside_safe_land = !state.provinces.contains(&army.province)
                    || self.occupation(army.province).is_some_and(|occupier| {
                        war.is_none_or(|war| war.side(occupier) != war.side(country))
                    });
                let under_threat = contact
                    || (outside_safe_land && army.morale <= self.rules.retreat_morale_threshold);
                if war.is_none()
                    || !under_threat
                    || !self.can_retreat(polity, graph, country, army.province, destination)
                {
                    return Err(WarError::InvalidRetreat(destination));
                }
                army.province = destination;
                army.movement = None;
                Ok(MilitaryAction::Update(id, army))
            }
            MilitaryOrder::Declare(defender, goal)
            | MilitaryOrder::DeclareIndependence(defender, goal) => {
                let independence = matches!(order, MilitaryOrder::DeclareIndependence(..));
                if !polity.countries.contains_key(&defender) {
                    return Err(WarError::UnknownCountry(defender));
                }
                if country == defender
                    || !self.can_reach_goal(graph, polity, country, defender, goal, independence)
                {
                    return Err(WarError::InvalidWarGoal(goal));
                }
                if self.war_for(country).is_some() || self.war_for(defender).is_some() {
                    return Err(WarError::AlreadyAtWar);
                }
                let pair = if country < defender {
                    (country, defender)
                } else {
                    (defender, country)
                };
                if self.treaties.values().any(|treaty| {
                    ((treaty.attacker == country && treaty.defender == defender)
                        || (treaty.defender == country && treaty.attacker == defender))
                        && tick.get().saturating_sub(treaty.concluded_at.get())
                            < u64::from(self.rules.truce_ticks)
                }) || self
                    .coalition_truces
                    .get(&pair)
                    .is_some_and(|concluded_at| {
                        tick.get().saturating_sub(concluded_at.get())
                            < u64::from(self.rules.truce_ticks)
                    })
                {
                    return Err(WarError::TruceActive);
                }
                self.next_war_id
                    .checked_add(1)
                    .ok_or(WarError::IdExhausted)?;
                let war = War {
                    attacker: country,
                    defender,
                    attacker_supporters: Default::default(),
                    defender_supporters: Default::default(),
                    goal,
                    independence,
                    started_at: tick,
                    score: 0,
                    occupations: Default::default(),
                    offer: None,
                };
                Ok(MilitaryAction::Declare(WarId::new(self.next_war_id), war))
            }
            MilitaryOrder::Offer(id, cede_goal) => {
                let war = self.leader_war(country, id)?;
                if cede_goal && !self.valid_cession(polity, war) {
                    return Err(WarError::InvalidPeaceTerms);
                }
                Ok(MilitaryAction::Offer(
                    id,
                    PeaceOffer {
                        offered_by: country,
                        cede_goal,
                    },
                ))
            }
            MilitaryOrder::Accept(id) => {
                let war = self.leader_war(country, id)?;
                let offer = war.offer.ok_or(WarError::NoPeaceOffer)?;
                if offer.offered_by == country {
                    return Err(WarError::CannotAcceptOwnOffer);
                }
                if offer.cede_goal && !self.valid_cession(polity, war) {
                    return Err(WarError::InvalidPeaceTerms);
                }
                Ok(MilitaryAction::Accept(
                    id,
                    PeaceTreaty {
                        attacker: war.attacker,
                        defender: war.defender,
                        ceded: (offer.cede_goal && !war.independence).then_some(war.goal),
                        concluded_at: tick,
                    },
                ))
            }
        }
    }

    pub(crate) fn apply(
        &mut self,
        polity: &mut PolityState,
        action: MilitaryAction,
    ) -> Vec<StateChange> {
        let mut changes = Vec::new();
        match action {
            MilitaryAction::Raise(id, army, treasury) => {
                self.next_army_id += 1;
                let economy = &mut polity
                    .countries
                    .get_mut(&army.country)
                    .expect("country validated")
                    .economy;
                if economy.treasury != treasury {
                    economy.treasury = treasury;
                    changes.push(StateChange::EconomyChanged {
                        country: army.country,
                        economy: *economy,
                    });
                }
                self.set_army(id, army, &mut changes);
            }
            MilitaryAction::Update(id, army) => self.set_army(id, army, &mut changes),
            MilitaryAction::Merge(into, army, from) => {
                self.set_army(into, army, &mut changes);
                self.armies.remove(&from);
                changes.push(StateChange::ArmyChanged {
                    army: from,
                    state: None,
                });
            }
            MilitaryAction::Split(id, army, new_id, child) => {
                self.next_army_id += 1;
                self.set_army(id, army, &mut changes);
                self.set_army(new_id, child, &mut changes);
            }
            MilitaryAction::Declare(id, war) => {
                self.next_war_id += 1;
                self.wars.insert(id, war.clone());
                changes.push(StateChange::WarChanged {
                    war: id,
                    state: Some(war),
                });
            }
            MilitaryAction::Offer(id, offer) => {
                let war = self.wars.get_mut(&id).expect("war validated");
                if war.offer != Some(offer) {
                    war.offer = Some(offer);
                    changes.push(StateChange::WarChanged {
                        war: id,
                        state: Some(war.clone()),
                    });
                }
            }
            MilitaryAction::Accept(id, treaty) => {
                self.conclude_peace(polity, id, treaty, &mut changes)
            }
        }
        self.prune_abandoned_sieges(&mut changes);
        changes
    }

    fn conclude_peace(
        &mut self,
        polity: &mut PolityState,
        id: WarId,
        treaty: PeaceTreaty,
        changes: &mut Vec<StateChange>,
    ) {
        let war = self.wars.remove(&id).expect("war validated");
        for attacker in std::iter::once(war.attacker).chain(war.attacker_supporters.iter().copied())
        {
            for defender in
                std::iter::once(war.defender).chain(war.defender_supporters.iter().copied())
            {
                if attacker == treaty.attacker && defender == treaty.defender {
                    continue;
                }
                let pair = if attacker < defender {
                    (attacker, defender)
                } else {
                    (defender, attacker)
                };
                self.coalition_truces.insert(pair, treaty.concluded_at);
            }
        }
        changes.push(StateChange::WarChanged {
            war: id,
            state: None,
        });
        if let Some(province) = treaty.ceded {
            let defender = polity
                .countries
                .get_mut(&treaty.defender)
                .expect("war party validated");
            defender.provinces.remove(&province);
            if defender.capital == province {
                defender.capital = *defender
                    .provinces
                    .first()
                    .expect("cession preserves a province");
            }
            let attacker = polity
                .countries
                .get_mut(&treaty.attacker)
                .expect("war party validated");
            attacker.provinces.insert(province);
            changes.push(StateChange::ProvinceOwnershipChanged {
                province,
                owner: treaty.attacker,
            });
        }
        for (&army_id, army) in &mut self.armies {
            if war.side(army.country).is_none() {
                continue;
            }
            let state = &polity.countries[&army.country];
            let foreign = !state.provinces.contains(&army.province);
            if foreign || army.movement.is_some() {
                if foreign {
                    army.province = state.capital;
                }
                army.movement = None;
                changes.push(StateChange::ArmyChanged {
                    army: army_id,
                    state: Some(army.clone()),
                });
            }
        }
        self.sieges.retain(|&province, siege| {
            if siege.war == id {
                changes.push(StateChange::SiegeChanged {
                    province,
                    state: None,
                });
                false
            } else {
                true
            }
        });
        self.treaties.insert(id, treaty);
        changes.push(StateChange::PeaceConcluded {
            war: id,
            attacker: treaty.attacker,
            defender: treaty.defender,
            ceded: treaty.ceded,
        });
    }

    fn prune_abandoned_sieges(&mut self, changes: &mut Vec<StateChange>) {
        self.sieges.retain(|&province, siege| {
            let active = self.armies.values().any(|army| {
                army.province == province
                    && army.country == siege.besieger
                    && army.movement.is_none()
            });
            if !active {
                changes.push(StateChange::SiegeChanged {
                    province,
                    state: None,
                });
            }
            active
        });
    }

    fn set_army(&mut self, id: ArmyId, army: Army, changes: &mut Vec<StateChange>) {
        if self.armies.get(&id) != Some(&army) {
            self.armies.insert(id, army.clone());
            changes.push(StateChange::ArmyChanged {
                army: id,
                state: Some(army),
            });
        }
    }

    fn allocate_army_id(&self) -> Result<ArmyId, WarError> {
        self.next_army_id
            .checked_add(1)
            .ok_or(WarError::IdExhausted)?;
        Ok(ArmyId::new(self.next_army_id))
    }

    fn owned_army(&self, country: CountryId, id: ArmyId) -> Result<&Army, WarError> {
        let army = self.armies.get(&id).ok_or(WarError::UnknownArmy(id))?;
        if army.country != country {
            return Err(WarError::NotArmyOwner(id));
        }
        Ok(army)
    }

    fn idle_army(&self, country: CountryId, id: ArmyId) -> Result<&Army, WarError> {
        let army = self.owned_army(country, id)?;
        if army.movement.is_some() {
            return Err(WarError::ArmyInMotion(id));
        }
        Ok(army)
    }

    fn leader_war(&self, country: CountryId, id: WarId) -> Result<&War, WarError> {
        let war = self.wars.get(&id).ok_or(WarError::UnknownWar(id))?;
        if !war.is_leader(country) {
            if war.side(country).is_some() {
                return Err(WarError::NotWarLeader);
            }
            return Err(WarError::NotWarParty);
        }
        Ok(war)
    }
}
