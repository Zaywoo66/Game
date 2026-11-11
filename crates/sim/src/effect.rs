use std::num::NonZeroU64;

use crate::{Fixed, ModKey, Modifier, ModifierKind, ModifierSourceId};

/// A data-driven change to prepare before applying it to simulation state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    AddModifier {
        key: ModKey,
        value: Fixed,
        kind: ModifierKind,
        duration_days: Option<NonZeroU64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectError {
    ExpiryOverflow,
    InvalidMultiplier,
}

/// A change ready for the simulation to apply to the subject's modifier store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedModifier {
    pub modifier: Modifier,
    pub expires_at: Option<u64>,
}

impl Effect {
    /// Resolves an effect using the caller's stable source ID and the current tick.
    pub fn prepare(
        &self,
        source: ModifierSourceId,
        current_tick: u64,
    ) -> Result<PreparedModifier, EffectError> {
        match self {
            Self::AddModifier {
                key,
                value,
                kind,
                duration_days,
            } => {
                let expires_at = duration_days
                    .map(|days| {
                        current_tick
                            .checked_add(days.get())
                            .ok_or(EffectError::ExpiryOverflow)
                    })
                    .transpose()?;

                if *kind == ModifierKind::Multiplicative && Fixed::ONE.checked_add(*value).is_none()
                {
                    return Err(EffectError::InvalidMultiplier);
                }

                Ok(PreparedModifier {
                    modifier: Modifier {
                        source,
                        key: key.clone(),
                        value: *value,
                        kind: *kind,
                    },
                    expires_at,
                })
            }
        }
    }
}
