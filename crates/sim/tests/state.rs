use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU32, NonZeroU64},
};

use strategy_sim::{
    AiCommandRejection, AiProfileKey, Command, CommandActor, CommandError, Controller, CountryId,
    CountryState, Date, EdgeKind, Effect, EffectError, Fixed, GameState, MapIdentity, ModKey,
    Modifier, ModifierInsertError, ModifierKind, ModifierSourceId, ModifierStore, PlayerId,
    PreparedModifier, Province, ProvinceEdge, ProvinceGraph, ProvinceId, ProvinceKind,
    ScheduleError, ScheduledEffect, ScheduledEffectFailure, SimTime, Speed, StateChange, Tick,
    TimeCommand, TimeError, Trigger,
};

const HOST: CommandActor = CommandActor::Host(PlayerId::new(1));
const GUEST: CommandActor = CommandActor::Guest(PlayerId::new(2));

#[test]
fn only_host_can_change_country_controller() {
    let country = CountryId::new(5);
    let mut state = game(false, [(country, empty_country(human(2)))]);
    let command = Command::SetController {
        country,
        controller: ai(),
    };
    let original = state.clone();

    for actor in [GUEST, CommandActor::Ai(country)] {
        assert_eq!(
            state.validate_command(actor, &command),
            Err(CommandError::HostRequired)
        );
        assert_eq!(
            state.apply(actor, &command),
            Err(CommandError::HostRequired)
        );
        assert_eq!(state, original);
    }

    let missing = CountryId::new(99);
    let invalid = Command::SetController {
        country: missing,
        controller: ai(),
    };
    assert_eq!(
        state.validate_command(HOST, &invalid),
        Err(CommandError::UnknownCountry(missing))
    );
    assert_eq!(
        state.apply(HOST, &invalid),
        Err(CommandError::UnknownCountry(missing))
    );
    assert_eq!(state, original);

    assert_eq!(state.validate_command(HOST, &command), Ok(()));
    assert_eq!(state, original);
    assert_eq!(
        state.apply(HOST, &command).unwrap().changes(),
        &[StateChange::ControllerChanged {
            country,
            controller: ai(),
        }]
    );
    assert_eq!(state.country(country).unwrap().controller(), &ai());
    assert!(state.apply(HOST, &command).unwrap().changes().is_empty());
}

#[test]
fn time_commands_keep_authority_and_only_emit_real_changes() {
    let mut state = game(false, []);
    let speed = Command::Time(TimeCommand::SetSpeed(Speed::Four));
    let pause = Command::Time(TimeCommand::SetPaused(true));
    let resume = Command::Time(TimeCommand::SetPaused(false));
    let original = state.clone();

    for (actor, command, error) in [
        (GUEST, &speed, TimeError::HostRequired),
        (
            CommandActor::Ai(CountryId::new(5)),
            &pause,
            TimeError::HumanRequired,
        ),
    ] {
        assert_eq!(
            state.validate_command(actor, command),
            Err(CommandError::Time(error))
        );
        assert_eq!(state.apply(actor, command), Err(CommandError::Time(error)));
        assert_eq!(state, original);
    }

    assert_eq!(
        state.apply(GUEST, &pause).unwrap().changes(),
        &[StateChange::PauseChanged(true)]
    );
    assert_eq!(
        state.apply(GUEST, &resume).unwrap().changes(),
        &[StateChange::PauseChanged(false)]
    );
    assert_eq!(
        state.apply(HOST, &speed).unwrap().changes(),
        &[StateChange::SpeedChanged(Speed::Four)]
    );
    assert!(state.apply(HOST, &speed).unwrap().changes().is_empty());
    assert_eq!(state.time().speed(), Speed::Four);
    assert_eq!(state.time().tick(), Tick::ZERO);
}

