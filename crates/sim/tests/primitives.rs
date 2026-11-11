use strategy_sim::{derive_ai_rng, CountryId, Fixed, SeededRng};

#[test]
fn fixed_arithmetic_preserves_scale_and_handles_negative_values() {
    let positive = Fixed::from_raw(2_500);
    let negative = Fixed::from_integer(-2).unwrap();

    assert_eq!(positive.checked_add(negative), Some(Fixed::from_raw(500)));
    assert_eq!(positive.checked_sub(negative), Some(Fixed::from_raw(4_500)));
    assert_eq!(positive.checked_mul(negative), Fixed::from_integer(-5));
    assert_eq!(
        positive.checked_div(negative),
        Some(Fixed::from_raw(-1_250))
    );
}

#[test]
fn fixed_arithmetic_detects_overflow_and_zero_divisors() {
    let max = Fixed::from_raw(i64::MAX);
    let min = Fixed::from_raw(i64::MIN);
    let unit = Fixed::from_raw(1);
    let two = Fixed::from_integer(2).unwrap();
    let negative_one = Fixed::from_integer(-1).unwrap();

    assert_eq!(Fixed::from_integer(i64::MAX), None);
    assert_eq!(max.checked_add(unit), None);
    assert_eq!(min.checked_sub(unit), None);
    assert_eq!(max.checked_mul(two), None);
    assert_eq!(min.checked_div(negative_one), None);
    assert_eq!(max.checked_div(Fixed::ZERO), None);

    // Both operations require an intermediate larger than the storage type.
    assert_eq!(max.checked_mul(Fixed::ONE), Some(max));
    assert_eq!(min.checked_div(Fixed::ONE), Some(min));
}

#[test]
fn fixed_mul_and_div_truncate_toward_zero() {
    let half = Fixed::from_raw(500);
    let three = Fixed::from_integer(3).unwrap();

    assert_eq!(
        Fixed::from_raw(3).checked_mul(half),
        Some(Fixed::from_raw(1))
    );
    assert_eq!(
        Fixed::from_raw(-3).checked_mul(half),
        Some(Fixed::from_raw(-1))
    );
    assert_eq!(Fixed::ONE.checked_div(three), Some(Fixed::from_raw(333)));
    assert_eq!(
        Fixed::from_integer(-1).unwrap().checked_div(three),
        Some(Fixed::from_raw(-333))
    );
}

#[test]
fn seeded_rng_replays_the_same_sequence() {
    let mut first = SeededRng::new(42);
    let mut second = SeededRng::new(42);
    assert_eq!(first.next_u64(), 0xbdd7_3226_2feb_6e95);
    assert_eq!(second.next_u64(), 0xbdd7_3226_2feb_6e95);
    assert_eq!(draws(first), draws(second));
}

#[test]
fn ai_rng_is_reproducible_and_varies_by_tick_and_country() {
    let country = CountryId::new(4);
    let stream = draws(derive_ai_rng(42, 10, country));

    assert_ne!(stream, draws(derive_ai_rng(43, 10, country)));
    assert_ne!(stream, draws(derive_ai_rng(42, 11, country)));
    assert_ne!(stream, draws(derive_ai_rng(42, 10, CountryId::new(5))));
    assert_eq!(stream, draws(derive_ai_rng(42, 10, country)));
}

fn draws(mut rng: SeededRng) -> [u64; 8] {
    std::array::from_fn(|_| rng.next_u64())
}
