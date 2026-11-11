use serde::{Deserialize, Serialize};
use strategy_sim::{Fixed, ModKey, WarRules};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WarRulesFile {
    raise_cost_per_soldier: i64,
    max_army_size: u32,
    max_morale: i64,
    morale_recovery_per_tick: i64,
    retreat_morale_threshold: i64,
    winner_casualty_rate: i64,
    loser_casualty_rate: i64,
    battle_morale_loss: i64,
    base_supply: u32,
    supply_per_development: u32,
    attrition_rate: i64,
    siege_days_per_level: u32,
    battle_score: i32,
    occupation_score: i32,
    peace_score: i32,
    truce_ticks: u32,
    combat_modifier: String,
    morale_modifier: String,
}

impl From<WarRulesFile> for WarRules {
    fn from(file: WarRulesFile) -> Self {
        Self {
            raise_cost_per_soldier: Fixed::from_raw(file.raise_cost_per_soldier),
            max_army_size: file.max_army_size,
            max_morale: Fixed::from_raw(file.max_morale),
            morale_recovery_per_tick: Fixed::from_raw(file.morale_recovery_per_tick),
            retreat_morale_threshold: Fixed::from_raw(file.retreat_morale_threshold),
            winner_casualty_rate: Fixed::from_raw(file.winner_casualty_rate),
            loser_casualty_rate: Fixed::from_raw(file.loser_casualty_rate),
            battle_morale_loss: Fixed::from_raw(file.battle_morale_loss),
            base_supply: file.base_supply,
            supply_per_development: file.supply_per_development,
            attrition_rate: Fixed::from_raw(file.attrition_rate),
            siege_days_per_level: file.siege_days_per_level,
            battle_score: file.battle_score,
            occupation_score: file.occupation_score,
            peace_score: file.peace_score,
            truce_ticks: file.truce_ticks,
            combat_modifier: ModKey::new(file.combat_modifier),
            morale_modifier: ModKey::new(file.morale_modifier),
        }
    }
}

impl From<&WarRules> for WarRulesFile {
    fn from(rules: &WarRules) -> Self {
        Self {
            raise_cost_per_soldier: rules.raise_cost_per_soldier.raw(),
            max_army_size: rules.max_army_size,
            max_morale: rules.max_morale.raw(),
            morale_recovery_per_tick: rules.morale_recovery_per_tick.raw(),
            retreat_morale_threshold: rules.retreat_morale_threshold.raw(),
            winner_casualty_rate: rules.winner_casualty_rate.raw(),
            loser_casualty_rate: rules.loser_casualty_rate.raw(),
            battle_morale_loss: rules.battle_morale_loss.raw(),
            base_supply: rules.base_supply,
            supply_per_development: rules.supply_per_development,
            attrition_rate: rules.attrition_rate.raw(),
            siege_days_per_level: rules.siege_days_per_level,
            battle_score: rules.battle_score,
            occupation_score: rules.occupation_score,
            peace_score: rules.peace_score,
            truce_ticks: rules.truce_ticks,
            combat_modifier: rules.combat_modifier.as_str().into(),
            morale_modifier: rules.morale_modifier.as_str().into(),
        }
    }
}
