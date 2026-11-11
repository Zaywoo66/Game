use std::{collections::BTreeMap, num::NonZeroU64};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use strategy_data::{load_game, parse_provinces, save_game, state_hash, SaveError};
use strategy_sim::{
    AiProfileKey, Command, CommandActor, Controller, CountryId, CountryState, Date, DiplomacyState,
    Effect, Fixed, GameState, ModKey, Modifier, ModifierInsertError, ModifierKind,
    ModifierSourceId, ModifierStore, Opinion, PlayerId, ProvinceGraph, ScheduleError,
    ScheduledEffect, SeededRng, SimTime, Speed, Tick, TimeCommand, TriggerView,
};

const MAP: &[u8] = include_bytes!("fixtures/provinces.json");
const COUNTRY: CountryId = CountryId::new(2);
const GUEST_COUNTRY: CountryId = CountryId::new(7);
const HOST: CommandActor = CommandActor::Host(PlayerId::new(1));
const GUEST: CommandActor = CommandActor::Guest(PlayerId::new(2));
const SEED: u64 = 123_456;
const CURRENT_RNG: u64 = 0xb305_d1f6_8b05_1e4c;

#[test]
fn alliance_save_v5_preserves_offers_cooldowns_and_opinions() {
    let state = setup()
        .with_opinions([(COUNTRY, GUEST_COUNTRY, Opinion::new(-25).unwrap())])
        .unwrap();
    let mut diplomacy = DiplomacyState::default();
    diplomacy.offers.insert(
        (COUNTRY, GUEST_COUNTRY),
        state.time().date().checked_add_days(30).unwrap(),
    );
    let state = state.with_diplomacy(diplomacy).unwrap();
    let encoded = save_game(&state).unwrap();
    let document: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(document["schema_version"], 5);
    let loaded = load_game(&encoded, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(loaded, state);
    assert_eq!(save_game(&loaded).unwrap(), encoded);
    assert_eq!(state_hash(&loaded).unwrap(), state_hash(&state).unwrap());
    let mut corrupt: Value = serde_json::from_slice(&encoded).unwrap();
    corrupt["state"]["diplomacy"]["offers"][0]["until"] = json!([1440, 1, 1]);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&corrupt).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::InvalidDiplomacy(_))
    ));

    let mut cooldown = DiplomacyState::default();
    cooldown.cooldowns.insert(
        (COUNTRY, GUEST_COUNTRY),
        state.time().date().checked_add_months(2).unwrap(),
    );
    let state = setup().with_diplomacy(cooldown).unwrap();
    let encoded = save_game(&state).unwrap();
    let loaded = load_game(&encoded, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(loaded, state);
}

#[test]
fn directional_opinions_round_trip_in_version_four_and_reject_invalid_records() {
    let state = setup()
        .with_opinions([
            (COUNTRY, GUEST_COUNTRY, Opinion::new(-35).unwrap()),
            (GUEST_COUNTRY, COUNTRY, Opinion::new(60).unwrap()),
        ])
        .unwrap();
    let encoded = save_game(&state).unwrap();
    let mut document: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(document["schema_version"], 4);
    let loaded = load_game(&encoded, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(loaded, state);
    assert_eq!(save_game(&loaded).unwrap(), encoded);
    assert_eq!(state_hash(&loaded).unwrap(), state_hash(&state).unwrap());

    document["state"]["opinions"][0]["points"] = json!(101);
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::InvalidOpinion(_))
    ));
    document["state"]["opinions"][0]["points"] = json!(-35);
    document["state"]["opinions"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert!(matches!(
        load_game(
            &serde_json::to_vec(&document).unwrap(),
            parse_provinces(MAP).unwrap()
        ),
        Err(SaveError::UnorderedRecords("opinions"))
    ));
}

fn setup() -> GameState {
    let permanent = ModifierSourceId::new(3);
    let mut modifiers = ModifierStore::default();
    modifiers
        .insert_source(
            permanent,
            None,
            vec![Modifier {
                source: permanent,
                key: ModKey::new("test_stat"),
                value: Fixed::from_raw(2_000),
                kind: ModifierKind::Additive,
            }],
        )
        .unwrap();
    let countries = BTreeMap::from([
        (
            GUEST_COUNTRY,
            CountryState::new(
                Controller::Human(PlayerId::new(2)),
                Default::default(),
                false,
                ModifierStore::default(),
            ),
        ),
        (
            COUNTRY,
            CountryState::new(
                Controller::Ai(AiProfileKey::new("test_profile")),
                ["test_tech".to_owned()].into_iter().collect(),
                true,
                modifiers,
            ),
        ),
    ]);
    let grant = |due, source, value, duration_days| ScheduledEffect {
        due: Tick::new(due),
        country: COUNTRY,
        source: ModifierSourceId::new(source),
        effect: Effect::AddModifier {
            key: ModKey::new("test_stat"),
            value: Fixed::from_raw(value),
            kind: ModifierKind::Additive,
            duration_days,
        },
    };

    GameState::restore(
        SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false),
        SEED,
        SeededRng::new(CURRENT_RNG),
        parse_provinces(MAP).unwrap(),
        countries,
        [
            grant(8_000, 9, 7_000, None),
            grant(4_900, 5, 11_000, NonZeroU64::new(300)),
        ],
    )
    .unwrap()
}

