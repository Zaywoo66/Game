use std::collections::BTreeMap;
use std::num::NonZeroU64;

use crate::{
    CountryId, Effect, EffectError, ModifierSourceId, PreparedModifier, SeededRng, Tick, Trigger,
    TriggerView,
};

/// Validated content copied from event YAML into an authoritative game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDefinition {
    pub key: String,
    pub title_key: String,
    pub desc_key: String,
    pub trigger: Trigger,
    pub mean_time_to_happen: Option<NonZeroU64>,
    pub repeatable: bool,
    pub cooldown_days: Option<NonZeroU64>,
    pub options: Vec<EventOption>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventOption {
    pub text_key: String,
    pub effects: Vec<Effect>,
    pub ai_weight: u32,
    pub ai_weight_if: Option<ConditionalAiWeight>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionalAiWeight {
    pub when: Trigger,
    pub add: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventState {
    pub definitions: BTreeMap<String, EventDefinition>,
    /// A pending event records its firing tick.
    pub pending: BTreeMap<(CountryId, String), Tick>,
    /// Last resolution tick for repeatable events; one-shot entries never fire again.
    pub resolved: BTreeMap<(CountryId, String), Tick>,
    pub next_source_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventError {
    AlreadyConfigured,
    EmptyCatalog,
    InvalidDefinition(String),
    UnknownCountry(CountryId),
    UnknownEvent(String),
    NotPending(String),
    InvalidOption(u32),
    ModifierSourceExhausted,
    InvalidEffect(EffectError),
    InvalidSavedState,
}

impl EventState {
    pub fn new(
        definitions: BTreeMap<String, EventDefinition>,
        next_source_id: u32,
    ) -> Result<Self, EventError> {
        if definitions.is_empty() {
            return Err(EventError::EmptyCatalog);
        }
        if next_source_id == 0 {
            return Err(EventError::ModifierSourceExhausted);
        }
        for (key, event) in &definitions {
            if key != &event.key
                || !valid_key(key)
                || !valid_key(&event.title_key)
                || !valid_key(&event.desc_key)
                || event.options.is_empty()
                || !event.options.iter().any(|option| option.ai_weight > 0)
                || event.repeatable != event.cooldown_days.is_some()
                || event.options.iter().any(|option| {
                    !valid_key(&option.text_key)
                        || option.effects.iter().any(|effect| match effect {
                            Effect::AddModifier {
                                key, value, kind, ..
                            } => {
                                !valid_key(key.as_str())
                                    || (*kind == crate::ModifierKind::Multiplicative
                                        && crate::Fixed::ONE.checked_add(*value).is_none())
                            }
                        })
                })
            {
                return Err(EventError::InvalidDefinition(key.clone()));
            }
        }
        Ok(Self {
            definitions,
            pending: BTreeMap::new(),
            resolved: BTreeMap::new(),
            next_source_id,
        })
    }

    pub fn validate_saved(
        &self,
        current: Tick,
        countries: impl Iterator<Item = CountryId>,
        minimum_source_id: u32,
    ) -> Result<(), EventError> {
        let countries = countries.collect::<std::collections::BTreeSet<_>>();
        if self.next_source_id == 0 || self.next_source_id <= minimum_source_id {
            return Err(EventError::InvalidSavedState);
        }
        for ((country, key), fired_at) in &self.pending {
            let Some(definition) = self.definitions.get(key) else {
                return Err(EventError::InvalidSavedState);
            };
            if !countries.contains(country)
                || *fired_at > current
                || self
                    .resolved
                    .get(&(*country, key.clone()))
                    .is_some_and(|last| {
                        !definition.repeatable
                            || fired_at.get().saturating_sub(last.get())
                                < definition.cooldown_days.expect("validated event").get()
                    })
            {
                return Err(EventError::InvalidSavedState);
            }
        }
        for ((country, key), resolved_at) in &self.resolved {
            if !countries.contains(country)
                || !self.definitions.contains_key(key)
                || *resolved_at > current
            {
                return Err(EventError::InvalidSavedState);
            }
        }
        Ok(())
    }

    /// Evaluates definitions in key order, then countries in ID order.
    pub fn fire_due<'a>(
        &mut self,
        seed: u64,
        tick: Tick,
        countries: impl Iterator<Item = (CountryId, &'a dyn TriggerView)>,
    ) -> Vec<(CountryId, String)> {
        let countries = countries.collect::<Vec<_>>();
        let mut fired = Vec::new();
        for (key, definition) in &self.definitions {
            let Some(mtth) = definition.mean_time_to_happen else {
                continue;
            };
            for &(country, view) in &countries {
                let pair = (country, key.clone());
                if self.pending.contains_key(&pair) || !definition.trigger.evaluate(view) {
                    continue;
                }
                if let Some(last) = self.resolved.get(&pair) {
                    if !definition.repeatable
                        || tick.get().saturating_sub(last.get())
                            < definition
                                .cooldown_days
                                .expect("validated repeatable event")
                                .get()
                    {
                        continue;
                    }
                }
                let mut rng = event_rng(seed, tick.get(), country, key);
                if draw_below(&mut rng, mtth.get()) == 0 {
                    self.pending.insert(pair, tick);
                    fired.push((country, key.clone()));
                }
            }
        }
        fired
    }

    pub fn prepare_choice(
        &self,
        country: CountryId,
        key: &str,
        option: u32,
        tick: Tick,
    ) -> Result<Vec<(ModifierSourceId, PreparedModifier)>, EventError> {
        let definition = self
            .definitions
            .get(key)
            .ok_or_else(|| EventError::UnknownEvent(key.into()))?;
        if !self.pending.contains_key(&(country, key.into())) {
            return Err(EventError::NotPending(key.into()));
        }
        let option = definition
            .options
            .get(usize::try_from(option).map_err(|_| EventError::InvalidOption(option))?)
            .ok_or(EventError::InvalidOption(option))?;
        self.next_source_id
            .checked_add(
                u32::try_from(option.effects.len())
                    .map_err(|_| EventError::ModifierSourceExhausted)?,
            )
            .ok_or(EventError::ModifierSourceExhausted)?;
        option
            .effects
            .iter()
            .enumerate()
            .map(|(index, effect)| {
                let id = self
                    .next_source_id
                    .checked_add(
                        u32::try_from(index).map_err(|_| EventError::ModifierSourceExhausted)?,
                    )
                    .ok_or(EventError::ModifierSourceExhausted)?;
                let source = ModifierSourceId::new(id);
                let prepared = effect
                    .prepare(source, tick.get())
                    .map_err(EventError::InvalidEffect)?;
                Ok((source, prepared))
            })
            .collect()
    }

    pub fn resolve(&mut self, country: CountryId, key: &str, tick: Tick, sources_used: usize) {
        self.pending.remove(&(country, key.into()));
        self.resolved.insert((country, key.into()), tick);
        self.next_source_id += u32::try_from(sources_used).expect("validated source count");
    }

    pub fn ai_option(
        &self,
        country: CountryId,
        key: &str,
        view: &impl TriggerView,
        seed: u64,
        tick: Tick,
    ) -> Option<u32> {
        self.pending.get(&(country, key.into()))?;
        let definition = self.definitions.get(key)?;
        let mut weights = definition
            .options
            .iter()
            .map(|option| {
                let conditional = option
                    .ai_weight_if
                    .as_ref()
                    .filter(|condition| condition.when.evaluate(view))
                    .map_or(0, |condition| condition.add);
                (i64::from(option.ai_weight) + i64::from(conditional)).max(0) as u64
            })
            .collect::<Vec<_>>();
        if weights.iter().all(|&weight| weight == 0) {
            weights = definition
                .options
                .iter()
                .map(|option| u64::from(option.ai_weight))
                .collect();
        }
        let total: u64 = weights.iter().sum();
        let mut rng = event_rng(
            seed ^ u64::from_le_bytes(*b"option01"),
            tick.get(),
            country,
            key,
        );
        let mut draw = draw_below(&mut rng, total);
        for (index, weight) in weights.into_iter().enumerate() {
            if draw < weight {
                return u32::try_from(index).ok();
            }
            draw -= weight;
        }
        None
    }
}

/// Stable per-event stream independent of catalog iteration and global RNG.
pub fn event_rng(seed: u64, tick: u64, country: CountryId, key: &str) -> SeededRng {
    let mut key_hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in key.bytes() {
        key_hash = (key_hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    crate::derive_ai_rng(seed ^ key_hash, tick, country)
}

fn draw_below(rng: &mut SeededRng, bound: u64) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = rng.next_u64();
        if value >= threshold {
            return value % bound;
        }
    }
}

fn valid_key(key: &str) -> bool {
    let mut segments = key.split('_');
    segments.next().is_some_and(valid_segment) && segments.all(valid_segment)
}

fn valid_segment(segment: &str) -> bool {
    segment
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && segment
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}
