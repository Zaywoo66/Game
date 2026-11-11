use std::{collections::BTreeMap, fmt, num::NonZeroU32, num::NonZeroU64};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use strategy_sim::{
    AiProfileKey, CharacterId, Controller, CountryId, CountryState, Date, DiplomacyError,
    DiplomacyState, Effect, Fixed, GameState, InvalidDate, InvalidSpeed, MapIdentity,
    MapIdentityMismatch, ModKey, Modifier, ModifierInsertError, ModifierKind, ModifierSourceId,
    ModifierStore, Opinion, OpinionError, OpinionStateError, PlayerId, PolityError, ProvinceGraph,
    ScheduleError, ScheduledEffect, SeededRng, SimTime, Speed, Tick, TimeError, TriggerView,
    WarError, WarState,
};

use crate::provinces::decode_hash;

mod military;
mod polity;

use military::WarStateFile;
use polity::PolityFile;

const LEGACY_VERSION: u32 = 1;
const POLITY_VERSION: u32 = 2;
const MILITARY_VERSION: u32 = 3;
const OPINION_VERSION: u32 = 4;
const ALLIANCE_VERSION: u32 = 5;
const COALITION_VERSION: u32 = 6;
const MARRIAGE_VERSION: u32 = 7;
const STATE_HASH_DOMAIN_V1: &[u8] = b"strategy:game-state:v1\0";
const STATE_HASH_DOMAIN_V2: &[u8] = b"strategy:game-state:v2\0";
const STATE_HASH_DOMAIN_V3: &[u8] = b"strategy:game-state:v3\0";
const STATE_HASH_DOMAIN_V4: &[u8] = b"strategy:game-state:v4\0";
const STATE_HASH_DOMAIN_V5: &[u8] = b"strategy:game-state:v5\0";
const STATE_HASH_DOMAIN_V6: &[u8] = b"strategy:game-state:v6\0";
const STATE_HASH_DOMAIN_V7: &[u8] = b"strategy:game-state:v7\0";

#[derive(Debug)]
pub enum SaveError {
    Json(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    VersionPolityMismatch(u32),
    VersionMilitaryMismatch(u32),
    VersionOpinionMismatch(u32),
    VersionDiplomacyMismatch(u32),
    VersionCoalitionMismatch(u32),
    VersionMarriageMismatch(u32),
    InvalidPolity(PolityError),
    InvalidMilitary(WarError),
    InvalidOpinion(OpinionError),
    InvalidOpinionState(OpinionStateError),
    InvalidDiplomacy(DiplomacyError),
    InvalidMapHash,
    MapMismatch(MapIdentityMismatch),
    UnorderedRecords(&'static str),
    InvalidDate(InvalidDate),
    InvalidSpeed(InvalidSpeed),
    InvalidClock(TimeError),
    InvalidModifier(ModifierInsertError),
    InvalidScheduledExpiry { due: u64, expires_at: u64 },
    InvalidSchedule(ScheduleError),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid save JSON: {error}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported save schema_version {version}")
            }
            Self::VersionPolityMismatch(version) => {
                write!(f, "save version {version} has the wrong polity payload")
            }
            Self::InvalidPolity(error) => write!(f, "invalid saved polity: {error:?}"),
            Self::VersionMilitaryMismatch(version) => {
                write!(f, "save version {version} has the wrong military payload")
            }
            Self::InvalidMilitary(error) => write!(f, "invalid saved military state: {error:?}"),
            Self::VersionOpinionMismatch(version) => {
                write!(f, "save version {version} has the wrong opinion payload")
            }
            Self::VersionDiplomacyMismatch(version) => {
                write!(f, "save version {version} has the wrong diplomacy payload")
            }
            Self::VersionCoalitionMismatch(version) => {
                write!(f, "save version {version} has the wrong coalition payload")
            }
            Self::VersionMarriageMismatch(version) => {
                write!(f, "save version {version} has the wrong marriage payload")
            }
            Self::InvalidOpinion(error) => write!(f, "invalid saved opinion: {error:?}"),
            Self::InvalidOpinionState(error) => write!(f, "invalid saved opinion state: {error:?}"),
            Self::InvalidDiplomacy(error) => write!(f, "invalid saved diplomacy state: {error:?}"),
            Self::InvalidMapHash => f.write_str("map_hash must be 64 lowercase hexadecimal digits"),
            Self::MapMismatch(error) => error.fmt(f),
            Self::UnorderedRecords(records) => write!(f, "{records} must be unique and sorted"),
            Self::InvalidDate(error) => write!(f, "invalid saved start date: {error:?}"),
            Self::InvalidSpeed(error) => write!(f, "invalid saved speed {}", error.0),
            Self::InvalidClock(error) => write!(f, "invalid saved clock: {error:?}"),
            Self::InvalidModifier(error) => write!(f, "invalid saved modifier: {error:?}"),
            Self::InvalidScheduledExpiry { due, expires_at } => write!(
                f,
                "scheduled expiry {expires_at} must be after due tick {due}"
            ),
            Self::InvalidSchedule(ScheduleError::ExpiredActiveSource {
                country,
                source,
                expires_at,
                current,
            }) => write!(
                f,
                "country {} has active modifier source {} with expiry {expires_at} at saved tick {}",
                country.get(),
                source.get(),
                current.get()
            ),
            Self::InvalidSchedule(error) => write!(f, "invalid saved schedule: {error:?}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::MapMismatch(error) => Some(error),
            _ => None,
        }
    }
}