fn replay_until(state: &mut GameState, target: u64) {
    while state.time().tick().get() < target {
        match state.time().tick().get() {
            100 => {
                state
                    .apply(GUEST, &Command::Time(TimeCommand::SetPaused(true)))
                    .unwrap();
                assert!(state
                    .tick(|_, _| panic!("paused tick called AI"))
                    .unwrap()
                    .delta
                    .changes()
                    .is_empty());
                state
                    .apply(HOST, &Command::Time(TimeCommand::SetPaused(false)))
                    .unwrap();
            }
            1_200 => {
                state
                    .apply(HOST, &Command::Time(TimeCommand::SetSpeed(Speed::Four)))
                    .unwrap();
            }
            6_000 => {
                state
                    .apply(
                        HOST,
                        &Command::SetController {
                            country: GUEST_COUNTRY,
                            controller: Controller::Ai(AiProfileKey::new("second_profile")),
                        },
                    )
                    .unwrap();
            }
            _ => {}
        }
        state.tick(|_, _| Vec::new()).unwrap();
    }
}

#[test]
fn ten_thousand_ticks_replay_identically_after_save_and_resume() {
    let mut uninterrupted = setup();
    replay_until(&mut uninterrupted, 10_000);
    let mut replay = setup();
    replay_until(&mut replay, 5_000);

    let key = ModKey::new("test_stat");
    assert_eq!(
        replay
            .country(COUNTRY)
            .unwrap()
            .effective_value(Fixed::ZERO, &key),
        Some(Fixed::from_raw(13_000))
    );
    assert_eq!(replay.pending_effects().count(), 1);
    let encoded = save_game(&replay).unwrap();
    let mut resumed = load_game(&encoded, parse_provinces(MAP).unwrap()).unwrap();
    assert_eq!(resumed, replay);
    assert_eq!(save_game(&resumed).unwrap(), encoded);

    assert_ne!(resumed.rng_state(), resumed.seed());
    assert_eq!(resumed.rng_state(), CURRENT_RNG);
    assert_eq!(resumed.seed(), SEED);
    assert_eq!(
        SeededRng::new(resumed.rng_state()).next_u64(),
        SeededRng::new(CURRENT_RNG).next_u64()
    );

    replay_until(&mut replay, 10_000);
    replay_until(&mut resumed, 10_000);

    let expected_hash = state_hash(&uninterrupted).unwrap();
    assert_eq!(replay.time().tick(), Tick::new(10_000));
    assert_eq!(replay.time().speed(), Speed::Four);
    assert_eq!(state_hash(&replay).unwrap(), expected_hash);
    assert_eq!(state_hash(&resumed).unwrap(), expected_hash);
    assert_eq!(
        save_game(&resumed).unwrap(),
        save_game(&uninterrupted).unwrap()
    );
    let country = resumed.country(COUNTRY).unwrap();
    assert!(country.has_technology("test_tech"));
    assert!(country.is_at_war());
    assert_eq!(
        country.effective_value(Fixed::ZERO, &key),
        Some(Fixed::from_raw(9_000))
    );
    assert_eq!(resumed.pending_effects().count(), 0);
    assert_eq!(
        resumed.country(GUEST_COUNTRY).unwrap().controller(),
        &Controller::Ai(AiProfileKey::new("second_profile"))
    );
}

fn saved() -> Value {
    serde_json::from_slice(&save_game(&setup()).unwrap()).unwrap()
}

fn from_value(value: &Value, graph: ProvinceGraph) -> Result<GameState, SaveError> {
    load_game(&serde_json::to_vec(value).unwrap(), graph)
}

