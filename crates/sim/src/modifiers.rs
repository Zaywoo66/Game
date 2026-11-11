use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

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

impl ModifierKind {
    pub(crate) fn accepts_value(self, value: Fixed) -> bool {
        self == Self::Additive || Fixed::ONE.checked_add(value).is_some()
    }
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
    effective_value_from(base, key, modifiers.iter())
}

fn effective_value_from<'a>(
    base: Fixed,
    key: &ModKey,
    modifiers: impl Iterator<Item = &'a Modifier>,
) -> Option<Fixed> {
    let mut additive = i128::from(base.raw());
    let mut multipliers = Vec::new();

    for modifier in modifiers.filter(|modifier| modifier.key == *key) {
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

/// Active modifier bundles indexed by their stable source ID.
///
/// The simulation expires bundles explicitly during its tick. Queries use
/// the active bundles as stored and do not mutate or advance the simulation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModifierStore {
    sources: BTreeMap<ModifierSourceId, ModifierSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModifierSource {
    expires_at: Option<u64>,
    modifiers: Vec<Modifier>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierInsertError {
    DuplicateSource(ModifierSourceId),
    InvalidMultiplier {
        source: ModifierSourceId,
        index: usize,
    },
    SourceMismatch {
        expected: ModifierSourceId,
        actual: ModifierSourceId,
    },
}

impl ModifierStore {
    /// Adds a source bundle. `None` means it lasts until explicitly removed.
    ///
    /// All modifiers must name this source and each multiplier's factor
    /// must fit in Fixed. A rejected insert leaves the store unchanged.
    pub fn insert_source(
        &mut self,
        source: ModifierSourceId,
        expires_at: Option<u64>,
        modifiers: Vec<Modifier>,
    ) -> Result<(), ModifierInsertError> {
        let Entry::Vacant(entry) = self.sources.entry(source) else {
            return Err(ModifierInsertError::DuplicateSource(source));
        };

        for (index, modifier) in modifiers.iter().enumerate() {
            if modifier.source != source {
                return Err(ModifierInsertError::SourceMismatch {
                    expected: source,
                    actual: modifier.source,
                });
            }
            if !modifier.kind.accepts_value(modifier.value) {
                return Err(ModifierInsertError::InvalidMultiplier { source, index });
            }
        }

        entry.insert(ModifierSource {
            expires_at,
            modifiers,
        });
        Ok(())
    }

    /// Removes every modifier granted by the source. Returns whether it existed.
    pub fn remove_source(&mut self, source: ModifierSourceId) -> bool {
        self.sources.remove(&source).is_some()
    }

    /// Removes all timed sources whose expiry tick is at or before `tick`.
    ///
    /// Returns removed IDs in ascending order so callers can emit stable deltas.
    pub fn expire_through(&mut self, tick: u64) -> Vec<ModifierSourceId> {
        let mut expired = Vec::new();
        self.sources.retain(|id, source| {
            if source.expires_at.is_some_and(|expiry| expiry <= tick) {
                expired.push(*id);
                false
            } else {
                true
            }
        });
        expired
    }

    /// Iterates active bundles by source ID, preserving modifier order within each bundle.
    pub fn sources(&self) -> impl Iterator<Item = (ModifierSourceId, Option<u64>, &[Modifier])> {
        self.sources
            .iter()
            .map(|(&id, source)| (id, source.expires_at, source.modifiers.as_slice()))
    }

    /// Evaluates the requested key across currently active bundles.
    pub fn effective_value(&self, base: Fixed, key: &ModKey) -> Option<Fixed> {
        let modifiers = self
            .sources
            .values()
            .flat_map(|source| source.modifiers.iter());
        effective_value_from(base, key, modifiers)
    }
}