/// Encodes the authoritative state in the canonical save format.
pub fn save_game(state: &GameState) -> Result<Vec<u8>, SaveError> {
    let identity = state.map_identity();
    let map_hash = identity
        .sha256
        .into_iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    serde_json::to_vec(&SaveFile {
        schema_version: schema_version(state),
        map_version: identity.version,
        map_hash,
        state: StateFile::from(state),
    })
    .map_err(SaveError::Json)
}

/// Loads a save against an already validated map. The static graph is not in the save.
pub fn load_game(bytes: &[u8], graph: ProvinceGraph) -> Result<GameState, SaveError> {
    let save: SaveFile<&RawValue> = serde_json::from_slice(bytes).map_err(SaveError::Json)?;
    if !matches!(
        save.schema_version,
        LEGACY_VERSION
            | POLITY_VERSION
            | MILITARY_VERSION
            | OPINION_VERSION
            | ALLIANCE_VERSION
            | COALITION_VERSION
            | MARRIAGE_VERSION
    ) {
        return Err(SaveError::UnsupportedSchemaVersion(save.schema_version));
    }

    let saved_identity = MapIdentity {
        version: save.map_version,
        sha256: decode_hash(&save.map_hash).ok_or(SaveError::InvalidMapHash)?,
    };
    graph
        .identity()
        .verify_save(saved_identity)
        .map_err(SaveError::MapMismatch)?;

    let state: StateFile = serde_json::from_str(save.state.get()).map_err(SaveError::Json)?;
    if (save.schema_version == LEGACY_VERSION && state.polity.is_some())
        || (matches!(save.schema_version, POLITY_VERSION | MILITARY_VERSION)
            && state.polity.is_none())
        || (save.schema_version == MARRIAGE_VERSION && state.polity.is_none())
    {
        return Err(SaveError::VersionPolityMismatch(save.schema_version));
    }
    if (save.schema_version < MILITARY_VERSION && state.military.is_some())
        || (save.schema_version == MILITARY_VERSION && state.military.is_none())
    {
        return Err(SaveError::VersionMilitaryMismatch(save.schema_version));
    }
    if (save.schema_version == OPINION_VERSION && state.opinions.is_empty())
        || (save.schema_version < OPINION_VERSION && !state.opinions.is_empty())
    {
        return Err(SaveError::VersionOpinionMismatch(save.schema_version));
    }
    if (save.schema_version == ALLIANCE_VERSION && state.diplomacy.is_none())
        || (save.schema_version < ALLIANCE_VERSION && state.diplomacy.is_some())
    {
        return Err(SaveError::VersionDiplomacyMismatch(save.schema_version));
    }
    let coalition = state
        .military
        .as_ref()
        .is_some_and(WarStateFile::has_coalition_history);
    if (save.schema_version == COALITION_VERSION && !coalition)
        || (save.schema_version < COALITION_VERSION && coalition)
    {
        return Err(SaveError::VersionCoalitionMismatch(save.schema_version));
    }
    let restored = state.restore(graph)?;
    let has_marriage_data = restored.polity().is_some_and(|polity| {
        polity.characters.values().any(|character| {
            character.sex != strategy_sim::Sex::Unknown || character.spouse.is_some()
        })
    }) || restored
        .diplomacy()
        .is_some_and(|diplomacy| !diplomacy.marriage_offers.is_empty());
    if has_marriage_data != (save.schema_version == MARRIAGE_VERSION) {
        return Err(SaveError::VersionMarriageMismatch(save.schema_version));
    }
    Ok(restored)
}

