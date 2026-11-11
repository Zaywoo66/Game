use strategy_sim::{
    effective_value, Fixed, ModKey, Modifier, ModifierInsertError, ModifierKind, ModifierSourceId,
    ModifierStore,
};

#[test]
fn aggregates_additions_before_multiplicative_factors_for_the_requested_key() {
    let key = ModKey::new("test_stat");
    let unrelated = ModKey::new("other_stat");
    let modifiers = [
        modifier(&key, 4, ModifierKind::Multiplicative, -250),
        modifier(&key, 2, ModifierKind::Additive, -1_000),
        modifier(&unrelated, 1, ModifierKind::Additive, 50_000),
        modifier(&key, 1, ModifierKind::Additive, 2_000),
        modifier(&key, 3, ModifierKind::Multiplicative, 100),
    ];

    let base = Fixed::from_raw(10_000);
    assert_eq!(effective_value(base, &key, &[]), Some(base));
    assert_eq!(
        effective_value(base, &key, &modifiers),
        Some(Fixed::from_raw(9_075))
    );
}

#[test]
fn multiplier_rounding_is_independent_of_input_order_even_for_one_source() {
    let key = ModKey::new("test_stat");
    let a = modifier(&key, 1, ModifierKind::Multiplicative, -499);
    let b = modifier(&key, 2, ModifierKind::Multiplicative, -498);
    let c = modifier(&key, 2, ModifierKind::Multiplicative, 999);
    let expected = Some(Fixed::from_raw(501));

    // The factors 0.501, 0.502, and 1.999 give a different result when
    // folded as 0.501, 1.999, 0.502, because each multiplication truncates.
    assert_eq!(
        effective_value(Fixed::ONE, &key, &[a.clone(), b.clone(), c.clone()]),
        expected
    );
    assert_eq!(effective_value(Fixed::ONE, &key, &[c, a, b]), expected);
}

#[test]
fn additive_cancellation_can_fit_even_when_an_intermediate_sum_does_not() {
    let key = ModKey::new("test_stat");
    let modifiers = [
        modifier(&key, 1, ModifierKind::Additive, 1),
        modifier(&key, 2, ModifierKind::Additive, -2),
    ];

    assert_eq!(
        effective_value(Fixed::from_raw(i64::MAX), &key, &modifiers),
        Some(Fixed::from_raw(i64::MAX - 1))
    );
}

#[test]
fn returns_none_for_unrepresentable_intermediate_or_final_values() {
    let key = ModKey::new("test_stat");
    let max = Fixed::from_raw(i64::MAX);
    let positive_add = modifier(&key, 1, ModifierKind::Additive, 1);
    let overflow_factor = modifier(&key, 1, ModifierKind::Multiplicative, i64::MAX);
    let largest_factor = modifier(
        &key,
        1,
        ModifierKind::Multiplicative,
        i64::MAX - Fixed::SCALE,
    );
    let double = modifier(&key, 2, ModifierKind::Multiplicative, Fixed::SCALE);

    assert_eq!(effective_value(max, &key, &[positive_add]), None);
    assert_eq!(effective_value(Fixed::ONE, &key, &[overflow_factor]), None);
    assert_eq!(
        effective_value(max, &key, std::slice::from_ref(&double)),
        None
    );
    assert_eq!(
        effective_value(Fixed::ONE, &key, &[largest_factor, double]),
        None
    );
}

fn modifier(key: &ModKey, source: u32, kind: ModifierKind, raw: i64) -> Modifier {
    Modifier {
        source: ModifierSourceId::new(source),
        key: key.clone(),
        value: Fixed::from_raw(raw),
        kind,
    }
}

#[test]
fn expires_sources_at_their_exact_tick_and_keeps_permanent_sources() {
    let key = ModKey::new("test_stat");
    let mut store = ModifierStore::default();
    store
        .insert_source(
            ModifierSourceId::new(9),
            Some(12),
            vec![modifier(&key, 9, ModifierKind::Additive, 400)],
        )
        .unwrap();
    store
        .insert_source(
            ModifierSourceId::new(4),
            Some(12),
            vec![modifier(&key, 4, ModifierKind::Additive, 300)],
        )
        .unwrap();
    store
        .insert_source(
            ModifierSourceId::new(5),
            None,
            vec![modifier(&key, 5, ModifierKind::Additive, 200)],
        )
        .unwrap();

    assert!(store.expire_through(11).is_empty());
    assert_eq!(
        store.effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_900))
    );
    assert_eq!(
        store.expire_through(12),
        [ModifierSourceId::new(4), ModifierSourceId::new(9)]
    );
    assert!(store.expire_through(u64::MAX).is_empty());
    assert_eq!(
        store.effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_200))
    );
}

