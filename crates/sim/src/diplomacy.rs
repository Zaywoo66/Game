use std::collections::{BTreeMap, BTreeSet};

use crate::{CharacterId, CountryId, Date};

/// The pair key is always ordered by numeric country ID.
pub fn pair(a: CountryId, b: CountryId) -> (CountryId, CountryId) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllianceOffer {
    pub from: CountryId,
    pub toward: CountryId,
    pub expires_at: Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarriageOffer {
    pub from: CharacterId,
    pub toward: CharacterId,
    pub expires_at: Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarriageError {
    UnknownCharacter(CharacterId),
    Unnamed(CharacterId),
    WrongCountry(CharacterId),
    SameCountry,
    NotAlive(CharacterId),
    Underage(CharacterId),
    AlreadyMarried(CharacterId),
    UnknownSex(CharacterId),
    SameSex,
    NoRulerOrHeir,
    OfferAlreadyPending,
    NoOffer,
    DateOverflow,
    InvalidState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiplomacyState {
    pub alliances: BTreeSet<(CountryId, CountryId)>,
    pub offers: BTreeMap<(CountryId, CountryId), Date>,
    pub cooldowns: BTreeMap<(CountryId, CountryId), Date>,
    pub marriage_offers: BTreeMap<(CharacterId, CharacterId), Date>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiplomacyError {
    UnknownCountry(CountryId),
    SameCountry,
    DirectWar,
    MustBreakAlliance,
    AlreadyAllied,
    NotAllied,
    OfferAlreadyPending,
    NoOffer,
    Cooldown(Date),
    DateOverflow,
    InvalidState,
}

impl DiplomacyState {
    pub fn marriage_offer(&self, from: CharacterId, toward: CharacterId) -> Option<MarriageOffer> {
        self.marriage_offers
            .get(&(from, toward))
            .copied()
            .map(|expires_at| MarriageOffer {
                from,
                toward,
                expires_at,
            })
    }

    pub fn marriage_offers_to<'a>(
        &'a self,
        characters: &'a BTreeMap<CharacterId, crate::CharacterState>,
        country: CountryId,
    ) -> impl Iterator<Item = MarriageOffer> + 'a {
        self.marriage_offers
            .iter()
            .filter_map(move |(&(from, toward), &expires_at)| {
                (characters
                    .get(&toward)
                    .is_some_and(|person| person.country == country))
                .then_some(MarriageOffer {
                    from,
                    toward,
                    expires_at,
                })
            })
    }

    pub fn expire_marriages(
        &mut self,
        date: Date,
        characters: &BTreeMap<CharacterId, crate::CharacterState>,
    ) -> Vec<MarriageOffer> {
        let expired: Vec<_> = self
            .marriage_offers
            .iter()
            .filter_map(|(&(from, toward), &expires_at)| {
                (expires_at <= date
                    || characters
                        .get(&from)
                        .is_none_or(|person| !person.is_alive_on(date) || person.spouse.is_some())
                    || characters
                        .get(&toward)
                        .is_none_or(|person| !person.is_alive_on(date) || person.spouse.is_some()))
                .then_some(MarriageOffer {
                    from,
                    toward,
                    expires_at,
                })
            })
            .collect();
        for offer in &expired {
            self.marriage_offers.remove(&(offer.from, offer.toward));
        }
        expired
    }
    pub fn allied(&self, a: CountryId, b: CountryId) -> bool {
        self.alliances.contains(&pair(a, b))
    }

    pub fn offer(&self, from: CountryId, toward: CountryId) -> Option<AllianceOffer> {
        self.offers
            .get(&(from, toward))
            .copied()
            .map(|expires_at| AllianceOffer {
                from,
                toward,
                expires_at,
            })
    }

    pub fn offers_to(&self, toward: CountryId) -> impl Iterator<Item = AllianceOffer> + '_ {
        self.offers
            .iter()
            .filter_map(move |(&(from, target), &expires_at)| {
                (target == toward).then_some(AllianceOffer {
                    from,
                    toward,
                    expires_at,
                })
            })
    }

    pub fn expire(&mut self, date: Date) -> Vec<AllianceOffer> {
        let expired: Vec<_> = self
            .offers
            .iter()
            .filter_map(|(&(from, toward), &expires_at)| {
                (expires_at <= date).then_some(AllianceOffer {
                    from,
                    toward,
                    expires_at,
                })
            })
            .collect();
        for offer in &expired {
            self.offers.remove(&(offer.from, offer.toward));
        }
        self.cooldowns.retain(|_, &mut until| until > date);
        expired
    }

    pub fn validate(
        &self,
        countries: impl IntoIterator<Item = CountryId>,
        now: Date,
    ) -> Result<(), DiplomacyError> {
        let countries: BTreeSet<_> = countries.into_iter().collect();
        for &(a, b) in &self.alliances {
            if a >= b || !countries.contains(&a) || !countries.contains(&b) {
                return Err(DiplomacyError::InvalidState);
            }
        }
        for (&(from, toward), &expires) in &self.offers {
            if from == toward
                || !countries.contains(&from)
                || !countries.contains(&toward)
                || expires <= now
                || self.allied(from, toward)
                || self.offers.contains_key(&(toward, from))
                || self.cooldowns.contains_key(&pair(from, toward))
            {
                return Err(DiplomacyError::InvalidState);
            }
        }
        for (&(a, b), &until) in &self.cooldowns {
            if a >= b
                || !countries.contains(&a)
                || !countries.contains(&b)
                || until <= now
                || self.allied(a, b)
            {
                return Err(DiplomacyError::InvalidState);
            }
        }
        Ok(())
    }
}