/// SHA-256 of the canonical state, including its map identity and save version.
pub fn state_hash(state: &GameState) -> Result<[u8; 32], SaveError> {
    let mut hash = Sha256::new();
    hash.update(match schema_version(state) {
        MARRIAGE_VERSION => STATE_HASH_DOMAIN_V7,
        COALITION_VERSION => STATE_HASH_DOMAIN_V6,
        ALLIANCE_VERSION => STATE_HASH_DOMAIN_V5,
        OPINION_VERSION => STATE_HASH_DOMAIN_V4,
        MILITARY_VERSION => STATE_HASH_DOMAIN_V3,
        POLITY_VERSION => STATE_HASH_DOMAIN_V2,
        _ => STATE_HASH_DOMAIN_V1,
    });
    hash.update(save_game(state)?);
    Ok(hash.finalize().into())
}

fn schema_version(state: &GameState) -> u32 {
    if state.polity().is_some_and(|polity| {
        polity.characters.values().any(|character| {
            character.sex != strategy_sim::Sex::Unknown || character.spouse.is_some()
        })
    }) || state
        .diplomacy()
        .is_some_and(|diplomacy| !diplomacy.marriage_offers.is_empty())
    {
        MARRIAGE_VERSION
    } else if state
        .war_state()
        .is_some_and(WarState::has_coalition_history)
    {
        COALITION_VERSION
    } else if state.diplomacy().is_some() {
        ALLIANCE_VERSION
    } else if state.opinions().next().is_some() {
        OPINION_VERSION
    } else if state.war_state().is_some() {
        MILITARY_VERSION
    } else if state.polity().is_some() {
        POLITY_VERSION
    } else {
        LEGACY_VERSION
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveFile<T> {
    schema_version: u32,
    map_version: NonZeroU32,
    map_hash: String,
    state: T,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    start_date: (u32, u8, u8),
    tick: u64,
    speed: u8,
    paused: bool,
    seed: u64,
    rng_state: u64,
    countries: Vec<CountryFile>,
    scheduled: Vec<ScheduledFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    opinions: Vec<OpinionFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diplomacy: Option<DiplomacyFile>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_polity"
    )]
    polity: Option<PolityFile>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_military"
    )]
    military: Option<WarStateFile>,
}

impl From<&GameState> for StateFile {
    fn from(state: &GameState) -> Self {
        Self {
            start_date: state.time().start_date().components(),
            tick: state.time().tick().get(),
            speed: state.time().speed().get(),
            paused: state.time().is_paused(),
            seed: state.seed(),
            rng_state: state.rng_state(),
            countries: state
                .countries()
                .map(|(id, state)| CountryFile::snapshot(id, state))
                .collect(),
            scheduled: state
                .pending_effects()
                .map(|(due, country, grant)| ScheduledFile {
                    due: due.get(),
                    country: country.get(),
                    source: grant.modifier.source.get(),
                    expires_at: grant.expires_at,
                    modifier: ModifierFile::from(&grant.modifier),
                })
                .collect(),
            opinions: state
                .opinions()
                .map(|(from, toward, opinion)| OpinionFile {
                    from: from.get(),
                    toward: toward.get(),
                    points: i32::from(opinion.points()),
                })
                .collect(),
            diplomacy: state.diplomacy().map(DiplomacyFile::from),
            polity: state.polity().map(PolityFile::from),
            military: state.war_state().map(WarStateFile::from),
        }
    }
}

