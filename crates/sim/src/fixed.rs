/// A signed fixed-point value stored in thousandths.
///
/// Checked operations return `None` if the result is outside the stored range
/// or the divisor is zero. Multiplication and division use wide integer
/// intermediates and truncate fractional thousandths toward zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed(i64);

impl Fixed {
    pub const SCALE: i64 = 1_000;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);

    pub const fn from_raw(thousandths: i64) -> Self {
        Self(thousandths)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub fn from_integer(value: i64) -> Option<Self> {
        value.checked_mul(Self::SCALE).map(Self)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let product = i128::from(self.0) * i128::from(rhs.0);
        i64::try_from(product / i128::from(Self::SCALE))
            .ok()
            .map(Self)
    }

    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        let scaled_dividend = i128::from(self.0) * i128::from(Self::SCALE);
        let quotient = scaled_dividend.checked_div(i128::from(rhs.0))?;
        i64::try_from(quotient).ok().map(Self)
    }
}
