use strategy_sim::{InvalidSpeed, Speed, Tick};

#[test]
fn tick_counts_elapsed_game_days_without_wrapping() {
    assert_eq!(Tick::default(), Tick::ZERO);
    assert_eq!(Tick::ZERO.get(), 0);
    assert_eq!(Tick::ZERO.checked_add(1), Some(Tick::new(1)));
    assert_eq!(Tick::new(12).checked_add(3), Some(Tick::new(15)));
    assert_eq!(
        Tick::new(u64::MAX - 1).checked_add(1),
        Some(Tick::new(u64::MAX))
    );
    assert_eq!(Tick::new(u64::MAX).checked_add(1), None);
}

#[test]
fn speed_accepts_exactly_five_levels() {
    for (level, speed) in [
        (1, Speed::One),
        (2, Speed::Two),
        (3, Speed::Three),
        (4, Speed::Four),
        (5, Speed::Five),
    ] {
        assert_eq!(Speed::try_from(level), Ok(speed));
        assert_eq!(speed.get(), level);
    }

    assert_eq!(Speed::try_from(0), Err(InvalidSpeed(0)));
    assert_eq!(Speed::try_from(6), Err(InvalidSpeed(6)));
    assert_eq!(Speed::try_from(u8::MAX), Err(InvalidSpeed(u8::MAX)));
}
