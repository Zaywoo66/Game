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
