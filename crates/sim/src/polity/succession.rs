use std::collections::{BTreeMap, BTreeSet};

use crate::{CharacterId, CountryId, Date, ModifierStore, StateChange};

use super::{CountryPolity, PolityState};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterStats {
    pub adm: u8,
    pub dip: u8,
    pub mil: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Unknown,
    Male,
    Female,
}

/// Precision of a historical birth date. `Year` records only the year and
/// treats a character as having reached each new age on January 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BirthPrecision {
    #[default]
    Day,
    Year,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterState {
    pub name: String,
    pub country: CountryId,
    pub dynasty: String,
    pub sex: Sex,
    pub spouse: Option<CharacterId>,
    pub birth: Date,
    pub birth_precision: BirthPrecision,
    pub death: Option<Date>,
    pub stats: CharacterStats,
    pub traits: BTreeSet<String>,
    pub modifiers: ModifierStore,
}

impl CharacterState {
    pub fn is_alive_on(&self, date: Date) -> bool {
        self.birth <= date && self.death.is_none_or(|death| date < death)
    }

    /// Whole calendar years since birth, frozen at death. Before birth there
    /// is no age. A 29 February birthday reaches its next year on 1 March in
    /// a common year.
    pub fn age_years(&self, date: Date) -> Option<u32> {
        let end = self.death.map_or(date, |death| death.min(date));
        if end < self.birth {
            return None;
        }
        let (year, month, day) = end.components();
        let (birth_year, birth_month, birth_day) = self.birth.components();
        if self.birth_precision == BirthPrecision::Year {
            return Some(year - birth_year);
        }
        Some(year - birth_year - u32::from((month, day) < (birth_month, birth_day)))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SuccessionState {
    pub ruler: Option<CharacterId>,
    pub heir: Option<CharacterId>,
}

impl PolityState {
    pub(crate) fn character_is_available(
        &self,
        country: CountryId,
        character: CharacterId,
        date: Date,
    ) -> bool {
        available(&self.characters, country, character, date)
    }

    pub(crate) fn heir_is_eligible(
        &self,
        country: CountryId,
        heir: CharacterId,
        date: Date,
    ) -> bool {
        self.countries.get(&country).is_some_and(|state| {
            state.succession.ruler != Some(heir) && self.character_is_available(country, heir, date)
        })
    }

    pub(crate) fn eligible_heirs(&self, country: CountryId, date: Date) -> Vec<CharacterId> {
        let mut candidates: Vec<_> = self
            .characters
            .iter()
            .filter(|(id, _)| self.heir_is_eligible(country, **id, date))
            .map(|(&id, character)| (character.birth, id))
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, id)| id).collect()
    }

    pub(crate) fn is_in_regency(&self, country: CountryId, date: Date) -> Option<bool> {
        let state = self.countries.get(&country)?;
        Some(regency(
            &self.characters,
            state.succession.ruler,
            date,
            self.rules.adulthood_years,
        ))
    }

    pub(super) fn projected_succession(
        &self,
        country: CountryId,
        state: &CountryPolity,
        date: Date,
    ) -> SuccessionState {
        project(&self.characters, country, state.succession, date)
    }

    pub(super) fn advance_characters(&mut self, previous: Date, date: Date) -> Vec<StateChange> {
        let mut changes = Vec::new();
        let ended: BTreeSet<_> = self
            .characters
            .iter()
            .filter_map(|(&id, state)| {
                let spouse = state.spouse?;
                (state.death == Some(date) && id < spouse)
                    .then_some((id, spouse))
                    .or_else(|| (state.death == Some(date) && spouse < id).then_some((spouse, id)))
            })
            .collect();
        for (a, b) in ended {
            if let Some(state) = self.characters.get_mut(&a) {
                state.spouse = None;
            }
            if let Some(state) = self.characters.get_mut(&b) {
                state.spouse = None;
            }
            changes.push(StateChange::MarriageChanged {
                a,
                b,
                active: false,
            });
        }
        for (&character, state) in &self.characters {
            if state.death == Some(date) {
                changes.push(StateChange::CharacterDied {
                    character,
                    country: state.country,
                });
            }
        }
        for (&country, state) in &mut self.countries {
            let old = state.succession;
            let next = project(&self.characters, country, old, date);
            let was_minor = regency(
                &self.characters,
                old.ruler,
                previous,
                self.rules.adulthood_years,
            );
            let is_minor = regency(
                &self.characters,
                next.ruler,
                date,
                self.rules.adulthood_years,
            );
            if old != next || was_minor != is_minor {
                state.succession = next;
                changes.push(StateChange::SuccessionChanged {
                    country,
                    succession: next,
                    regency: is_minor,
                });
            }
        }
        changes
    }
}

fn available(
    characters: &BTreeMap<CharacterId, CharacterState>,
    country: CountryId,
    character: CharacterId,
    date: Date,
) -> bool {
    characters
        .get(&character)
        .is_some_and(|state| state.country == country && state.is_alive_on(date))
}

fn project(
    characters: &BTreeMap<CharacterId, CharacterState>,
    country: CountryId,
    old: SuccessionState,
    date: Date,
) -> SuccessionState {
    let heir = old
        .heir
        .filter(|&id| Some(id) != old.ruler && available(characters, country, id, date));
    if old
        .ruler
        .is_some_and(|id| available(characters, country, id, date))
    {
        return SuccessionState {
            ruler: old.ruler,
            heir,
        };
    }

    let previous_dynasty = old
        .ruler
        .and_then(|ruler| characters.get(&ruler))
        .map(|ruler| ruler.dynasty.as_str());
    let ruler = heir.or_else(|| {
        characters
            .iter()
            .filter(|(_, candidate)| candidate.country == country && candidate.is_alive_on(date))
            .min_by_key(|(id, candidate)| {
                let outside_dynasty =
                    previous_dynasty.is_some_and(|dynasty| candidate.dynasty != dynasty);
                (outside_dynasty, candidate.birth, **id)
            })
            .map(|(&id, _)| id)
    });
    SuccessionState { ruler, heir: None }
}

fn regency(
    characters: &BTreeMap<CharacterId, CharacterState>,
    ruler: Option<CharacterId>,
    date: Date,
    adulthood_years: u16,
) -> bool {
    ruler
        .and_then(|id| characters.get(&id))
        .filter(|ruler| ruler.is_alive_on(date))
        .and_then(|ruler| ruler.age_years(date))
        .is_some_and(|age| age < u32::from(adulthood_years))
}
