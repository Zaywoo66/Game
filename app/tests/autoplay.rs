use std::collections::BTreeSet;

use strategy_app::{load_embedded_scenario, tick_host_with_log, AiDecisionLog};
use strategy_data::{load_game, save_game, state_hash};
use strategy_sim::{Command, CommandActor, Controller, Date, Fixed, PlayerId, TimeCommand};

#[test]
#[ignore = "101-year deterministic AI soak; run for release acceptance or AI changes"]
fn the_1440_scenario_replays_for_101_years_under_ai_control() {
    let host = CommandActor::Host(PlayerId::new(1));
    let mut state = load_embedded_scenario().unwrap();
    let initial = state.polity().unwrap().clone();
    let debt_floor = Fixed::from_raw(-initial.rules.debt_limit.raw());
    assert!(initial.ai_profiles.len() >= 2);
    for (&country, polity) in &initial.countries {
        state
            .apply(
                host,
                &Command::SetController {
                    country,
                    controller: Controller::Ai(polity.ai_profile.clone()),
                },
            )
            .unwrap();
    }
    state
        .apply(host, &Command::Time(TimeCommand::SetPaused(false)))
        .unwrap();

    let mut replay = state.clone();
    let mut decisions = AiDecisionLog::default();
    let mut replay_decisions = AiDecisionLog::default();
    let mut observed_succession = false;
    let mut observed_regency = false;
    let mut observed_policy = false;
    let mut replayed_save = false;
    let fifty_years = Date::new(1490, 1, 1).unwrap();
    let end = Date::new(1541, 1, 1).unwrap();
    while state.time().date() < end {
        let report = tick_host_with_log(&mut state, &mut decisions).unwrap();
        assert!(report.polity_failures.is_empty(), "{report:?}");
        assert!(report.scheduled_failures.is_empty(), "{report:?}");
        assert!(report.ai_rejections.is_empty(), "{report:?}");
        assert_eq!(
            report,
            tick_host_with_log(&mut replay, &mut replay_decisions).unwrap()
        );

        let date = state.time().date();
        let mut provinces = BTreeSet::new();
        let polity = state.polity().unwrap();
        for (&id, country) in &polity.countries {
            assert!(country.provinces.contains(&country.capital));
            for province in &country.provinces {
                assert!(provinces.insert(*province));
                assert_eq!(state.owner_of_province(*province), Some(id));
            }
            assert!(country.economy.treasury >= debt_floor);
            assert!(country.economy.treasury >= Fixed::ZERO || country.economy.bankrupt);
            if country.economy.bankrupt {
                assert!(country.economy.last_month.is_some());
                assert!(country.economy.treasury <= Fixed::ZERO);
            }
            let succession = country.succession;
            if succession.ruler.is_some() {
                assert_ne!(succession.ruler, succession.heir);
            }
            for character in [succession.ruler, succession.heir].into_iter().flatten() {
                let candidate = state.character(character).unwrap();
                assert_eq!(candidate.country, id);
                assert!(candidate.is_alive_on(date));
            }
            observed_succession |= succession.ruler.is_some()
                && succession.ruler != initial.countries[&id].succession.ruler;
            observed_regency |= state.is_in_regency(id).unwrap();
            observed_policy |= country.economy.policy != initial.countries[&id].economy.policy;
        }
        if date == fifty_years {
            let bytes = save_game(&replay).unwrap();
            replay = load_game(&bytes, replay.province_graph().clone()).unwrap();
            assert_eq!(state_hash(&state).unwrap(), state_hash(&replay).unwrap());
            replayed_save = true;
        }
    }
    assert!(replayed_save && observed_succession && observed_regency && observed_policy);
    assert!(decisions.entries().next().is_some());
    assert_eq!(state_hash(&state).unwrap(), state_hash(&replay).unwrap());
}