#[test]
fn tick_expires_all_due_sources_before_ascending_ai_dispatch() {
    let human_country = CountryId::new(1);
    let first_ai = CountryId::new(3);
    let last_ai = CountryId::new(9);
    let key = ModKey::new("test_stat");
    let mut state = game(
        false,
        [
            (last_ai, country_with_modifiers(ai(), &key, &[8, 5])),
            (human_country, country_with_modifiers(human(2), &key, &[4])),
            (first_ai, country_with_modifiers(ai(), &key, &[2])),
        ],
    );
    let attempt = Command::SetController {
        country: human_country,
        controller: ai(),
    };
    let pause = Command::Time(TimeCommand::SetPaused(true));
    let next_date = Date::new(1440, 1, 2).unwrap();
    let mut visited = Vec::new();

    let report = state
        .tick(|snapshot, country| {
            visited.push(country);
            assert_eq!(snapshot.seed(), 42);
            assert_eq!(snapshot.time().tick(), Tick::new(1));
            assert_eq!(snapshot.time().date(), next_date);
            for (_, facts) in snapshot.countries() {
                assert_eq!(facts.effective_value(Fixed::ONE, &key), Some(Fixed::ONE));
            }
            assert_eq!(
                snapshot.country(human_country).unwrap().controller(),
                &human(2)
            );
            assert!(!snapshot.time().is_paused());
            if country == first_ai {
                vec![attempt.clone(), pause.clone()]
            } else {
                Vec::new()
            }
        })
        .unwrap();

    assert_eq!(visited, [first_ai, last_ai]);
    assert_eq!(
        report.delta.changes(),
        &[
            StateChange::ClockAdvanced {
                tick: Tick::new(1),
                date: next_date,
            },
            expired(human_country, 4),
            expired(first_ai, 2),
            expired(last_ai, 5),
            expired(last_ai, 8),
        ]
    );
    assert_eq!(
        report.ai_rejections,
        [
            AiCommandRejection {
                country: first_ai,
                command: attempt,
                error: CommandError::HostRequired,
            },
            AiCommandRejection {
                country: first_ai,
                command: pause,
                error: CommandError::Time(TimeError::HumanRequired),
            },
        ]
    );
}

#[test]
fn paused_or_out_of_range_tick_leaves_country_state_unchanged() {
    let country = CountryId::new(5);
    let key = ModKey::new("test_stat");
    let mut paused = game(true, [(country, country_with_modifiers(ai(), &key, &[4]))]);
    let previous = paused.clone();
    let report = paused
        .tick(|_, _| panic!("paused tick dispatched AI"))
        .unwrap();
    assert!(report.delta.changes().is_empty());
    assert!(report.ai_rejections.is_empty());
    assert_eq!(paused, previous);

    let mut overflow = GameState::new(
        SimTime::new(Date::new(u32::MAX, 12, 31).unwrap(), Speed::One, false),
        42,
        map(),
        BTreeMap::from([(country, country_with_modifiers(ai(), &key, &[4]))]),
        [],
    )
    .unwrap();
    let previous = overflow.clone();
    assert_eq!(
        overflow.tick(|_, _| panic!("out-of-range tick dispatched AI")),
        Err(TimeError::DateOverflow)
    );
    assert_eq!(overflow, previous);
}

#[test]
fn trigger_reads_the_selected_countrys_scenario_facts() {
    let a = CountryId::new(2);
    let b = CountryId::new(5);
    let state = game(
        false,
        [
            (
                a,
                CountryState::new(
                    ai(),
                    BTreeSet::from(["test_technology".to_owned()]),
                    true,
                    ModifierStore::default(),
                ),
            ),
            (b, empty_country(human(2))),
        ],
    );
    let trigger = Trigger::All(vec![
        Trigger::CountryHasTech("test_technology".to_owned()),
        Trigger::AtWar(true),
    ]);
    let previous = state.clone();

    assert!(trigger.evaluate(state.country(a).unwrap()));
    assert!(!trigger.evaluate(state.country(b).unwrap()));
    assert_eq!(state, previous);
}

fn game<const N: usize>(paused: bool, countries: [(CountryId, CountryState); N]) -> GameState {
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, paused),
        42,
        map(),
        BTreeMap::from(countries),
        [],
    )
    .unwrap()
}

fn human(player: u32) -> Controller {
    Controller::Human(PlayerId::new(player))
}

fn ai() -> Controller {
    Controller::Ai(AiProfileKey::new("test_profile"))
}

fn empty_country(controller: Controller) -> CountryState {
    CountryState::new(controller, BTreeSet::new(), false, ModifierStore::default())
}

fn country_with_modifiers(controller: Controller, key: &ModKey, sources: &[u32]) -> CountryState {
    let mut modifiers = ModifierStore::default();
    for &source in sources {
        let source = ModifierSourceId::new(source);
        modifiers
            .insert_source(
                source,
                Some(1),
                vec![Modifier {
                    source,
                    key: key.clone(),
                    kind: ModifierKind::Additive,
                    value: Fixed::ONE,
                }],
            )
            .unwrap();
    }
    CountryState::new(controller, BTreeSet::new(), false, modifiers)
}

