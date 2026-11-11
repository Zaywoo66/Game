use strategy_sim::{
    Date, InvalidDate, InvalidSpeed, SimTime, Speed, Tick, TimeActor, TimeCommand, TimeError,
};

#[test]
fn adding_months_clamps_to_last_gregorian_day() {
    assert_eq!(
        Date::new(1440, 12, 31)
            .unwrap()
            .checked_add_months(2)
            .unwrap()
            .components(),
        (1441, 2, 28)
    );
    assert_eq!(
        Date::new(1443, 12, 31)
            .unwrap()
            .checked_add_months(2)
            .unwrap()
            .components(),
        (1444, 2, 29)
    );
    assert_eq!(
        Date::new(1440, 1, 30)
            .unwrap()
            .checked_add_months(2)
            .unwrap()
            .components(),
        (1440, 3, 30)
    );
}

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

#[test]
fn dates_reject_invalid_year_month_and_day() {
    assert_eq!(Date::new(0, 1, 1), Err(InvalidDate::Year));
    assert_eq!(Date::new(1440, 0, 1), Err(InvalidDate::Month));
    assert_eq!(Date::new(1440, 13, 1), Err(InvalidDate::Month));
    assert_eq!(Date::new(1440, 1, 0), Err(InvalidDate::Day));
    assert_eq!(Date::new(1440, 4, 31), Err(InvalidDate::Day));
    assert_eq!(Date::new(1500, 2, 29), Err(InvalidDate::Day));
}

#[test]
fn date_arithmetic_crosses_gregorian_month_year_and_cycle_boundaries() {
    for (year, month, day, expected) in [
        (1440, 2, 28, (1440, 2, 29)),
        (1440, 2, 29, (1440, 3, 1)),
        (1440, 4, 30, (1440, 5, 1)),
        (1440, 12, 31, (1441, 1, 1)),
        (1500, 2, 28, (1500, 3, 1)),
        (1600, 2, 28, (1600, 2, 29)),
        (1600, 12, 31, (1601, 1, 1)),
        (u32::MAX, 12, 30, (u32::MAX, 12, 31)),
    ] {
        let date = Date::new(year, month, day).unwrap();
        assert_eq!(date.components(), (year, month, day));
        assert_eq!(date.checked_add_days(1).unwrap().components(), expected);
    }

    let epoch = Date::new(1, 1, 1).unwrap();
    assert_eq!(
        epoch.checked_add_days(146_097 * 5).unwrap().components(),
        (2001, 1, 1)
    );
    assert_eq!(epoch.checked_add_days(u64::MAX), None);
}

#[test]
fn every_speed_advances_exactly_one_day_per_tick() {
    let start = Date::new(1440, 1, 1).unwrap();
    for level in 1..=5 {
        let speed = Speed::try_from(level).unwrap();
        let mut time = SimTime::new(start, speed, false);
        assert_eq!(time.tick(), Tick::ZERO);
        assert_eq!(time.date(), start);
        assert_eq!(time.advance(), Ok(true));
        assert_eq!(time.tick(), Tick::new(1));
        assert_eq!(time.date().components(), (1440, 1, 2));
    }
}

#[test]
fn both_players_can_pause_and_resume_without_resetting_speed() {
    let start = Date::new(1440, 1, 1).unwrap();
    let mut time = SimTime::new(start, Speed::One, false);
    assert_eq!(
        time.apply(TimeActor::Host, TimeCommand::SetSpeed(Speed::Four)),
        Ok(())
    );

    for (pauser, resumer) in [
        (TimeActor::Guest, TimeActor::Host),
        (TimeActor::Host, TimeActor::Guest),
    ] {
        let before_pause = time;
        assert_eq!(time.apply(pauser, TimeCommand::SetPaused(true)), Ok(()));
        assert!(time.is_paused());
        assert_eq!(time.advance(), Ok(false));
        assert_eq!(time.tick(), before_pause.tick());
        assert_eq!(time.date(), before_pause.date());
        assert_eq!(time.speed(), Speed::Four);

        assert_eq!(time.apply(resumer, TimeCommand::SetPaused(false)), Ok(()));
        assert!(!time.is_paused());
        assert_eq!(time.advance(), Ok(true));
        assert_eq!(time.tick(), before_pause.tick().checked_add(1).unwrap());
        assert_eq!(time.speed(), Speed::Four);
    }
}

#[test]
fn only_authorized_actors_can_change_time_controls() {
    let mut time = SimTime::new(Date::new(1440, 1, 1).unwrap(), Speed::One, false);
    let original = time;

    assert_eq!(
        time.apply(TimeActor::Guest, TimeCommand::SetSpeed(Speed::Five)),
        Err(TimeError::HostRequired)
    );
    assert_eq!(
        time.apply(TimeActor::Ai, TimeCommand::SetPaused(true)),
        Err(TimeError::HumanRequired)
    );
    assert_eq!(
        time.apply(TimeActor::Ai, TimeCommand::SetSpeed(Speed::Five)),
        Err(TimeError::HumanRequired)
    );
    assert_eq!(time, original);
}

#[test]
fn overflowing_the_date_leaves_simulation_time_unchanged() {
    let last_date = Date::new(u32::MAX, 12, 31).unwrap();
    let mut time = SimTime::new(last_date, Speed::One, false);
    let original = time;

    assert_eq!(last_date.checked_add_days(1), None);
    assert_eq!(time.advance(), Err(TimeError::DateOverflow));
    assert_eq!(time, original);
}
