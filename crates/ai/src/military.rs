use std::{cmp::Reverse, collections::BTreeSet};

use strategy_sim::{
    Army, ArmyId, Command, CommandActor, CountryId, CountryPolity, EdgeKind, FiscalAiProfile,
    Fixed, GameState, ProvinceId, War, WarId, WarSide, WarState,
};

use crate::{AiContext, MilitaryReason, Reason};

const ORDER_PERIOD: u64 = 3;
const STRATEGY_PERIOD: u64 = 31;
const MAX_WAR_CANDIDATES: usize = 32;
const WAR_TIMEOUT_DAYS: u64 = 365 * 2;

pub(super) fn decide(
    state: &GameState,
    country: CountryId,
    nation: &CountryPolity,
    profile: &FiscalAiProfile,
    ctx: &mut AiContext,
) -> Option<(Command, Reason)> {
    let military = state.war_state()?;
    let tick = state.time().tick().get();
    if !on_turn(tick, country, ORDER_PERIOD) {
        return None;
    }
    let planner = Planner {
        state,
        military,
        country,
        nation,
        profile,
    };
    if let Some((war_id, war)) = military.war_for(country) {
        return planner.peace(war_id, war).or_else(|| planner.orders(war));
    }

    planner.raise().or_else(|| {
        on_turn(tick, country, STRATEGY_PERIOD).then_some(())?;
        planner.declare_war(ctx)
    })
}

fn on_turn(tick: u64, country: CountryId, period: u64) -> bool {
    (tick % period + u64::from(country.get())) % period == 0
}

struct Planner<'a> {
    state: &'a GameState,
    military: &'a WarState,
    country: CountryId,
    nation: &'a CountryPolity,
    profile: &'a FiscalAiProfile,
}

