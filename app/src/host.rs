use std::panic::{catch_unwind, AssertUnwindSafe};

use strategy_ai::{decide, AiContext, Reason};
use strategy_sim::{Command, CountryId, GameState, TickReport, TimeError};

/// Advances the authoritative simulation with the host's AI implementation.
pub fn tick_host(state: &mut GameState) -> Result<TickReport, TimeError> {
    tick_with_decider(state, decide)
}

fn tick_with_decider(
    state: &mut GameState,
    mut decide_ai: impl FnMut(&GameState, CountryId, &mut AiContext) -> Vec<(Command, Reason)>,
) -> Result<TickReport, TimeError> {
    state.tick(|snapshot, country| {
        let mut context = AiContext::new(snapshot, country);
        // The decider borrows an immutable snapshot and owns no simulation state.
        // A failed decision discards this country's temporary context.
        let decision = catch_unwind(AssertUnwindSafe(|| {
            decide_ai(snapshot, country, &mut context)
        }));
        match decision {
            Ok(commands) => commands.into_iter().map(|(command, _)| command).collect(),
            Err(_) => {
                eprintln!(
                    "AI decision panicked for country {} at tick {}",
                    country.get(),
                    snapshot.time().tick().get()
                );
                Vec::new()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU32, NonZeroU64},
    };

    use strategy_data::{load_game, save_game, state_hash};
    use strategy_sim::{
        derive_ai_rng, AiProfileKey, Controller, CountryState, Date, Effect, Fixed, MapIdentity,
        ModKey, ModifierKind, ModifierSourceId, ModifierStore, ProvinceGraph, ScheduledEffect,
        SimTime, Speed, Tick,
    };

    use super::*;

    const FIRST: CountryId = CountryId::new(3);
    const LAST: CountryId = CountryId::new(9);
    const SEED: u64 = 42;

    fn all_ai_state() -> GameState {
        let country = || {
            CountryState::new(
                Controller::Ai(AiProfileKey::new("stub")),
                BTreeSet::new(),
                false,
                ModifierStore::default(),
            )
        };
        let map = ProvinceGraph::try_new(
            MapIdentity {
                version: NonZeroU32::new(1).unwrap(),
                sha256: [0; 32],
            },
            [],
        )
        .unwrap();
        GameState::new(
            SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
            SEED,
            map,
            BTreeMap::from([(LAST, country()), (FIRST, country())]),
            [ScheduledEffect {
                due: Tick::new(1),
                country: FIRST,
                source: ModifierSourceId::new(1),
                effect: Effect::AddModifier {
                    key: ModKey::new("test_stat"),
                    value: Fixed::ONE,
                    kind: ModifierKind::Additive,
                    duration_days: NonZeroU64::new(1),
                },
            }],
        )
        .unwrap()
    }

    #[test]
    fn all_ai_stub_replays_for_ten_thousand_ticks() {
        let mut first_run = all_ai_state();
        let mut second_run = first_run.clone();

        let key = ModKey::new("test_stat");
        for tick in 1..=10_000 {
            let mut decisions = Vec::new();
            let observed = tick_with_decider(&mut first_run, |snapshot, country, ctx| {
                let commands = decide(snapshot, country, ctx);
                let value = snapshot
                    .country(FIRST)
                    .and_then(|facts| facts.effective_value(Fixed::ONE, &key));
                decisions.push((
                    country,
                    snapshot.time().tick(),
                    ctx.rng().next_u64(),
                    value,
                    commands.len(),
                ));
                commands
            })
            .unwrap();

            let expected_value = if tick == 1 {
                Fixed::from_raw(2_000)
            } else {
                Fixed::ONE
            };
            assert_eq!(
                decisions,
                [FIRST, LAST].map(|country| (
                    country,
                    Tick::new(tick),
                    derive_ai_rng(SEED, tick, country).next_u64(),
                    Some(expected_value),
                    0,
                ))
            );
            assert_eq!(observed, tick_host(&mut second_run).unwrap());
            if tick == 5_000 {
                let bytes = save_game(&second_run).unwrap();
                second_run = load_game(&bytes, second_run.province_graph().clone()).unwrap();
                assert_eq!(
                    state_hash(&first_run).unwrap(),
                    state_hash(&second_run).unwrap()
                );
            }
        }

        assert_eq!(first_run.time().tick(), Tick::new(10_000));
        assert_eq!(first_run, second_run);
        assert_eq!(
            state_hash(&first_run).unwrap(),
            state_hash(&second_run).unwrap()
        );
    }

    #[test]
    fn a_panicking_decision_does_not_skip_the_next_country() {
        let mut state = all_ai_state();
        let mut visited = Vec::new();
        tick_with_decider(&mut state, |_, country, _| {
            visited.push(country);
            assert_ne!(country, FIRST, "simulated decider panic");
            Vec::new()
        })
        .unwrap();

        assert_eq!(visited, [FIRST, LAST]);
        assert_eq!(state.time().tick(), Tick::new(1));
    }
}