fn expired(country: CountryId, source: u32) -> StateChange {
    StateChange::ModifierSourceExpired {
        country,
        source: ModifierSourceId::new(source),
    }
}

#[test]
fn state_exposes_the_loaded_graph_and_its_identity_read_only() {
    let graph = map();
    let identity = graph.identity();
    let state = GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        42,
        graph,
        BTreeMap::new(),
        [],
    )
    .unwrap();

    assert_eq!(state.map_identity(), identity);
    assert_eq!(
        state.province_graph().neighbors(ProvinceId::new(1)),
        Some(
            [ProvinceEdge {
                to: ProvinceId::new(2),
                kind: EdgeKind::River,
                cost: 1,
            }]
            .as_slice()
        )
    );
    assert_eq!(state.province_graph().neighbors(ProvinceId::new(3)), None);
}

fn map() -> ProvinceGraph {
    ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [42; 32],
        },
        [province(2, "second", 1), province(1, "first", 2)],
    )
    .unwrap()
}

fn province(id: u32, key: &str, neighbor: u32) -> Province {
    Province {
        id: ProvinceId::new(id),
        key: key.to_owned(),
        kind: ProvinceKind::Land,
        terrain: "test_terrain".to_owned(),
        centroid: [0, 0],
        development: 0,
        neighbors: vec![ProvinceEdge {
            to: ProvinceId::new(neighbor),
            kind: EdgeKind::River,
            cost: 1,
        }],
        ports: Vec::new(),
    }
}

#[test]
fn scheduled_grants_activate_after_expiry_and_before_ai_in_stable_order() {
    let early = CountryId::new(2);
    let late = CountryId::new(9);
    let key = ModKey::new("test_stat");
    let mut state = state_with_schedule(
        true,
        BTreeMap::from([
            (early, country_with_modifiers(ai(), &key, &[4])),
            (late, empty_country(ai())),
        ]),
        [
            scheduled(1, late, 3, 300, Some(2)),
            scheduled(1, early, 4, 200, Some(1)),
            scheduled(1, early, 2, 100, None),
        ],
    )
    .unwrap();

    let before_pause = state.clone();
    assert!(state
        .tick(|_, _| panic!("paused tick dispatched AI"))
        .unwrap()
        .delta
        .changes()
        .is_empty());
    assert_eq!(state, before_pause);
    state
        .apply(HOST, &Command::Time(TimeCommand::SetPaused(false)))
        .unwrap();

    let report = state
        .tick(|snapshot, _| {
            for country in [early, late] {
                assert_eq!(
                    snapshot
                        .country(country)
                        .unwrap()
                        .effective_value(Fixed::ONE, &key),
                    Some(Fixed::from_raw(1_300))
                );
            }
            Vec::new()
        })
        .unwrap();
    assert_eq!(
        report.delta.changes(),
        &[
            StateChange::ClockAdvanced {
                tick: Tick::new(1),
                date: Date::new(1440, 1, 2).unwrap(),
            },
            expired(early, 4),
            activated(early, 2, 100, None),
            activated(early, 4, 200, Some(2)),
            activated(late, 3, 300, Some(3)),
        ]
    );
    assert!(report.scheduled_failures.is_empty());

    let second = state.tick(|_, _| Vec::new()).unwrap();
    assert_eq!(&second.delta.changes()[1..], &[expired(early, 4)]);
    let third = state.tick(|_, _| Vec::new()).unwrap();
    assert_eq!(&third.delta.changes()[1..], &[expired(late, 3)]);
    assert_eq!(
        state
            .country(early)
            .unwrap()
            .effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_100))
    );
}