impl Planner<'_> {
    fn checked(
        &self,
        command: Command,
        action: MilitaryReason,
        score: i32,
    ) -> Option<(Command, Reason)> {
        self.state
            .validate_command(CommandActor::Ai(self.country), &command)
            .ok()?;
        Some((command, Reason::Military { action, score }))
    }

    fn armies(&self) -> impl Iterator<Item = (ArmyId, &Army)> {
        self.military
            .armies
            .iter()
            .filter(move |(_, army)| army.country == self.country)
            .map(|(&id, army)| (id, army))
    }

    fn strength(&self, country: CountryId) -> u64 {
        self.military
            .armies
            .values()
            .filter(|army| army.country == country)
            .map(|army| u64::from(army.soldiers))
            .sum()
    }

    fn levies(&self, nation: &CountryPolity) -> u32 {
        let provinces = nation.provinces.len() as u64;
        let supply = u64::from(self.military.rules.base_supply);
        let soldiers = supply * provinces / (provinces + 4);
        self.military.rules.max_army_size.min(soldiers as u32)
    }

    fn raise(&self) -> Option<(Command, Reason)> {
        if self.armies().next().is_some() {
            return None;
        }
        let desired = self.levies(self.nation);
        let cost = self.military.rules.raise_cost_per_soldier.raw();
        let funds = self.nation.economy.treasury.raw().max(0);
        let affordable = funds
            .checked_div(cost)
            .map_or(desired, |funds| u32::try_from(funds).unwrap_or(u32::MAX));
        let soldiers = affordable.min(desired);
        if soldiers == 0 {
            return None;
        }
        self.checked(
            Command::RaiseArmy {
                country: self.country,
                province: self.nation.capital,
                soldiers,
            },
            MilitaryReason::RaiseArmy,
            i32::try_from(soldiers).unwrap_or(i32::MAX),
        )
    }

    fn declare_war(&self, ctx: &mut AiContext) -> Option<(Command, Reason)> {
        let strength = self.strength(self.country);
        if strength == 0 || self.nation.economy.treasury < Fixed::ZERO {
            return None;
        }
        let graph = self.state.province_graph();
        let borders = self.nation.provinces.iter().flat_map(|&from| {
            graph
                .get(from)
                .into_iter()
                .flat_map(move |province| province.neighbors.iter().map(move |edge| (from, edge)))
        });
        let neighbors: BTreeSet<_> = borders
            .filter(|(_, edge)| edge.kind != EdgeKind::Port)
            .filter_map(|(from, edge)| {
                let defender = self.state.owner_of_province(edge.to)?;
                if defender == self.country {
                    return None;
                }
                self.state
                    .find_army_path(self.country, self.nation.capital, from)?;
                Some(defender)
            })
            .collect();
        neighbors
            .into_iter()
            .filter_map(|defender| {
                self.state
                    .country_polity(defender)
                    .map(|enemy| (defender, enemy))
            })
            .flat_map(|(defender, enemy)| {
                enemy
                    .provinces
                    .iter()
                    .filter(move |&&goal| goal != enemy.capital)
                    .map(move |&goal| (goal, defender, enemy))
            })
            .take(MAX_WAR_CANDIDATES)
            .filter_map(|(goal, defender, enemy)| {
                let opposition = self.strength(defender).max(u64::from(self.levies(enemy)));
                if strength <= opposition {
                    return None;
                }
                let advantage = (strength - opposition) * 1000 / strength;
                let weights = self.profile.weights;
                let score = (advantage * u64::from(weights.aggression + weights.expansion) / 1000)
                    as i32
                    - i32::from(weights.caution) / 4
                    + (ctx.rng().next_u64() % 11) as i32;
                if score < self.profile.thresholds.war_min_score {
                    return None;
                }
                self.checked(
                    Command::DeclareWar {
                        country: self.country,
                        defender,
                        goal,
                    },
                    MilitaryReason::DeclareAgainstWeakerNeighbor,
                    score,
                )
                .map(|decision| ((score, Reverse(goal)), decision))
            })
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, decision)| decision)
    }

    fn peace(&self, id: WarId, war: &War) -> Option<(Command, Reason)> {
        if !war.is_leader(self.country) {
            return None;
        }
        let attacking = war.side(self.country) == Some(WarSide::Attacker);
        let enemy = if attacking {
            war.defender
        } else {
            war.attacker
        };
        let local_score = if attacking {
            war.score
        } else {
            war.score.saturating_neg()
        };
        let elapsed = self
            .state
            .time()
            .tick()
            .get()
            .saturating_sub(war.started_at.get());
        let goal_occupied = war
            .occupations
            .get(&war.goal)
            .is_some_and(|occupier| war.side(*occupier) == Some(WarSide::Attacker));
        let decisive = war.score >= self.military.rules.peace_score && goal_occupied;
        let outmatched = self.strength(self.country) < self.strength(enemy);
        let peace_threshold = self.profile.thresholds.peace_accept_score.max(0);
        let losing = local_score <= peace_threshold.saturating_neg();

        if let Some(offer) = war.offer {
            if offer.offered_by == self.country {
                return None;
            }
            let acceptable = if offer.cede_goal {
                attacking || (decisive && (losing || outmatched || elapsed >= WAR_TIMEOUT_DAYS))
            } else {
                (elapsed >= WAR_TIMEOUT_DAYS || losing) && !(attacking && decisive)
            };
            if acceptable {
                return self.checked(
                    Command::AcceptPeace {
                        country: self.country,
                        war: id,
                    },
                    MilitaryReason::AcceptPeace,
                    local_score,
                );
            }
        }

        let cede_goal = decisive && (attacking || losing || outmatched);
        if cede_goal || elapsed >= WAR_TIMEOUT_DAYS {
            return self.checked(
                Command::OfferPeace {
                    country: self.country,
                    war: id,
                    cede_goal,
                },
                if cede_goal {
                    MilitaryReason::OfferGoal
                } else {
                    MilitaryReason::OfferWhitePeace
                },
                local_score,
            );
        }
        None
    }

    fn orders(&self, war: &War) -> Option<(Command, Reason)> {
        let attacking = war.side(self.country) == Some(WarSide::Attacker);
        let target = if attacking {
            war.goal
        } else {
            let threatened_capital = self.military.armies.values().any(|army| {
                war.opposed(self.country, army.country)
                    && (army.province == self.nation.capital
                        || army
                            .movement
                            .as_ref()
                            .and_then(|movement| movement.route.last())
                            .is_some_and(|leg| leg.to == self.nation.capital))
            });
            if threatened_capital {
                self.nation.capital
            } else {
                war.goal
            }
        };
        self.raise().or_else(|| {
            self.armies().find_map(|(id, army)| {
                self.retreat(id, army, war)
                    .or_else(|| self.reorganize(id, army))
                    .or_else(|| self.move_towards(id, army, target, attacking))
            })
        })
    }

    fn retreat(&self, id: ArmyId, army: &Army, war: &War) -> Option<(Command, Reason)> {
        let threat: u64 = self
            .military
            .armies
            .values()
            .filter(|unit| {
                war.opposed(self.country, unit.country) && unit.province == army.province
            })
            .map(|unit| u64::from(unit.soldiers))
            .sum();
        if threat <= u64::from(army.soldiers)
            || army.morale > self.military.rules.retreat_morale_threshold
        {
            return None;
        }
        self.state
            .province_graph()
            .get(army.province)?
            .neighbors
            .iter()
            .find_map(|edge| {
                self.checked(
                    Command::RetreatArmy {
                        country: self.country,
                        army: id,
                        destination: edge.to,
                    },
                    MilitaryReason::Retreat,
                    i32::try_from(threat - u64::from(army.soldiers)).unwrap_or(i32::MAX),
                )
            })
    }

    fn reorganize(&self, id: ArmyId, army: &Army) -> Option<(Command, Reason)> {
        if army.movement.is_some() {
            return None;
        }
        let supply = self.military.rules.base_supply;
        if army.soldiers > supply && army.soldiers > 1 {
            return self.checked(
                Command::SplitArmy {
                    country: self.country,
                    army: id,
                    soldiers: army.soldiers / 2,
                },
                MilitaryReason::ReduceSupplyPressure,
                i32::try_from(army.soldiers - supply).unwrap_or(i32::MAX),
            );
        }
        self.armies()
            .filter(|(other_id, other)| {
                *other_id > id
                    && other.province == army.province
                    && army.soldiers.saturating_add(other.soldiers) <= supply
            })
            .find_map(|(from, other)| {
                self.checked(
                    Command::MergeArmies {
                        country: self.country,
                        into: id,
                        from,
                    },
                    MilitaryReason::CombineArmies,
                    i32::try_from(army.soldiers + other.soldiers).unwrap_or(i32::MAX),
                )
            })
    }

    fn move_towards(
        &self,
        id: ArmyId,
        army: &Army,
        destination: ProvinceId,
        attacking: bool,
    ) -> Option<(Command, Reason)> {
        if army.province == destination || army.movement.is_some() {
            return None;
        }
        self.checked(
            Command::MoveArmy {
                country: self.country,
                army: id,
                destination,
            },
            if attacking {
                MilitaryReason::BesiegeWarGoal
            } else if destination == self.nation.capital {
                MilitaryReason::DefendCapital
            } else {
                MilitaryReason::DefendWarGoal
            },
            i32::from(if attacking {
                self.profile.weights.expansion
            } else {
                self.profile.weights.caution
            }),
        )
    }
}
