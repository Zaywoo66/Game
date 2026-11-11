//! Directional diplomatic opinion in whole points.

use crate::CountryId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Opinion(i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpinionError {
    OutOfRange(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpinionStateError {
    UnknownCountry(CountryId),
    SameCountry(CountryId),
    Duplicate { from: CountryId, toward: CountryId },
}

impl Opinion {
    pub const MIN: i16 = -100;
    pub const MAX: i16 = 100;
    pub const NEUTRAL: Self = Self(0);

    pub fn new(points: i32) -> Result<Self, OpinionError> {
        if !(i32::from(Self::MIN)..=i32::from(Self::MAX)).contains(&points) {
            return Err(OpinionError::OutOfRange(points));
        }
        Ok(Self(points as i16))
    }

    pub const fn points(self) -> i16 {
        self.0
    }

    /// Adds whole opinion points and clamps at the documented bounds.
    pub fn adjusted(self, change: i32) -> Self {
        let total = i64::from(self.0) + i64::from(change);
        Self(total.clamp(i64::from(Self::MIN), i64::from(Self::MAX)) as i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bounds_and_clamps_changes_without_overflow() {
        assert_eq!(Opinion::new(-100).unwrap().points(), -100);
        assert_eq!(Opinion::new(100).unwrap().points(), 100);
        assert_eq!(Opinion::new(-101), Err(OpinionError::OutOfRange(-101)));
        assert_eq!(Opinion::new(101), Err(OpinionError::OutOfRange(101)));
        assert_eq!(Opinion::NEUTRAL.adjusted(i32::MAX).points(), 100);
        assert_eq!(Opinion::NEUTRAL.adjusted(i32::MIN).points(), -100);
        assert_eq!(Opinion::new(7).unwrap().adjusted(-12).points(), -5);
    }
}
