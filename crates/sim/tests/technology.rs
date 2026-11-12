use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use strategy_sim::{
    Command, CommandActor, CommandError, Controller, CountryId, CountryState, Date, Effect, Fixed,
    GameState, MapIdentity, ModKey, ModifierKind, ModifierStore, PlayerId, ProvinceGraph, SimTime,
    Speed, StateChange, TechnologyDefinition, TechnologyError,
};

const COUNTRY: CountryId = CountryId::new(1);
const HUMAN: CommandActor = CommandActor::Host(PlayerId::new(1));

fn game() -> GameState {
    let graph = ProvinceGraph::try_new(
        MapIdentity {
            version: NonZeroU32::new(1).unwrap(),
            sha256: [7; 32],
        },
        [],
    )
    .unwrap();
    let state = GameState::new(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        17,
        graph,
        BTreeMap::from([(
            COUNTRY,
            CountryState::new(
                Controller::Human(PlayerId::new(1)),
                BTreeSet::new(),
                false,
                ModifierStore::default(),
            ),
        )]),
        [],
    )
    .unwrap();
    let tech = |key: &str, requires: Vec<String>, cost| TechnologyDefinition {
        key: key.into(),
        branch: "administration".into(),
        cost: NonZeroU32::new(cost).unwrap(),
        requires,
        effects: vec![Effect::AddModifier {
            key: ModKey::new("tax_income"),
            value: Fixed::from_raw(100),
            kind: ModifierKind::Multiplicative,
            duration_days: None,
        }],
    };
    state
        .with_technologies(BTreeMap::from([
            ("first".into(), tech("first", vec![], 10)),
            ("second".into(), tech("second", vec!["first".into()], 10)),
        ]))
        .unwrap()
}

#[test]
fn monthly_points_purchase_and_prerequisites_are_deterministic() {
    let mut a = game();
    let mut b = game();
    let first = Command::UnlockTechnology {
        country: COUNTRY,
        key: "first".into(),
    };
    let second = Command::UnlockTechnology {
        country: COUNTRY,
        key: "second".into(),
    };
    assert_eq!(
        a.validate_command(HUMAN, &second),
        Err(CommandError::Technology(
            TechnologyError::MissingPrerequisite("first".into())
        ))
    );
    assert_eq!(
        a.validate_command(HUMAN, &first),
        Err(CommandError::Technology(
            TechnologyError::InsufficientPoints {
                required: 10,
                available: 0
            }
        ))
    );
    for _ in 0..31 {
        assert_eq!(
            a.tick(|_, _| vec![]).unwrap(),
            b.tick(|_, _| vec![]).unwrap()
        );
    }
    assert_eq!(a.time().date().components(), (1440, 2, 1));
    assert_eq!(a.research_points(COUNTRY), Some(10));
    assert_eq!(a.apply(HUMAN, &first), b.apply(HUMAN, &first));
    assert_eq!(a.research_points(COUNTRY), Some(0));
    assert_eq!(
        a.apply(HUMAN, &first),
        Err(CommandError::Technology(TechnologyError::AlreadyKnown(
            "first".into()
        )))
    );
    for _ in 0..29 {
        a.tick(|_, _| vec![]).unwrap();
        b.tick(|_, _| vec![]).unwrap();
    }
    assert_eq!(a.research_points(COUNTRY), Some(10));
    let delta = a.apply(HUMAN, &second).unwrap();
    b.apply(HUMAN, &second).unwrap();
    assert!(delta.changes().iter().any(
        |change| matches!(change, StateChange::TechnologyUnlocked { key, .. } if key == "second")
    ));
    assert_eq!(a, b);
    assert_eq!(
        a.country(COUNTRY)
            .unwrap()
            .known_technologies()
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(a.country(COUNTRY).unwrap().modifiers().sources().count(), 2);
}

#[test]
fn research_points_cap_and_command_authorization() {
    let mut state = game();
    let command = Command::UnlockTechnology {
        country: COUNTRY,
        key: "first".into(),
    };
    assert_eq!(
        state.validate_command(CommandActor::Guest(PlayerId::new(2)), &command),
        Err(CommandError::NotCountryController(COUNTRY))
    );
    for _ in 0..(365 * 10) {
        state.tick(|_, _| vec![]).unwrap();
    }
    assert_eq!(state.research_points(COUNTRY), Some(1000));
}
