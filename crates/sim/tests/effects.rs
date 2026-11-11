use std::num::NonZeroU64;

use strategy_sim::{
    Effect, EffectError, Fixed, ModKey, ModifierKind, ModifierSourceId, ModifierStore,
};

#[test]
fn prepared_modifier_uses_the_source_and_expiry_tick() {
    let key = ModKey::new("army_morale");
    let source = ModifierSourceId::new(2);
    let effect = Effect::AddModifier {
        key: key.clone(),
        value: Fixed::from_raw(100),
        kind: ModifierKind::Additive,
        duration_days: NonZeroU64::new(3),
    };
    let prepared = effect.prepare(source, 5).unwrap();

    assert_eq!(prepared.modifier.source, source);
    assert_eq!(prepared.expires_at, Some(8));

    let mut store = ModifierStore::default();
    store
        .insert_source(source, prepared.expires_at, vec![prepared.modifier])
        .unwrap();
    assert!(store.expire_through(7).is_empty());
    assert_eq!(
        store.effective_value(Fixed::ONE, &key),
        Some(Fixed::from_raw(1_100))
    );
    assert_eq!(store.expire_through(8), vec![source]);
    assert_eq!(store.effective_value(Fixed::ONE, &key), Some(Fixed::ONE));
}

#[test]
fn rejects_invalid_expiry_and_multiplier() {
    let key = ModKey::new("army_morale");
    let source = ModifierSourceId::new(1);
    let timed = Effect::AddModifier {
        key: key.clone(),
        value: Fixed::ONE,
        kind: ModifierKind::Additive,
        duration_days: NonZeroU64::new(1),
    };
    assert_eq!(
        timed.prepare(source, u64::MAX),
        Err(EffectError::ExpiryOverflow)
    );

    let invalid = Effect::AddModifier {
        key: key.clone(),
        value: Fixed::from_raw(i64::MAX),
        kind: ModifierKind::Multiplicative,
        duration_days: None,
    };
    assert_eq!(
        invalid.prepare(source, 0),
        Err(EffectError::InvalidMultiplier)
    );

    let permanent = Effect::AddModifier {
        key,
        value: Fixed::ONE,
        kind: ModifierKind::Additive,
        duration_days: None,
    };
    assert_eq!(
        permanent.prepare(source, u64::MAX).unwrap().expires_at,
        None
    );
}
