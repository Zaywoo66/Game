use crate::Fixed;

/// A modifier key supplied by game data, such as an army statistic.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModKey(String);

impl ModKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identifier of the source that granted a modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModifierSourceId(u32);

impl ModifierSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierKind {
    Additive,
    Multiplicative,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modifier {
    pub source: ModifierSourceId,
    pub key: ModKey,
    pub value: Fixed,
    pub kind: ModifierKind,
}

/// Evaluates `(base + sum(additive)) * product(1 + multiplicative)` for one key.
///
/// Additions are summed in a wide integer before converting the adjusted base
/// to `Fixed`. Multipliers are folded in ascending source ID order, with
/// ascending value breaking ties, so the truncation of fractional thousandths
/// at each step is reproducible. Returns `None` if an adjusted base, factor,
/// product step, or final result is outside the range of `Fixed`.
pub fn effective_value(base: Fixed, key: &ModKey, modifiers: &[Modifier]) -> Option<Fixed> {
    let mut additive = i128::from(base.raw());
    let mut multipliers = Vec::new();

    for modifier in modifiers.iter().filter(|modifier| modifier.key == *key) {
        match modifier.kind {
            ModifierKind::Additive => additive += i128::from(modifier.value.raw()),
            ModifierKind::Multiplicative => multipliers.push((modifier.source, modifier.value)),
        }
    }

    let adjusted_base = Fixed::from_raw(i64::try_from(additive).ok()?);
    multipliers.sort_unstable();
    let mut product = Fixed::ONE;
    for (_, value) in multipliers {
        let factor = Fixed::ONE.checked_add(value)?;
        product = product.checked_mul(factor)?;
    }

    adjusted_base.checked_mul(product)
}