impl StateFile {
    fn restore(self, graph: ProvinceGraph) -> Result<GameState, SaveError> {
        let (year, month, day) = self.start_date;
        let start_date = Date::new(year, month, day).map_err(SaveError::InvalidDate)?;
        let speed = Speed::try_from(self.speed).map_err(SaveError::InvalidSpeed)?;
        let time = SimTime::restore(start_date, Tick::new(self.tick), speed, self.paused)
            .map_err(SaveError::InvalidClock)?;

        require_sorted(&self.countries, "countries", |country| country.id)?;
        require_sorted(&self.scheduled, "scheduled grants", |grant| {
            (grant.due, grant.country, grant.source)
        })?;
        require_sorted(&self.opinions, "opinions", |opinion| {
            (opinion.from, opinion.toward)
        })?;
        let countries = self
            .countries
            .into_iter()
            .map(CountryFile::restore)
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let schedule = self
            .scheduled
            .into_iter()
            .map(ScheduledFile::restore)
            .collect::<Result<Vec<_>, _>>()?;

        let base = GameState::restore(
            time,
            self.seed,
            SeededRng::new(self.rng_state),
            graph,
            countries,
            schedule,
        )
        .map_err(SaveError::InvalidSchedule)?;
        let state = if let Some(polity) = self.polity {
            base.with_polity(polity.restore()?)
                .map_err(SaveError::InvalidPolity)?
        } else {
            base
        };
        let opinions = self
            .opinions
            .into_iter()
            .map(|entry| {
                Ok((
                    CountryId::new(entry.from),
                    CountryId::new(entry.toward),
                    Opinion::new(entry.points).map_err(SaveError::InvalidOpinion)?,
                ))
            })
            .collect::<Result<Vec<_>, SaveError>>()?;
        let state = state
            .with_opinions(opinions)
            .map_err(SaveError::InvalidOpinionState)?;
        let state = if let Some(military) = self.military {
            state
                .with_war_state(military.restore()?)
                .map_err(SaveError::InvalidMilitary)
        } else {
            Ok(state)
        }?;
        if let Some(diplomacy) = self.diplomacy {
            state
                .with_diplomacy(diplomacy.restore()?)
                .map_err(SaveError::InvalidDiplomacy)
        } else {
            Ok(state)
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpinionFile {
    from: u32,
    toward: u32,
    points: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiplomacyFile {
    alliances: Vec<PairFile>,
    offers: Vec<DatedPairFile>,
    cooldowns: Vec<DatedPairFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marriage_offers: Vec<DatedPairFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PairFile {
    a: u32,
    b: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatedPairFile {
    a: u32,
    b: u32,
    until: (u32, u8, u8),
}

impl From<&DiplomacyState> for DiplomacyFile {
    fn from(state: &DiplomacyState) -> Self {
        Self {
            alliances: state
                .alliances
                .iter()
                .map(|&(a, b)| PairFile {
                    a: a.get(),
                    b: b.get(),
                })
                .collect(),
            offers: state
                .offers
                .iter()
                .map(|(&(a, b), &until)| DatedPairFile {
                    a: a.get(),
                    b: b.get(),
                    until: until.components(),
                })
                .collect(),
            cooldowns: state
                .cooldowns
                .iter()
                .map(|(&(a, b), &until)| DatedPairFile {
                    a: a.get(),
                    b: b.get(),
                    until: until.components(),
                })
                .collect(),
            marriage_offers: state
                .marriage_offers
                .iter()
                .map(|(&(a, b), &until)| DatedPairFile {
                    a: a.get(),
                    b: b.get(),
                    until: until.components(),
                })
                .collect(),
        }
    }
}

impl DiplomacyFile {
    fn restore(self) -> Result<DiplomacyState, SaveError> {
        require_sorted(&self.alliances, "alliances", |item| (item.a, item.b))?;
        require_sorted(&self.offers, "alliance offers", |item| (item.a, item.b))?;
        require_sorted(&self.cooldowns, "alliance cooldowns", |item| {
            (item.a, item.b)
        })?;
        require_sorted(&self.marriage_offers, "marriage offers", |item| {
            (item.a, item.b)
        })?;
        let alliances = self
            .alliances
            .into_iter()
            .map(|item| (CountryId::new(item.a), CountryId::new(item.b)))
            .collect();
        let mut offers = std::collections::BTreeMap::new();
        let mut cooldowns = std::collections::BTreeMap::new();
        let mut marriage_offers = std::collections::BTreeMap::new();
        for item in self.offers {
            let (year, month, day) = item.until;
            offers.insert(
                (CountryId::new(item.a), CountryId::new(item.b)),
                Date::new(year, month, day).map_err(SaveError::InvalidDate)?,
            );
        }
        for item in self.cooldowns {
            let (year, month, day) = item.until;
            cooldowns.insert(
                (CountryId::new(item.a), CountryId::new(item.b)),
                Date::new(year, month, day).map_err(SaveError::InvalidDate)?,
            );
        }
        for item in self.marriage_offers {
            let (year, month, day) = item.until;
            marriage_offers.insert(
                (CharacterId::new(item.a), CharacterId::new(item.b)),
                Date::new(year, month, day).map_err(SaveError::InvalidDate)?,
            );
        }
        Ok(DiplomacyState {
            alliances,
            offers,
            cooldowns,
            marriage_offers,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountryFile {
    id: u32,
    controller: ControllerFile,
    known_technologies: Vec<String>,
    at_war: bool,
    sources: Vec<SourceFile>,
}

impl CountryFile {
    fn snapshot(id: CountryId, country: &CountryState) -> Self {
        Self {
            id: id.get(),
            controller: match country.controller() {
                Controller::Human(player) => ControllerFile::Human(player.get()),
                Controller::Ai(profile) => ControllerFile::Ai(profile.as_str().into()),
            },
            known_technologies: country.known_technologies().map(str::to_owned).collect(),
            at_war: country.is_at_war(),
            sources: SourceFile::snapshot_all(country.modifiers()),
        }
    }

    fn restore(self) -> Result<(CountryId, CountryState), SaveError> {
        if self
            .known_technologies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(SaveError::UnorderedRecords("known technologies"));
        }

        let controller = match self.controller {
            ControllerFile::Human(player) => Controller::Human(PlayerId::new(player)),
            ControllerFile::Ai(profile) => Controller::Ai(AiProfileKey::new(profile)),
        };
        let modifiers = SourceFile::restore_all(self.sources, "modifier sources")?;
        Ok((
            CountryId::new(self.id),
            CountryState::new(
                controller,
                self.known_technologies.into_iter().collect(),
                self.at_war,
                modifiers,
            ),
        ))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ControllerFile {
    Human(u32),
    Ai(String),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    id: u32,
    #[serde(deserialize_with = "required_optional")]
    expires_at: Option<u64>,
    modifiers: Vec<ModifierFile>,
}

impl SourceFile {
    fn snapshot_all(store: &ModifierStore) -> Vec<Self> {
        store
            .sources()
            .map(|(id, expires_at, modifiers)| Self {
                id: id.get(),
                expires_at,
                modifiers: modifiers.iter().map(ModifierFile::from).collect(),
            })
            .collect()
    }

    fn restore_all(sources: Vec<Self>, label: &'static str) -> Result<ModifierStore, SaveError> {
        require_sorted(&sources, label, |source| source.id)?;
        let mut store = ModifierStore::default();
        for source in sources {
            let id = ModifierSourceId::new(source.id);
            let entries = source
                .modifiers
                .into_iter()
                .map(|modifier| modifier.restore(id))
                .collect();
            store
                .insert_source(id, source.expires_at, entries)
                .map_err(SaveError::InvalidModifier)?;
        }
        Ok(store)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModifierFile {
    key: String,
    value: i64,
    kind: KindFile,
}

impl From<&Modifier> for ModifierFile {
    fn from(modifier: &Modifier) -> Self {
        Self {
            key: modifier.key.as_str().into(),
            value: modifier.value.raw(),
            kind: match modifier.kind {
                ModifierKind::Additive => KindFile::Additive,
                ModifierKind::Multiplicative => KindFile::Multiplicative,
            },
        }
    }
}

impl ModifierFile {
    pub(super) fn key(&self) -> &str {
        &self.key
    }

    pub(super) fn restore(self, source: ModifierSourceId) -> Modifier {
        Modifier {
            source,
            key: ModKey::new(self.key),
            value: Fixed::from_raw(self.value),
            kind: match self.kind {
                KindFile::Additive => ModifierKind::Additive,
                KindFile::Multiplicative => ModifierKind::Multiplicative,
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KindFile {
    Additive,
    Multiplicative,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduledFile {
    due: u64,
    country: u32,
    source: u32,
    #[serde(deserialize_with = "required_optional")]
    expires_at: Option<u64>,
    modifier: ModifierFile,
}

impl ScheduledFile {
    fn restore(self) -> Result<ScheduledEffect, SaveError> {
        let source = ModifierSourceId::new(self.source);
        let modifier = self.modifier.restore(source);
        let duration_days = self
            .expires_at
            .map(|expires_at| {
                expires_at
                    .checked_sub(self.due)
                    .and_then(NonZeroU64::new)
                    .ok_or(SaveError::InvalidScheduledExpiry {
                        due: self.due,
                        expires_at,
                    })
            })
            .transpose()?;
        Ok(ScheduledEffect {
            due: Tick::new(self.due),
            country: CountryId::new(self.country),
            source,
            effect: Effect::AddModifier {
                key: modifier.key,
                value: modifier.value,
                kind: modifier.kind,
                duration_days,
            },
        })
    }
}

fn require_sorted<T, K: Ord>(
    records: &[T],
    label: &'static str,
    key: impl Fn(&T) -> K,
) -> Result<(), SaveError> {
    if records
        .windows(2)
        .any(|pair| key(&pair[0]) >= key(&pair[1]))
    {
        return Err(SaveError::UnorderedRecords(label));
    }
    Ok(())
}

fn required_optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn present_polity<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PolityFile>, D::Error> {
    PolityFile::deserialize(deserializer).map(Some)
}

fn present_military<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<WarStateFile>, D::Error> {
    WarStateFile::deserialize(deserializer).map(Some)
}
