use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ArmyId, CountryId, EdgeKind, Fixed, ModKey, PolityState, ProvinceGraph, ProvinceId,
    ProvinceKind, Tick, WarId,
};

mod commands;
mod path;
mod tick;

pub(crate) use commands::{MilitaryAction, MilitaryOrder};
pub(crate) use tick::MilitaryBonuses;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarRules {
    pub raise_cost_per_soldier: Fixed,
    pub max_army_size: u32,
    pub max_morale: Fixed,
    pub morale_recovery_per_tick: Fixed,
    pub retreat_morale_threshold: Fixed,
    pub winner_casualty_rate: Fixed,
    pub loser_casualty_rate: Fixed,
    pub battle_morale_loss: Fixed,
    pub base_supply: u32,
    pub supply_per_development: u32,
    pub attrition_rate: Fixed,
    pub siege_days_per_level: u32,
    pub battle_score: i32,
    pub occupation_score: i32,
    pub peace_score: i32,
    pub truce_ticks: u32,
    pub combat_modifier: ModKey,
    pub morale_modifier: ModKey,
}

impl WarRules {
    fn valid(&self) -> bool {
        self.raise_cost_per_soldier >= Fixed::ZERO
            && self.max_army_size > 0
            && self.max_morale > Fixed::ZERO
            && self.morale_recovery_per_tick >= Fixed::ZERO
            && (Fixed::ZERO..=self.max_morale).contains(&self.retreat_morale_threshold)
            && self.battle_morale_loss >= Fixed::ZERO
            && self.siege_days_per_level > 0
            && self.battle_score >= 0
            && self.occupation_score >= 0
            && self.peace_score > 0
            && [
                self.winner_casualty_rate,
                self.loser_casualty_rate,
                self.attrition_rate,
            ]
            .into_iter()
            .all(|value| (Fixed::ZERO..=Fixed::ONE).contains(&value))
            && crate::province_graph::is_content_key(self.combat_modifier.as_str())
            && crate::province_graph::is_content_key(self.morale_modifier.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Army {
    pub country: CountryId,
    pub province: ProvinceId,
    pub soldiers: u32,
    pub morale: Fixed,
    pub movement: Option<ArmyMovement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmyMovement {
    /// Remaining legs, starting with the leg currently in progress.
    pub route: Vec<MovementLeg>,
    pub elapsed_ticks: u32,
}

impl ArmyMovement {
    pub fn progress(&self) -> Fixed {
        self.route.first().map_or(Fixed::ZERO, |leg| {
            if leg.cost == 0 {
                return Fixed::ZERO;
            }
            let elapsed = i64::from(self.elapsed_ticks.min(leg.cost));
            Fixed::from_raw(elapsed * Fixed::SCALE / i64::from(leg.cost))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementLeg {
    pub to: ProvinceId,
    pub kind: EdgeKind,
    pub cost: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct War {
    pub attacker: CountryId,
    pub defender: CountryId,
    /// Countries fighting with the attacker, excluding the war leader.
    pub attacker_supporters: BTreeSet<CountryId>,
    /// Countries fighting with the defender, excluding the war leader.
    pub defender_supporters: BTreeSet<CountryId>,
    pub goal: ProvinceId,
    pub started_at: Tick,
    /// Positive scores favor the attacker.
    pub score: i32,
    pub occupations: BTreeMap<ProvinceId, CountryId>,
    pub offer: Option<PeaceOffer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarSide {
    Attacker,
    Defender,
}

impl War {
    pub fn side(&self, country: CountryId) -> Option<WarSide> {
        if country == self.attacker || self.attacker_supporters.contains(&country) {
            Some(WarSide::Attacker)
        } else if country == self.defender || self.defender_supporters.contains(&country) {
            Some(WarSide::Defender)
        } else {
            None
        }
    }

    pub fn opposed(&self, a: CountryId, b: CountryId) -> bool {
        matches!((self.side(a), self.side(b)), (Some(x), Some(y)) if x != y)
    }

    pub fn is_leader(&self, country: CountryId) -> bool {
        country == self.attacker || country == self.defender
    }

    pub fn opponent(&self, country: CountryId) -> Option<CountryId> {
        match self.side(country)? {
            WarSide::Attacker => Some(self.defender),
            WarSide::Defender => Some(self.attacker),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeaceOffer {
    pub offered_by: CountryId,
    pub cede_goal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeaceTreaty {
    pub attacker: CountryId,
    pub defender: CountryId,
    pub ceded: Option<ProvinceId>,
    pub concluded_at: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Siege {
    pub war: WarId,
    pub besieger: CountryId,
    pub progress_days: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarState {
    pub rules: WarRules,
    pub armies: BTreeMap<ArmyId, Army>,
    pub wars: BTreeMap<WarId, War>,
    pub forts: BTreeMap<ProvinceId, u8>,
    pub sieges: BTreeMap<ProvinceId, Siege>,
    pub treaties: BTreeMap<WarId, PeaceTreaty>,
    /// Extra opposing pairs from coalition peace; original leaders remain in `treaties`.
    pub coalition_truces: BTreeMap<(CountryId, CountryId), Tick>,
    pub next_army_id: u32,
    pub next_war_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarError {
    Unavailable,
    AlreadyConfigured,
    InvalidRules,
    InvalidArmy(ArmyId),
    InvalidWar(WarId),
    InvalidFort(ProvinceId),
    InvalidSiege(ProvinceId),
    InvalidTreaty(WarId),
    InvalidCoalitionTruce(CountryId, CountryId),
    InvalidIdSequence,
    UnknownCountry(CountryId),
    UnknownArmy(ArmyId),
    UnknownWar(WarId),
    NotArmyOwner(ArmyId),
    ArmyInMotion(ArmyId),
    ArmiesNotCoLocated,
    InvalidArmySize,
    InvalidProvince(ProvinceId),
    InsufficientTreasury,
    Unreachable(ProvinceId),
    InvalidRetreat(ProvinceId),
    AlreadyAtWar,
    AllianceActive,
    InvalidWarGoal(ProvinceId),
    TruceActive,
    NotWarParty,
    NotWarLeader,
    NoPeaceOffer,
    CannotAcceptOwnOffer,
    InvalidPeaceTerms,
    IdExhausted,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MilitaryTickFailure {
    pub country: CountryId,
    pub error: WarError,
}

impl WarState {
    pub fn new(rules: WarRules, forts: BTreeMap<ProvinceId, u8>) -> Self {
        Self {
            rules,
            armies: BTreeMap::new(),
            wars: BTreeMap::new(),
            forts,
            sieges: BTreeMap::new(),
            treaties: BTreeMap::new(),
            coalition_truces: BTreeMap::new(),
            next_army_id: 1,
            next_war_id: 1,
        }
    }

    pub fn war_for(&self, country: CountryId) -> Option<(WarId, &War)> {
        self.wars
            .iter()
            .find(|(_, war)| war.opponent(country).is_some())
            .map(|(&id, war)| (id, war))
    }

    pub fn has_coalition_history(&self) -> bool {
        !self.coalition_truces.is_empty()
            || self.wars.values().any(|war| {
                !war.attacker_supporters.is_empty() || !war.defender_supporters.is_empty()
            })
    }

    pub fn occupation(&self, province: ProvinceId) -> Option<CountryId> {
        self.wars
            .values()
            .find_map(|war| war.occupations.get(&province).copied())
    }

    pub fn siege_days(&self, province: ProvinceId) -> u32 {
        self.sieges
            .get(&province)
            .map_or(0, |siege| siege.progress_days)
    }

    fn can_enter(&self, polity: &PolityState, country: CountryId, province: ProvinceId) -> bool {
        owner(polity, province).is_some_and(|owner| {
            owner == country
                || self
                    .war_for(country)
                    .is_some_and(|(_, war)| war.side(owner).is_some())
        })
    }

    fn can_retreat(
        &self,
        polity: &PolityState,
        graph: &ProvinceGraph,
        country: CountryId,
        from: ProvinceId,
        to: ProvinceId,
    ) -> bool {
        from != to
            && owner(polity, to).is_some_and(|owner| {
                owner == country
                    || self
                        .war_for(country)
                        .is_some_and(|(_, war)| war.side(owner) == war.side(country))
            })
            && self.occupation(to).is_none_or(|occupier| {
                occupier == country
                    || self
                        .war_for(country)
                        .is_some_and(|(_, war)| war.side(occupier) == war.side(country))
            })
            && graph.neighbors(from).is_some_and(|edges| {
                edges
                    .iter()
                    .any(|edge| edge.to == to && edge.kind != EdgeKind::Port)
            })
            && self.war_for(country).is_none_or(|(_, war)| {
                self.armies
                    .values()
                    .all(|army| army.province != to || !war.opposed(country, army.country))
            })
    }

    fn valid_cession(&self, polity: &PolityState, war: &War) -> bool {
        war.score >= self.rules.peace_score
            && war
                .occupations
                .get(&war.goal)
                .is_some_and(|occupier| war.side(*occupier) == Some(WarSide::Attacker))
            && polity.countries.get(&war.defender).is_some_and(|defender| {
                defender.provinces.len() > 1 && defender.provinces.contains(&war.goal)
            })
    }

    pub(crate) fn validate(
        &self,
        polity: &PolityState,
        graph: &ProvinceGraph,
        tick: Tick,
    ) -> Result<(), WarError> {
        if !self.rules.valid() {
            return Err(WarError::InvalidRules);
        }
        if self.next_army_id == 0
            || self.next_war_id == 0
            || self
                .armies
                .keys()
                .any(|id| id.get() == 0 || id.get() >= self.next_army_id)
            || self
                .wars
                .keys()
                .chain(self.treaties.keys())
                .any(|id| id.get() == 0 || id.get() >= self.next_war_id)
        {
            return Err(WarError::InvalidIdSequence);
        }
        let mut parties = BTreeSet::new();
        for (&id, war) in &self.wars {
            if self.treaties.contains_key(&id)
                || !polity.countries.contains_key(&war.attacker)
                || !polity.countries.contains_key(&war.defender)
                || !parties.insert(war.attacker)
                || !parties.insert(war.defender)
                || war
                    .attacker_supporters
                    .iter()
                    .chain(&war.defender_supporters)
                    .any(|&country| {
                        !polity.countries.contains_key(&country) || !parties.insert(country)
                    })
                || war.started_at > tick
                || !self.can_reach_goal(graph, polity, war.attacker, war.defender, war.goal)
                || war
                    .offer
                    .is_some_and(|offer| !war.is_leader(offer.offered_by))
                || war.occupations.iter().any(|(&province, &occupier)| {
                    owner(polity, province)
                        .is_none_or(|territorial_owner| !war.opposed(occupier, territorial_owner))
                })
            {
                return Err(WarError::InvalidWar(id));
            }
        }
        for (&id, treaty) in &self.treaties {
            if treaty.attacker == treaty.defender
                || !polity.countries.contains_key(&treaty.attacker)
                || !polity.countries.contains_key(&treaty.defender)
                || treaty.concluded_at > tick
                || treaty.ceded.is_some_and(|province| {
                    !graph
                        .get(province)
                        .is_some_and(|province| province.kind == ProvinceKind::Land)
                })
            {
                return Err(WarError::InvalidTreaty(id));
            }
        }
        for (&(a, b), &concluded_at) in &self.coalition_truces {
            if a >= b
                || !polity.countries.contains_key(&a)
                || !polity.countries.contains_key(&b)
                || concluded_at > tick
            {
                return Err(WarError::InvalidCoalitionTruce(a, b));
            }
        }
        for (&id, army) in &self.armies {
            if !polity.countries.contains_key(&army.country)
                || !self.can_enter(polity, army.country, army.province)
                || army.soldiers == 0
                || army.soldiers > self.rules.max_army_size
                || army.morale < Fixed::ZERO
            {
                return Err(WarError::InvalidArmy(id));
            }
            if let Some(movement) = &army.movement {
                if movement
                    .route
                    .first()
                    .is_none_or(|leg| movement.elapsed_ticks >= leg.cost)
                {
                    return Err(WarError::InvalidArmy(id));
                }
                let mut from = army.province;
                for leg in &movement.route {
                    let edge_exists = graph.neighbors(from).is_some_and(|edges| {
                        edges.iter().any(|edge| {
                            edge.to == leg.to && edge.kind == leg.kind && edge.cost == leg.cost
                        })
                    });
                    if leg.kind == EdgeKind::Port
                        || !edge_exists
                        || !self.can_enter(polity, army.country, leg.to)
                    {
                        return Err(WarError::InvalidArmy(id));
                    }
                    from = leg.to;
                }
            }
        }
        for (&province, &level) in &self.forts {
            if level == 0
                || self
                    .rules
                    .siege_days_per_level
                    .checked_mul(u32::from(level))
                    .is_none()
                || !graph
                    .get(province)
                    .is_some_and(|province| province.kind == ProvinceKind::Land)
            {
                return Err(WarError::InvalidFort(province));
            }
        }
        for (&province, siege) in &self.sieges {
            let valid = self.wars.get(&siege.war).is_some_and(|war| {
                let controller = war
                    .occupations
                    .get(&province)
                    .copied()
                    .or_else(|| owner(polity, province));
                controller.is_some_and(|holder| war.opposed(siege.besieger, holder))
                    && self.armies.values().any(|army| {
                        army.country == siege.besieger
                            && army.province == province
                            && army.movement.is_none()
                    })
            });
            if !valid
                || siege.progress_days == 0
                || siege.progress_days >= self.siege_required(province)
            {
                return Err(WarError::InvalidSiege(province));
            }
        }
        Ok(())
    }

    /// Days needed to complete an uninterrupted siege at this province.
    pub fn siege_required(&self, province: ProvinceId) -> u32 {
        self.rules
            .siege_days_per_level
            .saturating_mul(u32::from(self.forts.get(&province).copied().unwrap_or(1)))
    }
}

fn owner(polity: &PolityState, province: ProvinceId) -> Option<CountryId> {
    polity
        .countries
        .iter()
        .find_map(|(&id, country)| country.provinces.contains(&province).then_some(id))
}
