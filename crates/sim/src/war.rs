use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ArmyId, CharacterId, CountryId, EdgeKind, Fixed, ModKey, PolityState, ProvinceGraph,
    ProvinceId, ProvinceKind, StateChange, Tick, WarId,
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
    /// Optional war-local side override for a rebel army. Country remains the
    /// controller and supplies all country modifiers.
    pub faction_override: Option<CountryId>,
    pub province: ProvinceId,
    pub soldiers: u32,
    pub morale: Fixed,
    /// Days until a newly recruited army is available for orders and battle.
    pub recruitment_days_remaining: u16,
    pub movement: Option<ArmyMovement>,
}

/// Newly raised armies become operational after one month of training.
pub const ARMY_RECRUITMENT_DAYS: u16 = 30;

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
    /// Set only for a succession crisis where the disputed country is the
    /// default royalist side and claimant factions fight for its throne.
    pub succession_country: Option<CountryId>,
    /// Countries fighting with the attacker, excluding the war leader.
    pub attacker_supporters: BTreeSet<CountryId>,
    /// Countries fighting with the defender, excluding the war leader.
    pub defender_supporters: BTreeSet<CountryId>,
    /// Independent country-backed factions, keyed by their war leader. Members
    /// are supporters only; the key is implicitly part of that faction.
    /// Ordinary wars leave this empty and continue to use the two coalition
    /// fields above.
    pub factions: BTreeMap<CountryId, WarFaction>,
    pub goal: ProvinceId,
    /// Subject's war against its overlord; enforcing the goal ends the bond.
    pub independence: bool,
    pub started_at: Tick,
    /// Positive scores favor the attacker.
    pub score: i32,
    pub occupations: BTreeMap<ProvinceId, CountryId>,
    pub offer: Option<PeaceOffer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarFaction {
    /// Other country-backed members; the map key is the faction leader.
    pub members: BTreeSet<CountryId>,
    /// Internal factions have a generated leader/name and dynasty.
    pub name: Option<String>,
    pub dynasty: Option<String>,
    /// Character represented by a dynastic claimant faction, if any.
    pub claimant: Option<CharacterId>,
    /// `false` means the faction ID is synthetic and has no country record.
    pub country_backed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarSide {
    Attacker,
    Defender,
    /// An independent faction led by this country.
    Faction(CountryId),
}

impl War {
    pub fn side(&self, country: CountryId) -> Option<WarSide> {
        if country == self.attacker || self.attacker_supporters.contains(&country) {
            Some(WarSide::Attacker)
        } else if country == self.defender || self.defender_supporters.contains(&country) {
            Some(WarSide::Defender)
        } else {
            self.factions.iter().find_map(|(&leader, faction)| {
                (country == leader || faction.members.contains(&country))
                    .then_some(WarSide::Faction(leader))
            })
        }
    }

    pub fn side_for_army(&self, army: &Army) -> Option<WarSide> {
        army.faction_override
            .map(WarSide::Faction)
            .or_else(|| self.side(army.country))
    }

    pub fn opposed(&self, a: CountryId, b: CountryId) -> bool {
        matches!((self.side(a), self.side(b)), (Some(x), Some(y)) if x != y)
    }

    pub fn is_leader(&self, country: CountryId) -> bool {
        country == self.attacker || country == self.defender || self.factions.contains_key(&country)
    }

    pub fn countries_on_side(&self, side: WarSide) -> BTreeSet<CountryId> {
        match side {
            WarSide::Attacker => std::iter::once(self.attacker)
                .chain(self.attacker_supporters.iter().copied())
                .collect(),
            WarSide::Defender => std::iter::once(self.defender)
                .chain(self.defender_supporters.iter().copied())
                .collect(),
            WarSide::Faction(leader) => self.factions.get(&leader).map_or_else(
                || BTreeSet::from([leader]),
                |faction| {
                    std::iter::once(leader)
                        .chain(faction.members.iter().copied())
                        .collect()
                },
            ),
        }
    }

    pub fn side_leader(&self, side: WarSide) -> CountryId {
        match side {
            WarSide::Attacker => self.attacker,
            WarSide::Defender => self.defender,
            WarSide::Faction(leader) => leader,
        }
    }

    /// Selects the highest-strength faction; equal strengths go to the lower
    /// war faction ID (the side leader's CountryId).
    pub fn strongest_side(&self, strengths: &BTreeMap<WarSide, u128>) -> Option<WarSide> {
        strengths
            .iter()
            .max_by(
                |(first_side, first_strength), (second_side, second_strength)| {
                    first_strength.cmp(second_strength).then_with(|| {
                        self.side_leader(**second_side)
                            .cmp(&self.side_leader(**first_side))
                    })
                },
            )
            .map(|(&side, _)| side)
    }

    /// Whether the war contains an independent faction beyond the two
    /// ordinary coalitions.
    pub fn has_independent_factions(&self) -> bool {
        self.succession_country.is_some() || !self.factions.is_empty()
    }

    pub fn opponent(&self, country: CountryId) -> Option<CountryId> {
        match self.side(country)? {
            WarSide::Attacker => Some(self.defender),
            WarSide::Defender => Some(self.attacker),
            WarSide::Faction(leader) => [self.attacker, self.defender]
                .into_iter()
                .find(|candidate| self.opposed(leader, *candidate)),
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
    InvalidWarFaction(CountryId),
    UnsupportedMultiSideBattle,
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
    ArmyRecruiting(ArmyId),
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
    UnsupportedMultiSidePeace,
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

    /// Adds an internal, non-country faction to an active war and returns its
    /// stable synthetic ID. IDs descend from `u32::MAX` and are checked against
    /// both scenario countries and all retained faction/truce IDs.
    pub(crate) fn create_internal_faction(
        &mut self,
        polity: &PolityState,
        war_id: WarId,
        name: String,
        dynasty: String,
    ) -> Result<CountryId, WarError> {
        if name.trim().is_empty()
            || dynasty.trim().is_empty()
            || name.chars().any(char::is_control)
            || dynasty.chars().any(char::is_control)
        {
            return Err(WarError::InvalidWarFaction(CountryId::new(u32::MAX)));
        }
        let war = self.wars.get(&war_id).ok_or(WarError::UnknownWar(war_id))?;
        if war.succession_country.is_none() {
            return Err(WarError::InvalidWarFaction(CountryId::new(u32::MAX)));
        }
        let mut candidate = u32::MAX;
        let id = loop {
            let candidate_id = CountryId::new(candidate);
            let is_used = polity.countries.contains_key(&candidate_id)
                || self
                    .wars
                    .values()
                    .any(|war| war.factions.contains_key(&candidate_id))
                || self
                    .coalition_truces
                    .keys()
                    .any(|&(a, b)| a == candidate_id || b == candidate_id);
            if !is_used {
                break candidate_id;
            }
            candidate = candidate.checked_sub(1).ok_or(WarError::IdExhausted)?;
        };
        self.wars
            .get_mut(&war_id)
            .expect("war checked above")
            .factions
            .insert(
                id,
                WarFaction {
                    members: BTreeSet::new(),
                    name: Some(name),
                    dynasty: Some(dynasty),
                    claimant: None,
                    country_backed: false,
                },
            );
        Ok(id)
    }

    /// Splits an existing country-owned army between the royalist state and
    /// an internal claimant. The original keeps the larger half; the new army
    /// retains the same country owner and receives only a war-local side
    /// override. The caller emits both ArmyChanged deltas from the snapshots.
    pub(crate) fn split_army_for_faction(
        &mut self,
        war_id: WarId,
        army_id: ArmyId,
        faction: CountryId,
    ) -> Result<(ArmyId, Army, Army), WarError> {
        let succession_country = {
            let war = self.wars.get(&war_id).ok_or(WarError::UnknownWar(war_id))?;
            if war.succession_country.is_none()
                || war
                    .factions
                    .get(&faction)
                    .is_none_or(|definition| definition.country_backed)
            {
                return Err(WarError::InvalidWarFaction(faction));
            }
            war.succession_country
        };
        let new_id = self.allocate_army_id()?;
        let original = self
            .armies
            .get(&army_id)
            .ok_or(WarError::UnknownArmy(army_id))?;
        if Some(original.country) != succession_country
            || original.faction_override.is_some()
            || original.soldiers < 2
        {
            return Err(WarError::InvalidArmy(army_id));
        }
        let rebel_soldiers = original.soldiers / 2;
        let mut royalist = original.clone();
        royalist.soldiers -= rebel_soldiers;
        royalist.movement = None;
        let claimant = Army {
            soldiers: rebel_soldiers,
            faction_override: Some(faction),
            movement: None,
            ..original.clone()
        };
        self.next_army_id += 1;
        self.armies.insert(army_id, royalist.clone());
        self.armies.insert(new_id, claimant.clone());
        Ok((new_id, royalist, claimant))
    }

    /// Starts a succession conflict with the disputed country as the default
    /// royalist faction, creates one aristocratic claimant, and splits its
    /// existing armies. One-soldier armies stay with the royalists.
    pub(crate) fn begin_succession_conflict(
        &mut self,
        polity: &PolityState,
        country: CountryId,
        goal: ProvinceId,
        tick: Tick,
        aristocrat_name: String,
        dynasty: String,
    ) -> Result<(WarId, CountryId, Vec<StateChange>), WarError> {
        let state = polity
            .countries
            .get(&country)
            .ok_or(WarError::UnknownCountry(country))?;
        if !state.provinces.contains(&goal) || self.war_for(country).is_some() {
            return Err(WarError::InvalidWarGoal(goal));
        }
        let split_armies: Vec<_> = self
            .armies
            .iter()
            .filter_map(|(&id, army)| (army.country == country && army.soldiers >= 2).then_some(id))
            .collect();
        if self
            .next_army_id
            .checked_add(u32::try_from(split_armies.len()).map_err(|_| WarError::IdExhausted)?)
            .is_none()
        {
            return Err(WarError::IdExhausted);
        }
        let next_war_id = self
            .next_war_id
            .checked_add(1)
            .ok_or(WarError::IdExhausted)?;
        let war_id = WarId::new(self.next_war_id);
        if self.wars.contains_key(&war_id) || self.treaties.contains_key(&war_id) {
            return Err(WarError::InvalidIdSequence);
        }
        let war = War {
            attacker: country,
            defender: country,
            succession_country: Some(country),
            attacker_supporters: BTreeSet::new(),
            defender_supporters: BTreeSet::new(),
            factions: BTreeMap::new(),
            goal,
            independence: false,
            started_at: tick,
            score: 0,
            occupations: BTreeMap::new(),
            offer: None,
        };
        self.next_war_id = next_war_id;
        self.wars.insert(war_id, war);
        let faction = match self.create_internal_faction(polity, war_id, aristocrat_name, dynasty) {
            Ok(faction) => faction,
            Err(error) => {
                self.wars.remove(&war_id);
                self.next_war_id = war_id.get();
                return Err(error);
            }
        };
        let previous_armies = self.armies.clone();
        let previous_next_army_id = self.next_army_id;
        let mut changes = Vec::with_capacity(1 + split_armies.len() * 2);
        for army_id in split_armies {
            match self.split_army_for_faction(war_id, army_id, faction) {
                Ok((new_id, royalist, claimant)) => {
                    changes.push(StateChange::ArmyChanged {
                        army: army_id,
                        state: Some(royalist),
                    });
                    changes.push(StateChange::ArmyChanged {
                        army: new_id,
                        state: Some(claimant),
                    });
                }
                Err(error) => {
                    self.wars.remove(&war_id);
                    self.next_war_id = war_id.get();
                    self.next_army_id = previous_next_army_id;
                    self.armies = previous_armies;
                    return Err(error);
                }
            }
        }
        changes.push(StateChange::WarChanged {
            war: war_id,
            state: self.wars.get(&war_id).cloned(),
        });
        Ok((war_id, faction, changes))
    }

    pub fn war_for(&self, country: CountryId) -> Option<(WarId, &War)> {
        self.wars
            .iter()
            .find(|(_, war)| war.side(country).is_some())
            .map(|(&id, war)| (id, war))
    }

    pub fn has_coalition_history(&self) -> bool {
        !self.coalition_truces.is_empty()
            || self.wars.values().any(|war| {
                !war.attacker_supporters.is_empty()
                    || !war.defender_supporters.is_empty()
                    || !war.factions.is_empty()
                    || war.succession_country.is_some()
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
        army: &Army,
        to: ProvinceId,
    ) -> bool {
        let country = army.country;
        let from = army.province;
        from != to
            && owner(polity, to).is_some_and(|owner| {
                owner == country
                    || self
                        .war_for(country)
                        .is_some_and(|(_, war)| war.side(owner) == war.side_for_army(army))
            })
            && self.occupation(to).is_none_or(|occupier| {
                occupier == country
                    || self
                        .war_for(country)
                        .is_some_and(|(_, war)| war.side(occupier) == war.side_for_army(army))
            })
            && graph.neighbors(from).is_some_and(|edges| {
                edges
                    .iter()
                    .any(|edge| edge.to == to && edge.kind != EdgeKind::Port)
            })
            && self.war_for(country).is_none_or(|(_, war)| {
                self.armies.values().all(|other| {
                    other.province != to || war.side_for_army(other) == war.side_for_army(army)
                })
            })
    }

    fn can_retreat_as(
        &self,
        polity: &PolityState,
        graph: &ProvinceGraph,
        army: &Army,
        winner_side: WarSide,
        to: ProvinceId,
    ) -> bool {
        let from = army.province;
        let Some((_, war)) = self.war_for(army.country) else {
            return false;
        };
        let losing_side = war.side_for_army(army);
        from != to
            && losing_side.is_some_and(|side| side != winner_side)
            && owner(polity, to)
                .is_some_and(|owner| owner == army.country || war.side(owner) == losing_side)
            && self
                .occupation(to)
                .is_none_or(|occupier| war.side(occupier) == losing_side)
            && graph.neighbors(from).is_some_and(|edges| {
                edges
                    .iter()
                    .any(|edge| edge.to == to && edge.kind != EdgeKind::Port)
            })
            && self
                .armies
                .values()
                .all(|other| other.province != to || war.side_for_army(other) == losing_side)
    }

    /// Clears war-local faction assignments after a custom war resolution.
    /// The caller owns recording the returned army snapshots as state changes.
    pub(crate) fn clear_faction_overrides(&mut self, war_id: WarId) -> Vec<(ArmyId, Army)> {
        let Some(war) = self.wars.get(&war_id) else {
            return Vec::new();
        };
        let factions: BTreeSet<_> = war
            .factions
            .iter()
            .filter_map(|(&id, faction)| (!faction.country_backed).then_some(id))
            .collect();
        self.armies
            .iter_mut()
            .filter_map(|(&id, army)| {
                if army
                    .faction_override
                    .is_some_and(|faction| factions.contains(&faction))
                {
                    army.faction_override = None;
                    Some((id, army.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn valid_cession(&self, polity: &PolityState, war: &War) -> bool {
        war.succession_country.is_none()
            && war.score >= self.rules.peace_score
            && war
                .occupations
                .get(&war.goal)
                .is_some_and(|occupier| war.side(*occupier) == Some(WarSide::Attacker))
            && polity.countries.get(&war.defender).is_some_and(|defender| {
                defender.provinces.contains(&war.goal)
                    && (war.independence
                        || defender.provinces.len() > 1
                        || defender.provinces.iter().all(|province| {
                            war.occupations.get(province).is_some_and(|occupier| {
                                war.side(*occupier) == Some(WarSide::Attacker)
                            })
                        }))
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
                || (war.attacker != war.defender && !parties.insert(war.defender))
                || (war.attacker == war.defender && war.succession_country != Some(war.attacker))
                || (war.attacker != war.defender && war.succession_country.is_some())
                || war
                    .succession_country
                    .is_some_and(|country| country != war.attacker)
                || war
                    .attacker_supporters
                    .iter()
                    .chain(&war.defender_supporters)
                    .any(|&country| {
                        !polity.countries.contains_key(&country) || !parties.insert(country)
                    })
                || war.factions.iter().any(|(&leader, faction)| {
                    faction
                        .claimant
                        .is_some_and(|claimant| !polity.characters.contains_key(&claimant))
                        || (faction.country_backed && !polity.countries.contains_key(&leader))
                        || (!faction.country_backed
                            && (polity.countries.contains_key(&leader)
                                || faction.name.as_ref().is_none_or(|name| {
                                    name.trim().is_empty() || name.chars().any(char::is_control)
                                })
                                || faction.dynasty.as_ref().is_none_or(|dynasty| {
                                    dynasty.trim().is_empty()
                                        || dynasty.chars().any(char::is_control)
                                })))
                        || !parties.insert(leader)
                        || faction.members.contains(&leader)
                        || faction.members.iter().any(|&country| {
                            !polity.countries.contains_key(&country) || !parties.insert(country)
                        })
                })
                || war.started_at > tick
                || if war.succession_country.is_some() {
                    !graph
                        .get(war.goal)
                        .is_some_and(|province| province.kind == ProvinceKind::Land)
                } else {
                    !self.can_reach_goal(
                        graph,
                        polity,
                        war.attacker,
                        war.defender,
                        war.goal,
                        war.independence,
                    )
                }
                || (war.independence && polity.countries[&war.defender].capital != war.goal)
                || (war.succession_country.is_some() && war.offer.is_some())
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
                || army.faction_override.is_some_and(|faction| {
                    self.wars
                        .values()
                        .filter(|war| war.side(army.country).is_some())
                        .filter(|war| {
                            war.succession_country == Some(army.country)
                                && war
                                    .factions
                                    .get(&faction)
                                    .is_some_and(|definition| !definition.country_backed)
                        })
                        .count()
                        != 1
                })
                || !self.can_enter(polity, army.country, army.province)
                || army.soldiers == 0
                || army.soldiers > self.rules.max_army_size
                || army.morale < Fixed::ZERO
                || army.recruitment_days_remaining > ARMY_RECRUITMENT_DAYS
                || (army.recruitment_days_remaining > 0 && army.movement.is_some())
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
                controller.is_some_and(|holder| {
                    self.armies.values().any(|army| {
                        army.country == siege.besieger
                            && army.province == province
                            && army.movement.is_none()
                            && war.side_for_army(army).is_some_and(|side| {
                                war.side(holder)
                                    .is_some_and(|holder_side| side != holder_side)
                            })
                    })
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

#[cfg(test)]
mod faction_tests {
    use super::*;

    fn war() -> War {
        War {
            attacker: CountryId::new(1),
            defender: CountryId::new(2),
            succession_country: None,
            attacker_supporters: BTreeSet::new(),
            defender_supporters: BTreeSet::new(),
            factions: BTreeMap::from([
                (
                    CountryId::new(4),
                    WarFaction {
                        members: BTreeSet::from([CountryId::new(5)]),
                        name: None,
                        dynasty: None,
                        claimant: None,
                        country_backed: true,
                    },
                ),
                (
                    CountryId::new(u32::MAX),
                    WarFaction {
                        members: BTreeSet::new(),
                        name: Some("Aristocrats".to_owned()),
                        dynasty: Some("House of Vale".to_owned()),
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
        }
    }

    #[test]
    fn country_backed_factions_assign_members_to_their_own_side() {
        let war = war();
        assert_eq!(
            war.side(CountryId::new(4)),
            Some(WarSide::Faction(CountryId::new(4)))
        );
        assert_eq!(
            war.side(CountryId::new(5)),
            Some(WarSide::Faction(CountryId::new(4)))
        );
        assert!(war.opposed(CountryId::new(1), CountryId::new(4)));
        assert!(!war.opposed(CountryId::new(4), CountryId::new(5)));
    }

    #[test]
    fn internal_army_override_keeps_country_owner_but_changes_war_side() {
        let war = war();
        let army = Army {
            country: CountryId::new(1),
            faction_override: Some(CountryId::new(u32::MAX)),
            province: ProvinceId::new(1),
            soldiers: 50,
            morale: Fixed::ONE,
            recruitment_days_remaining: 0,
            movement: None,
        };
        assert_eq!(army.country, CountryId::new(1));
        assert_eq!(
            war.side_for_army(&army),
            Some(WarSide::Faction(CountryId::new(u32::MAX)))
        );
        assert_ne!(war.side(army.country), war.side_for_army(&army));
    }

    #[test]
    fn strongest_faction_wins_and_ties_use_the_lowest_side_id() {
        let war = war();
        let strengths = BTreeMap::from([
            (WarSide::Attacker, 10),
            (WarSide::Defender, 10),
            (WarSide::Faction(CountryId::new(4)), 10),
        ]);
        assert_eq!(war.strongest_side(&strengths), Some(WarSide::Attacker));

        let strengths = BTreeMap::from([
            (WarSide::Attacker, 10),
            (WarSide::Defender, 20),
            (WarSide::Faction(CountryId::new(4)), 30),
        ]);
        assert_eq!(
            war.strongest_side(&strengths),
            Some(WarSide::Faction(CountryId::new(4)))
        );
    }
}
