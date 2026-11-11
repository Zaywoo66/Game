use strategy_sim::{effective_value, Fixed, ModKey, Modifier, ModifierKind, ModifierSourceId};

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
