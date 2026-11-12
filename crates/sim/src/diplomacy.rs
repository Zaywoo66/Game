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
    /// Directed (guarantor, protected) pairs; no expiry.
    pub guarantees: BTreeSet<(CountryId, CountryId)>,
    /// Directed subject -> overlord relations. No subject may also be an overlord.
    pub vassals: BTreeMap<CountryId, CountryId>,
    /// Directed overlord -> prospective subject offers.
    pub vassal_offers: BTreeMap<(CountryId, CountryId), Date>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiplomacyError {
    UnknownCountry(CountryId),
    SameCountry,
    DirectWar,
    MustBreakAlliance,
    MustRevokeGuarantee,
    AlreadyAllied,
    NotAllied,
    AlreadyGuaranteed,
    NotGuaranteed,
    AlreadyVassal,
    NotVassal,
    VassalChain,
    SubjectCannotDeclareWar,
    SubjectCannotOfferTreaty,
    MustReleaseVassal,
    AtWar,
    OfferAlreadyPending,
    NoOffer,
    Cooldown(Date),
    DateOverflow,
    InvalidState,
}

impl DiplomacyState {
    pub fn overlord_of(&self, subject: CountryId) -> Option<CountryId> {
        self.vassals.get(&subject).copied()
    }

    pub fn vassal_offers_to(&self, subject: CountryId) -> impl Iterator<Item = AllianceOffer> + '_ {
        self.vassal_offers
            .iter()
            .filter_map(move |(&(from, toward), &expires_at)| {
                (toward == subject).then_some(AllianceOffer {
                    from,
                    toward,
                    expires_at,
                })
            })
    }

    pub fn expire_vassal_offers(&mut self, date: Date) -> Vec<AllianceOffer> {
        let expired: Vec<_> = self
            .vassal_offers_to_all()
            .filter(|offer| offer.expires_at <= date)
            .collect();
        for offer in &expired {
            self.vassal_offers.remove(&(offer.from, offer.toward));
        }
        expired
    }

    fn vassal_offers_to_all(&self) -> impl Iterator<Item = AllianceOffer> + '_ {
        self.vassal_offers
            .iter()
            .map(|(&(from, toward), &expires_at)| AllianceOffer {
                from,
                toward,
                expires_at,
            })
    }

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

    pub fn guarantees(&self, guarantor: CountryId, protected: CountryId) -> bool {
        self.guarantees.contains(&(guarantor, protected))
    }

    pub fn guarantors_of(&self, protected: CountryId) -> impl Iterator<Item = CountryId> + '_ {
        self.guarantees
            .iter()
            .filter_map(move |&(guarantor, target)| (target == protected).then_some(guarantor))
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
        for &(guarantor, protected) in &self.guarantees {
            if guarantor == protected
                || !countries.contains(&guarantor)
                || !countries.contains(&protected)
            {
                return Err(DiplomacyError::InvalidState);
            }
        }
        for (&subject, &overlord) in &self.vassals {
            if subject == overlord
                || !countries.contains(&subject)
                || !countries.contains(&overlord)
                || self.vassals.contains_key(&overlord)
            {
                return Err(DiplomacyError::InvalidState);
            }
        }
        for (&(from, toward), &expires) in &self.vassal_offers {
            if from == toward
                || !countries.contains(&from)
                || !countries.contains(&toward)
                || expires <= now
                || self.vassals.contains_key(&from)
                || self.vassals.contains_key(&toward)
                || self.vassals.values().any(|&overlord| overlord == toward)
            {
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