#[test]
fn conflicting_scheduled_grant_leaves_existing_source_and_is_consumed() {
    let country = CountryId::new(2);
    let key = ModKey::new("test_stat");
    let source = ModifierSourceId::new(4);
    let mut modifiers = ModifierStore::default();
    modifiers
        .insert_source(
            source,
            None,
            vec![Modifier {
                source,
                key: key.clone(),
                kind: ModifierKind::Additive,
                value: Fixed::from_raw(100),
            }],
        )
        .unwrap();

    let mut state = state_with_schedule(
        false,
        BTreeMap::from([(
            country,
            CountryState::new(ai(), BTreeSet::new(), false, modifiers),
        )]),
        [
            scheduled(1, country, 4, 900, Some(1)),
            scheduled(1, country, 9, 200, None),
        ],
    )
    .unwrap();
    let report = state.tick(|_, _| Vec::new()).unwrap();

    assert_eq!(
        report.scheduled_failures,
        [ScheduledEffectFailure {
            tick: Tick::new(1),
            country,
            source,
            error: ModifierInsertError::DuplicateSource(source),
        }]
    );
    assert_eq!(
        &report.delta.changes()[1..],
        &[activated(country, 9, 200, None)]
    );
    let next = state.tick(|_, _| Vec::new()).unwrap();
    assert!(next.scheduled_failures.is_empty());
    assert_eq!(
        state
            .country(country)
            .unwrap()
            .effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_300))
    );
}

#[test]
fn constructor_rejects_invalid_scheduled_records() {
    let country = CountryId::new(2);
    let unknown = CountryId::new(7);
    let source = ModifierSourceId::new(4);
    let countries = BTreeMap::from([(country, empty_country(ai()))]);
    let valid = scheduled(1, country, 4, 100, None);

    assert_eq!(
        state_with_schedule(
            false,
            countries.clone(),
            [scheduled(0, country, 4, 100, None)]
        ),
        Err(ScheduleError::NotFuture {
            due: Tick::ZERO,
            current: Tick::ZERO,
        })
    );
    assert_eq!(
        state_with_schedule(
            false,
            countries.clone(),
            [scheduled(1, unknown, 4, 100, None)]
        ),
        Err(ScheduleError::UnknownCountry(unknown))
    );
    assert_eq!(
        state_with_schedule(false, countries.clone(), [valid.clone(), valid]),
        Err(ScheduleError::Duplicate {
            due: Tick::new(1),
            country,
            source
        })
    );
    assert_eq!(
        state_with_schedule(
            false,
            countries,
            [scheduled(1, country, 4, 100, Some(u64::MAX))]
        ),
        Err(ScheduleError::InvalidEffect {
            due: Tick::new(1),
            country,
            source,
            error: EffectError::ExpiryOverflow,
        })
    );
}

fn state_with_schedule(
    paused: bool,
    countries: BTreeMap<CountryId, CountryState>,
    schedule: impl IntoIterator<Item = ScheduledEffect>,
) -> Result<GameState, ScheduleError> {
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, paused),
        42,
        map(),
        countries,
        schedule,
    )
}

fn scheduled(
    due: u64,
    country: CountryId,
    source: u32,
    value: i64,
    days: Option<u64>,
) -> ScheduledEffect {
    ScheduledEffect {
        due: Tick::new(due),
        country,
        source: ModifierSourceId::new(source),
        effect: Effect::AddModifier {
            key: ModKey::new("test_stat"),
            value: Fixed::from_raw(value),
            kind: ModifierKind::Additive,
            duration_days: days.map(|days| NonZeroU64::new(days).unwrap()),
        },
    }
}

fn activated(country: CountryId, source: u32, value: i64, expires_at: Option<u64>) -> StateChange {
    StateChange::ModifierSourceActivated {
        country,
        grant: PreparedModifier {
            modifier: Modifier {
                source: ModifierSourceId::new(source),
                key: ModKey::new("test_stat"),
                value: Fixed::from_raw(value),
                kind: ModifierKind::Additive,
            },
            expires_at,
        },
    }
}
#[test]
fn constructor_rejects_an_already_expired_active_source() {
    let country = CountryId::new(2);
    let source = ModifierSourceId::new(4);
    let mut modifiers = ModifierStore::default();
    modifiers
        .insert_source(
            source,
            Some(0),
            vec![Modifier {
                source,
                key: ModKey::new("test_stat"),
                kind: ModifierKind::Additive,
                value: Fixed::ONE,
            }],
        )
        .unwrap();

    assert_eq!(
        state_with_schedule(
            false,
            BTreeMap::from([(
                country,
                CountryState::new(ai(), BTreeSet::new(), false, modifiers),
            )]),
            [],
        ),
        Err(ScheduleError::ExpiredActiveSource {
            country,
            source,
            expires_at: 0,
            current: Tick::ZERO,
        })
    );
}