#[test]
fn removes_every_modifier_in_one_source_and_preserves_other_sources() {
    let attack = ModKey::new("attack");
    let movement = ModKey::new("movement");
    let removable = ModifierSourceId::new(2);
    let mut store = ModifierStore::default();
    store
        .insert_source(
            removable,
            Some(20),
            vec![
                modifier(&attack, 2, ModifierKind::Additive, 200),
                modifier(&movement, 2, ModifierKind::Multiplicative, 500),
            ],
        )
        .unwrap();
    store
        .insert_source(
            ModifierSourceId::new(3),
            None,
            vec![modifier(&attack, 3, ModifierKind::Additive, 500)],
        )
        .unwrap();

    assert_eq!(
        store.effective_value(Fixed::ONE, &attack),
        Some(Fixed::from_raw(1_700))
    );
    assert_eq!(
        store.effective_value(Fixed::ONE, &movement),
        Some(Fixed::from_raw(1_500))
    );
    assert!(store.remove_source(removable));
    assert!(!store.remove_source(removable));
    assert_eq!(
        store.effective_value(Fixed::ONE, &attack),
        Some(Fixed::from_raw(1_500))
    );
    assert_eq!(
        store.effective_value(Fixed::ONE, &movement),
        Some(Fixed::ONE)
    );
    assert!(store.expire_through(20).is_empty());
}

#[test]
fn rejects_duplicate_active_sources_without_changing_their_expiry_or_values() {
    let key = ModKey::new("test_stat");
    let id = ModifierSourceId::new(7);
    let mut store = ModifierStore::default();
    store
        .insert_source(
            id,
            Some(20),
            vec![modifier(&key, 7, ModifierKind::Additive, 100)],
        )
        .unwrap();
    let previous = store.clone();

    assert_eq!(
        store.insert_source(
            id,
            Some(10),
            vec![modifier(&key, 7, ModifierKind::Additive, 900)]
        ),
        Err(ModifierInsertError::DuplicateSource(id))
    );
    assert_eq!(store, previous);
    assert!(store.expire_through(10).is_empty());
    assert_eq!(store.expire_through(21), [id]);

    store
        .insert_source(
            id,
            None,
            vec![modifier(&key, 7, ModifierKind::Additive, 900)],
        )
        .unwrap();
    assert_eq!(
        store.effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_900))
    );
}

#[test]
fn rejects_a_bundle_whose_modifiers_name_a_different_source() {
    let key = ModKey::new("test_stat");
    let expected = ModifierSourceId::new(7);
    let actual = ModifierSourceId::new(8);
    let mut store = ModifierStore::default();

    assert_eq!(
        store.insert_source(
            expected,
            None,
            vec![
                modifier(&key, 7, ModifierKind::Additive, 200),
                modifier(&key, 8, ModifierKind::Additive, 100)
            ]
        ),
        Err(ModifierInsertError::SourceMismatch { expected, actual })
    );
    assert_eq!(store, ModifierStore::default());
}
#[test]
fn rejects_an_unrepresentable_multiplier_without_inserting_its_bundle() {
    let key = ModKey::new("test_stat");
    let source = ModifierSourceId::new(7);
    let mut store = ModifierStore::default();
    store
        .insert_source(
            ModifierSourceId::new(3),
            None,
            vec![modifier(
                &key,
                3,
                ModifierKind::Multiplicative,
                i64::MAX - Fixed::SCALE,
            )],
        )
        .unwrap();
    assert_eq!(
        store.effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(i64::MAX))
    );

    let previous = store.clone();
    assert_eq!(
        store.insert_source(
            source,
            None,
            vec![
                modifier(&key, 7, ModifierKind::Additive, 100),
                modifier(&key, 7, ModifierKind::Multiplicative, i64::MAX),
            ]
        ),
        Err(ModifierInsertError::InvalidMultiplier { source, index: 1 })
    );
    assert_eq!(store, previous);
}
