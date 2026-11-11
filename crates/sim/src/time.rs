/// Number of game days elapsed since the scenario started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    pub const fn new(days: u64) -> Self {
        Self(days)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the tick after the given number of game days, if it fits.
    pub fn checked_add(self, days: u64) -> Option<Self> {
        self.0.checked_add(days).map(Self)
    }
}

/// The host's selected pace. Each simulation tick still represents one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Speed {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
}

impl Speed {
    pub const fn get(self) -> u8 {
        self as u8
    }
}

/// A speed level outside the allowed range of one through five.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSpeed(pub u8);

impl TryFrom<u8> for Speed {
    type Error = InvalidSpeed;

    fn try_from(level: u8) -> Result<Self, Self::Error> {
        match level {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            5 => Ok(Self::Five),
            _ => Err(InvalidSpeed(level)),
        }
    }
}

const DAYS_IN_GREGORIAN_CYCLE: u64 = 146_097;
const LAST_DAY: u64 = days_before_year(u32::MAX as u64 + 1) - 1;

/// A proleptic Gregorian date. Supported years are 1 through `u32::MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidDate {
    Year,
    Month,
    Day,
}

impl Date {
    pub fn new(year: u32, month: u8, day: u8) -> Result<Self, InvalidDate> {
        if year == 0 {
            return Err(InvalidDate::Year);
        }
        if !(1..=12).contains(&month) {
            return Err(InvalidDate::Month);
        }
        if !(1..=days_in_month(year, month)).contains(&day) {
            return Err(InvalidDate::Day);
        }

        let preceding_months: u64 = (1..month)
            .map(|month| u64::from(days_in_month(year, month)))
            .sum();
        Ok(Self(
            days_before_year(u64::from(year)) + preceding_months + u64::from(day - 1),
        ))
    }

    /// Returns (year, month, day). Months and days start at one.
    pub fn components(self) -> (u32, u8, u8) {
        let cycles = self.0 / DAYS_IN_GREGORIAN_CYCLE;
        let day_in_cycle = self.0 % DAYS_IN_GREGORIAN_CYCLE;

        // Find the last year that begins on or before the date, within one
        // 400-year cycle. The exclusive upper bound starts the next cycle.
        let (mut first, mut end) = (1, 401);
        while first + 1 < end {
            let middle = first + (end - first) / 2;
            if days_before_year(middle) <= day_in_cycle {
                first = middle;
            } else {
                end = middle;
            }
        }
        let year = (cycles * 400 + first) as u32;
        let mut day_in_year = day_in_cycle - days_before_year(first);
        let mut month = 1;
        loop {
            let days = u64::from(days_in_month(year, month));
            if day_in_year < days {
                return (year, month, day_in_year as u8 + 1);
            }
            day_in_year -= days;
            month += 1;
        }
    }

    pub fn checked_add_days(self, days: u64) -> Option<Self> {
        let day = self.0.checked_add(days)?;
        (day <= LAST_DAY).then_some(Self(day))
    }

    /// Adds whole Gregorian months, clamping the day to the target month's end.
    pub fn checked_add_months(self, months: u32) -> Option<Self> {
        let (year, month, day) = self.components();
        let index = u64::from(year - 1)
            .checked_mul(12)?
            .checked_add(u64::from(month - 1))?
            .checked_add(u64::from(months))?;
        let target_year = u32::try_from(index / 12 + 1).ok()?;
        let target_month = (index % 12 + 1) as u8;
        Self::new(
            target_year,
            target_month,
            day.min(days_in_month(target_year, target_month)),
        )
        .ok()
    }
}

const fn days_before_year(year: u64) -> u64 {
    let preceding_years = year - 1;
    preceding_years * 365 + preceding_years / 4 - preceding_years / 100 + preceding_years / 400
}

fn days_in_month(year: u32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Role assigned to the command by the authoritative host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeActor {
    Host,
    Guest,
    Ai,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeCommand {
    SetPaused(bool),
    SetSpeed(Speed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    HumanRequired,
    HostRequired,
    DateOverflow,
}

/// Simulation time derived from a scenario start date and elapsed ticks.
/// The host schedules when to advance based on speed. Simulation advancement
/// itself always represents one day and never reads system time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimTime {
    start_date: Date,
    tick: Tick,
    speed: Speed,
    paused: bool,
}

impl SimTime {
    pub const fn new(start_date: Date, speed: Speed, paused: bool) -> Self {
        Self {
            start_date,
            tick: Tick::ZERO,
            speed,
            paused,
        }
    }

    /// Reconstructs a saved clock, checking that its current date is representable.
    pub fn restore(
        start_date: Date,
        tick: Tick,
        speed: Speed,
        paused: bool,
    ) -> Result<Self, TimeError> {
        start_date
            .checked_add_days(tick.get())
            .ok_or(TimeError::DateOverflow)?;
        Ok(Self {
            start_date,
            tick,
            speed,
            paused,
        })
    }

    pub const fn start_date(&self) -> Date {
        self.start_date
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn date(&self) -> Date {
        // Construction and advancement keep the sum within the date range.
        Date(self.start_date.0 + self.tick.get())
    }

    pub const fn speed(&self) -> Speed {
        self.speed
    }

    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn apply(&mut self, actor: TimeActor, command: TimeCommand) -> Result<(), TimeError> {
        match (actor, command) {
            (TimeActor::Ai, _) => return Err(TimeError::HumanRequired),
            (TimeActor::Guest, TimeCommand::SetSpeed(_)) => return Err(TimeError::HostRequired),
            (_, TimeCommand::SetPaused(paused)) => self.paused = paused,
            (TimeActor::Host, TimeCommand::SetSpeed(speed)) => self.speed = speed,
        }
        Ok(())
    }

    /// Advances one day. A paused simulation returns `Ok(false)` unchanged.
    pub fn advance(&mut self) -> Result<bool, TimeError> {
        if self.paused {
            return Ok(false);
        }

        let next = self.tick.checked_add(1).ok_or(TimeError::DateOverflow)?;
        self.start_date
            .checked_add_days(next.get())
            .ok_or(TimeError::DateOverflow)?;
        self.tick = next;
        Ok(true)
    }
}
