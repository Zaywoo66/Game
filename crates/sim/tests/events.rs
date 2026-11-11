use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU32, NonZeroU64};

use strategy_sim::{
    event_rng, AiProfileKey, Command, CommandActor, CommandError, ConditionalAiWeight, Controller,
    CountryId, CountryState, Date, Effect, EventDefinition, EventError, EventOption, Fixed,
    GameState, MapIdentity, ModKey, ModifierKind, ModifierStore, PlayerId, ProvinceGraph, SimTime,
    Speed, StateChange, Trigger,
};

const COUNTRY: CountryId = CountryId::new(1);
const HUMAN: CommandActor = CommandActor::Host(PlayerId::new(1));

fn game(controller: Controller, event: EventDefinition) -> GameState {
    let graph = ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [7; 32],
        },
        [],
    )
    .unwrap();
    GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        1234,
        graph,
        BTreeMap::from([(
            COUNTRY,
            CountryState::new(controller, BTreeSet::new(), false, ModifierStore::default()),
        )]),
        [],
    )
    .unwrap()
    .with_events(BTreeMap::from([(event.key.clone(), event)]))
    .unwrap()
}

#[test]
fn keyed_event_rng_replays_and_separates_seed_tick_country_and_key() {
    let draw =
        |seed, tick, country, key| event_rng(seed, tick, CountryId::new(country), key).next_u64();
    let baseline = draw(7, 40, 1, "harvest");
    assert_eq!(baseline, draw(7, 40, 1, "harvest"));
    assert_ne!(baseline, draw(8, 40, 1, "harvest"));
    assert_ne!(baseline, draw(7, 41, 1, "harvest"));
    assert_ne!(baseline, draw(7, 40, 2, "harvest"));
    assert_ne!(baseline, draw(7, 40, 1, "drought"));
}

fn event(repeatable: bool, mtth: u64) -> EventDefinition {
    EventDefinition {
        key: "harvest".into(),
        title_key: "harvest_title".into(),
        desc_key: "harvest_desc".into(),
        trigger: Trigger::AtWar(false),
        mean_time_to_happen: NonZeroU64::new(mtth),
        repeatable,
        cooldown_days: repeatable.then(|| NonZeroU64::new(2).unwrap()),
        options: vec![
            EventOption {
                text_key: "harvest_accept".into(),
                effects: vec![Effect::AddModifier {
                    key: ModKey::new("harvest_tax"),
                    value: Fixed::from_raw(100),
                    kind: ModifierKind::Multiplicative,
                    duration_days: NonZeroU64::new(3),
                }],
                ai_weight: 10,
                ai_weight_if: Some(ConditionalAiWeight {
                    when: Trigger::AtWar(false),
                    add: -20,
                }),
            },
            EventOption {
                text_key: "harvest_decline".into(),
                effects: Vec::new(),
                ai_weight: 1,
                ai_weight_if: None,
            },
        ],
    }
}

#[test]
fn daily_fire_choose_and_repeat_cooldown_are_deterministic() {
    let mut first = game(Controller::Human(PlayerId::new(1)), event(true, 1));
    let mut second = first.clone();
    for state in [&mut first, &mut second] {
        let fired = state.tick(|_, _| Vec::new()).unwrap();
        assert!(fired.delta.changes().iter().any(|change| matches!(change, StateChange::EventFired { country: COUNTRY, key } if key == "harvest")));
        assert_eq!(state.events().unwrap().pending.len(), 1);
        state.tick(|_, _| Vec::new()).unwrap();
        assert_eq!(state.events().unwrap().pending.len(), 1);
        let command = Command::ChooseEvent {
            country: COUNTRY,
            key: "harvest".into(),
            option: 0,
        };
        let delta = state.apply(HUMAN, &command).unwrap();
        assert!(delta
            .changes()
            .iter()
            .any(|change| matches!(change, StateChange::EventResolved { option: 0, .. })));
        assert_eq!(
            state
                .country(COUNTRY)
                .unwrap()
                .modifiers()
                .sources()
                .count(),
            1
        );
        state.tick(|_, _| Vec::new()).unwrap();
        assert!(state.events().unwrap().pending.is_empty());
        state.tick(|_, _| Vec::new()).unwrap();
        assert_eq!(state.events().unwrap().pending.len(), 1);
    }
    assert_eq!(first, second);
}

#[test]
fn one_shot_choice_cannot_repeat_and_unauthorized_choice_is_atomic() {
    let mut state = game(Controller::Human(PlayerId::new(1)), event(false, 1));
    state.tick(|_, _| Vec::new()).unwrap();
    let command = Command::ChooseEvent {
        country: COUNTRY,
        key: "harvest".into(),
        option: 0,
    };
    let before = state.clone();
    assert_eq!(
        state.apply(CommandActor::Guest(PlayerId::new(2)), &command),
        Err(CommandError::NotCountryController(COUNTRY))
    );
    assert_eq!(state, before);
    let invalid = Command::ChooseEvent {
        country: COUNTRY,
        key: "harvest".into(),
        option: 99,
    };
    assert_eq!(
        state.apply(HUMAN, &invalid),
        Err(CommandError::Event(EventError::InvalidOption(99)))
    );
    assert_eq!(state, before);
    state.apply(HUMAN, &command).unwrap();
    for _ in 0..10 {
        state.tick(|_, _| Vec::new()).unwrap();
    }
    assert!(state.events().unwrap().pending.is_empty());
    assert_eq!(
        state.apply(HUMAN, &command),
        Err(CommandError::Event(EventError::NotPending(
            "harvest".into()
        )))
    );
}

#[test]
fn conditional_ai_weights_clamp_then_fallback_to_base() {
    let state = game(Controller::Ai(AiProfileKey::new("test")), event(false, 1));
    let mut events = state.events().unwrap().clone();
    events
        .pending
        .insert((COUNTRY, "harvest".into()), state.time().tick());
    let choice = events.ai_option(
        COUNTRY,
        "harvest",
        state.country(COUNTRY).unwrap(),
        state.seed(),
        state.time().tick(),
    );
    assert_eq!(choice, Some(1));
    events.definitions.get_mut("harvest").unwrap().options[1].ai_weight_if =
        Some(ConditionalAiWeight {
            when: Trigger::AtWar(false),
            add: -10,
        });
    let choice = events.ai_option(
        COUNTRY,
        "harvest",
        state.country(COUNTRY).unwrap(),
        state.seed(),
        state.time().tick(),
    );
    assert!(matches!(choice, Some(0 | 1)));
    assert_eq!(
        choice,
        events.ai_option(
            COUNTRY,
            "harvest",
            state.country(COUNTRY).unwrap(),
            state.seed(),
            state.time().tick()
        )
    );
}
