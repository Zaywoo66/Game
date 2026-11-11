use std::{collections::BTreeMap, fmt, num::NonZeroU32, num::NonZeroU64};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use strategy_sim::{
    AiProfileKey, Controller, CountryId, CountryState, Date, Effect, Fixed, GameState, InvalidDate,
    InvalidSpeed, MapIdentity, MapIdentityMismatch, ModKey, Modifier, ModifierInsertError,
    ModifierKind, ModifierSourceId, ModifierStore, PlayerId, PolityError, ProvinceGraph,
    ScheduleError, ScheduledEffect, SeededRng, SimTime, Speed, Tick, TimeError, TriggerView,
};

use crate::provinces::decode_hash;

mod polity;

use polity::PolityFile;

const LEGACY_VERSION: u32 = 1;
const POLITY_VERSION: u32 = 2;
const STATE_HASH_DOMAIN_V1: &[u8] = b"strategy:game-state:v1\0";
const STATE_HASH_DOMAIN_V2: &[u8] = b"strategy:game-state:v2\0";

#[derive(Debug)]
pub enum SaveError {
    Json(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    VersionPolityMismatch(u32),
    InvalidPolity(PolityError),
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
        schema_version: if state.polity().is_some() {
            POLITY_VERSION
        } else {
            LEGACY_VERSION
        },
        map_version: identity.version,
        map_hash,
        state: StateFile::from(state),
    })
    .map_err(SaveError::Json)
}

/// Loads a save against an already validated map. The static graph is not in the save.
pub fn load_game(bytes: &[u8], graph: ProvinceGraph) -> Result<GameState, SaveError> {
    let save: SaveFile<&RawValue> = serde_json::from_slice(bytes).map_err(SaveError::Json)?;
    if !matches!(save.schema_version, LEGACY_VERSION | POLITY_VERSION) {
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
    if state.polity.is_some() != (save.schema_version == POLITY_VERSION) {
        return Err(SaveError::VersionPolityMismatch(save.schema_version));
    }
    state.restore(graph)
}

/// SHA-256 of the canonical state, including its map identity and save version.
pub fn state_hash(state: &GameState) -> Result<[u8; 32], SaveError> {
    let mut hash = Sha256::new();
    hash.update(if state.polity().is_some() {
        STATE_HASH_DOMAIN_V2
    } else {
        STATE_HASH_DOMAIN_V1
    });
    hash.update(save_game(state)?);
    Ok(hash.finalize().into())
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_polity"
    )]
    polity: Option<PolityFile>,
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
            polity: state.polity().map(PolityFile::from),
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
        if let Some(polity) = self.polity {
            base.with_polity(polity.restore()?)
                .map_err(SaveError::InvalidPolity)
        } else {
            Ok(base)
        }
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