#[test]
fn map_version_and_hash_are_checked_before_decoding_state() {
    let original = parse_provinces(MAP).unwrap();
    let mut new_version: Value = serde_json::from_slice(MAP).unwrap();
    new_version["map_version"] = json!(5);
    let different_version = parse_provinces(&serde_json::to_vec(&new_version).unwrap()).unwrap();
    assert_eq!(
        original.identity().sha256,
        different_version.identity().sha256
    );

    let provinces = r#"[{"id":1,"key":"other_land","kind":"land","terrain":"grassland","centroid":[0,0],"neighbors":[],"ports":[],"development":5}]"#;
    let hash = format!("{:x}", Sha256::digest(provinces.as_bytes()));
    let different_map = format!(
        r#"{{"schema_version":1,"map_version":4,"map_hash":"{hash}","provinces":{provinces}}}"#
    );
    let different_hash = parse_provinces(different_map.as_bytes()).unwrap();
    assert_eq!(
        original.identity().version,
        different_hash.identity().version
    );
    assert_ne!(original.identity().sha256, different_hash.identity().sha256);

    let mut save = saved();
    save["state"] = json!("malformed state");
    for graph in [different_version, different_hash] {
        let identity = graph.identity();
        let error = from_value(&save, graph).unwrap_err();
        assert!(matches!(
            error,
            SaveError::MapMismatch(mismatch)
                if mismatch.loaded == identity && mismatch.saved == original.identity()
        ));
    }
    assert!(matches!(
        from_value(&save, original),
        Err(SaveError::Json(_))
    ));
}

#[test]
fn malformed_records_are_rejected() {
    let graph = || parse_provinces(MAP).unwrap();
    let mut save = saved();
    save["schema_version"] = json!(8);
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::UnsupportedSchemaVersion(8))
    ));

    let mut save = saved();
    save["map_hash"] = json!("invalid");
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::InvalidMapHash)
    ));

    let mut save = saved();
    let countries = save["state"]["countries"].as_array_mut().unwrap();
    countries.push(countries[0].clone());
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::UnorderedRecords("countries"))
    ));

    let mut save = saved();
    save["state"]["scheduled"][0]["country"] = json!(99);
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::InvalidSchedule(ScheduleError::UnknownCountry(id))) if id == CountryId::new(99)
    ));

    let mut save = saved();
    save["state"]["scheduled"][0]["expires_at"] = json!(4_900);
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::InvalidScheduledExpiry { .. })
    ));

    let mut save = saved();
    save["state"]["tick"] = json!(u64::MAX);
    assert!(matches!(
        from_value(&save, graph()),
        Err(SaveError::InvalidClock(_))
    ));
}

#[test]
fn reordered_fields_and_whitespace_do_not_change_canonical_hash() {
    let mut state = setup();
    state
        .apply(GUEST, &Command::Time(TimeCommand::SetPaused(true)))
        .unwrap();
    let document: Value = serde_json::from_slice(&save_game(&state).unwrap()).unwrap();
    let pretty = serde_json::to_vec_pretty(&document).unwrap();
    let loaded = load_game(&pretty, parse_provinces(MAP).unwrap()).unwrap();
    assert!(loaded.time().is_paused());
    assert_eq!(save_game(&loaded).unwrap(), save_game(&state).unwrap());
    assert_eq!(state_hash(&loaded).unwrap(), state_hash(&state).unwrap());
}

#[test]
fn golden_v1_save_advances_to_the_expected_state_hash() {
    let mut state = load_game(
        include_bytes!("fixtures/save_v1.json"),
        parse_provinces(MAP).unwrap(),
    )
    .unwrap();
    state.tick(|_, _| Vec::new()).unwrap();
    assert_eq!(state.time().tick(), Tick::new(1));
    let digest: String = state_hash(&state)
        .unwrap()
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        digest,
        "0ef49193f46112406e7555cf8e7d34f62b77da2ecc591b69b1a804b24a9b4512"
    );
}

#[test]
fn rejects_saved_active_sources_already_due_or_expired() {
    let mut save = saved();
    save["state"]["tick"] = json!(1);
    save["state"]["paused"] = json!(true);
    for expires_at in [0, 1] {
        save["state"]["countries"][0]["sources"][0]["expires_at"] = json!(expires_at);
        assert!(matches!(
            from_value(&save, parse_provinces(MAP).unwrap()),
            Err(SaveError::InvalidSchedule(ScheduleError::ExpiredActiveSource {
                country: COUNTRY,
                source,
                expires_at: expiry,
                current,
            })) if source == ModifierSourceId::new(3) && expiry == expires_at && current == Tick::new(1)
        ));
    }
}

#[test]
fn rejects_saved_active_multipliers_whose_factor_overflows() {
    let mut save = saved();
    let modifier = &mut save["state"]["countries"][0]["sources"][0]["modifiers"][0];
    modifier["kind"] = json!("multiplicative");
    modifier["value"] = json!(i64::MAX);

    assert!(matches!(
        from_value(&save, parse_provinces(MAP).unwrap()),
        Err(SaveError::InvalidModifier(ModifierInsertError::InvalidMultiplier {
            source,
            index: 0,
        })) if source == ModifierSourceId::new(3)
    ));
}
