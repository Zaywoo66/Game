use std::{collections::BTreeSet, time::Instant};

use strategy_data::state_hash;
use strategy_sim::{
    Army, Command, CommandActor, Controller, Date, Fixed, GameState, PlayerId, ProvinceKind,
    StateChange, TimeCommand,
};

use crate::{load_embedded_scenario, tick_host_with_log, AiDecisionLog};

#[derive(Debug, PartialEq, Eq)]
pub struct AutoplaySummary {
    pub seed: u64,
    pub ticks: u64,
    pub wars_per_decade: Vec<u32>,
    pub treaties: usize,
    pub cessions: usize,
    pub country_count: usize,
    pub largest_country: usize,
    pub median_tick_ns: u128,
    pub p95_tick_ns: u128,
    pub peak_tick_ns: u128,
    pub final_hash: [u8; 32],
}

/// Recreates the authored opening with a supplied seed and all countries on AI.
pub fn new_autoplay(seed: u64) -> Result<GameState, String> {
    let scenario = load_embedded_scenario()?;
    let polity = scenario.polity().ok_or("scenario has no polity")?;
    let military = scenario
        .war_state()
        .ok_or("scenario has no military rules")?;
    let mut game = GameState::new(
        *scenario.time(),
        seed,
        scenario.province_graph().clone(),
        scenario
            .countries()
            .map(|(id, country)| (id, country.clone()))
            .collect(),
        [],
    )
    .map_err(|error| format!("invalid autoplay clock: {error:?}"))?
    .with_polity(polity.clone())
    .map_err(|error| format!("invalid autoplay polity: {error:?}"))?
    .with_opinions(scenario.opinions())
    .map_err(|error| format!("invalid autoplay opinions: {error:?}"))?
    .with_war_state(military.clone())
    .map_err(|error| format!("invalid autoplay military: {error:?}"))?;
    if let Some(diplomacy) = scenario.diplomacy() {
        game = game
            .with_diplomacy(diplomacy.clone())
            .map_err(|error| format!("invalid autoplay diplomacy: {error:?}"))?;
    }
    if let Some(events) = scenario.events() {
        game = game
            .with_event_state(events.clone())
            .map_err(|error| format!("invalid autoplay events: {error:?}"))?;
    }
    if let Some(navy) = scenario.navy() {
        game = game
            .with_navy(navy.clone())
            .map_err(|error| format!("invalid autoplay navy: {error:?}"))?;
    }
    let host = CommandActor::Host(PlayerId::new(1));
    for (&country, nation) in &polity.countries {
        game.apply(
            host,
            &Command::SetController {
                country,
                controller: Controller::Ai(nation.ai_profile.clone()),
            },
        )
        .map_err(|error| format!("cannot assign AI controller: {error:?}"))?;
    }
    game.apply(host, &Command::Time(TimeCommand::SetPaused(false)))
        .map_err(|error| format!("cannot start autoplay: {error:?}"))?;
    Ok(game)
}

/// Runs the host without a window and checks authoritative invariants every day.
pub fn advance_autoplay(game: &mut GameState, end: Date) -> Result<AutoplaySummary, String> {
    let start_tick = game.time().tick().get();
    let start_year = game.time().start_date().components().0;
    let initial_count = game
        .polity()
        .ok_or("autoplay requires a polity")?
        .countries
        .len();
    let mut decisions = AiDecisionLog::default();
    let mut samples = Vec::new();
    let mut wars_per_decade = Vec::new();
    let mut previous_wars = {
        let war = game.war_state().ok_or("autoplay requires military rules")?;
        war.wars.len() + war.treaties.len()
    };
    while game.time().date() < end {
        let before = Instant::now();
        let report = tick_host_with_log(game, &mut decisions)
            .map_err(|error| format!("cannot advance autoplay clock: {error:?}"))?;
        samples.push(before.elapsed().as_nanos());
        let tick = game.time().tick().get();
        if report
            .delta
            .changes()
            .iter()
            .all(|change| !matches!(change, StateChange::ClockAdvanced { .. }))
        {
            return Err(format!("autoplay did not advance at tick {tick}"));
        }
        if decisions.failures() > 0
            || !report.ai_rejections.is_empty()
            || !report.scheduled_failures.is_empty()
            || !report.polity_failures.is_empty()
            || !report.military_failures.is_empty()
        {
            return Err(format!("simulation failure at tick {tick}: {report:?}"));
        }
        check_invariants(game, initial_count).map_err(|error| format!("tick {tick}: {error}"))?;
        let military = game.war_state().unwrap();
        let decade = ((game.time().date().components().0 - start_year) / 10) as usize;
        if decade >= wars_per_decade.len() {
            wars_per_decade.resize(decade + 1, 0);
        }
        let all_wars = military.wars.len() + military.treaties.len();
        let new_wars = all_wars
            .checked_sub(previous_wars)
            .ok_or("war history was lost")?;
        wars_per_decade[decade] += new_wars as u32;
        previous_wars = all_wars;
    }
    samples.sort_unstable();
    let polity = game.polity().unwrap();
    let military = game.war_state().unwrap();
    Ok(AutoplaySummary {
        seed: game.seed(),
        ticks: game.time().tick().get() - start_tick,
        wars_per_decade,
        treaties: military.treaties.len(),
        cessions: military
            .treaties
            .values()
            .filter(|treaty| treaty.ceded.is_some())
            .count(),
        country_count: polity.countries.len(),
        largest_country: polity
            .countries
            .values()
            .map(|country| country.provinces.len())
            .max()
            .unwrap_or(0),
        median_tick_ns: samples.get(samples.len() / 2).copied().unwrap_or(0),
        p95_tick_ns: samples
            .get(samples.len().saturating_sub(1) * 95 / 100)
            .copied()
            .unwrap_or(0),
        peak_tick_ns: samples.last().copied().unwrap_or(0),
        final_hash: state_hash(game).map_err(|error| error.to_string())?,
    })
}

fn check_invariants(game: &GameState, initial_count: usize) -> Result<(), &'static str> {
    let polity = game.polity().unwrap();
    let military = game.war_state().unwrap();
    if polity.countries.len() != initial_count {
        return Err("country roster changed");
    }
    let debt_floor = Fixed::from_raw(-polity.rules.debt_limit.raw());
    let mut owned = BTreeSet::new();
    for country in polity.countries.values() {
        if country.economy.treasury < debt_floor || !country.provinces.contains(&country.capital) {
            return Err("treasury breached the debt floor or a country lost its capital");
        }
        for &province in &country.provinces {
            if !owned.insert(province)
                || game
                    .province_graph()
                    .get(province)
                    .is_none_or(|province| province.kind != ProvinceKind::Land)
            {
                return Err("province ownership is invalid");
            }
        }
    }
    for army in military.armies.values() {
        if !army_valid(game, army) {
            return Err("army state is invalid");
        }
    }
    let mut belligerents = BTreeSet::new();
    for war in military.wars.values() {
        if game.owner_of_province(war.goal) != Some(war.defender)
            || std::iter::once(war.attacker)
                .chain(std::iter::once(war.defender))
                .chain(war.attacker_supporters.iter().copied())
                .chain(war.defender_supporters.iter().copied())
                .any(|country| !belligerents.insert(country))
        {
            return Err("war participants or goal are invalid");
        }
        for (&province, &occupier) in &war.occupations {
            let owner = game.owner_of_province(province);
            if owner.is_none_or(|owner| !war.opposed(occupier, owner)) {
                return Err("occupation does not match the opposing owner");
            }
        }
    }
    Ok(())
}

fn army_valid(game: &GameState, army: &Army) -> bool {
    let Some(military) = game.war_state() else {
        return false;
    };
    army.soldiers > 0
        && army.soldiers <= military.rules.max_army_size
        && army.morale >= Fixed::ZERO
        && game
            .effective_army_morale_limit(army.country)
            .is_some_and(|limit| army.morale <= limit)
        && game
            .polity()
            .is_some_and(|polity| polity.countries.contains_key(&army.country))
        && game
            .province_graph()
            .get(army.province)
            .is_some_and(|province| province.kind == ProvinceKind::Land)
}

#[cfg(test)]
mod tests {
    use super::*;
    use strategy_sim::{ArmyId, CountryState, Modifier, ModifierKind, ModifierSourceId};

    #[test]
    fn autoplay_accepts_modified_morale_ceiling_but_rejects_excess() {
        let opening = new_autoplay(1440).unwrap();
        let (&country_id, nation) = opening.polity().unwrap().countries.iter().next().unwrap();
        let army = Army {
            country: country_id,
            province: nation.capital,
            soldiers: 1_000,
            morale: Fixed::ONE,
            movement: None,
        };
        let country = opening.country(army.country).unwrap();
        let source = ModifierSourceId::new(999_999);
        let mut modifiers = country.modifiers().clone();
        modifiers
            .insert_source(
                source,
                None,
                vec![Modifier {
                    source,
                    key: opening.war_state().unwrap().rules.morale_modifier.clone(),
                    value: Fixed::from_raw(100),
                    kind: ModifierKind::Additive,
                }],
            )
            .unwrap();
        let countries = opening
            .countries()
            .map(|(id, state)| {
                if id == army.country {
                    (
                        id,
                        CountryState::new(
                            state.controller().clone(),
                            state.known_technologies().map(str::to_owned).collect(),
                            false,
                            modifiers.clone(),
                        ),
                    )
                } else {
                    (id, state.clone())
                }
            })
            .collect();
        let mut military = opening.war_state().unwrap().clone();
        let army_id = ArmyId::new(military.next_army_id);
        military.next_army_id += 1;
        military.armies.insert(army_id, army.clone());
        let game = GameState::new(
            *opening.time(),
            opening.seed(),
            opening.province_graph().clone(),
            countries,
            [],
        )
        .unwrap()
        .with_polity(opening.polity().unwrap().clone())
        .unwrap()
        .with_war_state(military)
        .unwrap();
        let mut at_cap = army.clone();
        at_cap.morale = game.effective_army_morale_limit(army.country).unwrap();
        assert_eq!(at_cap.morale, Fixed::from_raw(1_100));
        assert!(army_valid(&game, &at_cap));
        let mut over_cap = at_cap;
        over_cap.morale = game
            .effective_army_morale_limit(army.country)
            .unwrap()
            .checked_add(Fixed::from_raw(1))
            .unwrap();
        assert!(!army_valid(&game, &over_cap));
    }
}
