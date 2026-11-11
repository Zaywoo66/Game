use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    marker::PhantomData,
    path::Path,
};

use serde::{de, Deserialize, Deserializer};
use strategy_sim::{
    AiProfileKey, AiThresholds, AiWeights, CharacterId, CharacterState, CharacterStats, Controller,
    CountryId, CountryPolity, CountryState, Date, EconomyState, FiscalPolicy, Fixed, GameState,
    ModifierSourceId, ModifierStore, PlayerId, PolityState, ProvinceGraph, ProvinceKind, SimTime,
    Speed, SuccessionState,
};

use crate::{
    polity_format::{ProfileFile, RulesFile, StatsFile, ThresholdsFile, WeightsFile},
    saves::ModifierFile,
};

const SCENARIO_VERSION: u32 = 1;
type DateFields = (u32, u8, u8);
type TraitModifiers = BTreeMap<String, (ModifierSourceId, Vec<strategy_sim::Modifier>)>;

#[derive(Debug)]
pub enum ScenarioLoadError {
    Io {
        path: String,
        error: io::Error,
    },
    Json {
        path: String,
        error: serde_json::Error,
    },
    Invalid(String),
}

impl fmt::Display for ScenarioLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{path}: {error}"),
            Self::Json { path, error } => write!(f, "{path}: {error}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ScenarioLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Json { error, .. } => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

type Result<T> = std::result::Result<T, ScenarioLoadError>;

fn invalid(message: impl Into<String>) -> ScenarioLoadError {
    ScenarioLoadError::Invalid(message.into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioFile {
    schema_version: u32,
    start_date: DateFields,
    seed: u64,
    countries: Vec<String>,
    characters: Vec<String>,
    #[serde(deserialize_with = "unique_map")]
    human_slots: BTreeMap<String, u32>,
    rules: RulesFile,
    #[serde(deserialize_with = "unique_map")]
    traits: BTreeMap<String, Vec<ModifierFile>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountryFile {
    schema_version: u32,
    tag: String,
    name: String,
    color: [u8; 3],
    capital: String,
    ai_profile: String,
    start: CountryStart,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountryStart {
    ruler: String,
    heir: Option<String>,
    provinces: Vec<String>,
    treasury: i64,
    tax_rate: i64,
    monthly_spending: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharacterFile {
    schema_version: u32,
    key: String,
    country: String,
    name: String,
    dynasty: String,
    birth: DateFields,
    death: Option<DateFields>,
    #[serde(with = "StatsFile")]
    stats: CharacterStats,
    traits: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileContentFile {
    schema_version: u32,
    key: String,
    #[serde(with = "WeightsFile")]
    weights: AiWeights,
    #[serde(with = "ThresholdsFile")]
    thresholds: AiThresholds,
    preferred_tax_rate: i64,
    preferred_monthly_spending: i64,
}

/// Loads a scenario file in `data/scenarios/` with its referenced content.
pub fn load_scenario(graph: ProvinceGraph, scenario_path: impl AsRef<Path>) -> Result<GameState> {
    let path = scenario_path.as_ref();
    let scenario_key = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|_| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .ok_or_else(|| invalid("scenario path must be scenarios/<year>.json"))?;
    let root = path
        .parent()
        .filter(|parent| parent.file_name().is_some_and(|name| name == "scenarios"))
        .and_then(Path::parent)
        .ok_or_else(|| invalid("scenario path must be inside a scenarios directory"))?;
    load_scenario_from_reader(graph, scenario_key, |relative| {
        fs::read(root.join(relative))
    })
}

/// Loads the same scenario from embedded assets or a filesystem resolver.
/// The reader receives only validated logical paths relative to the data root.
pub fn load_scenario_from_reader(
    graph: ProvinceGraph,
    scenario_key: &str,
    mut read: impl FnMut(&str) -> io::Result<Vec<u8>>,
) -> Result<GameState> {
    let year = scenario_key
        .parse::<u32>()
        .ok()
        .filter(|year| *year > 0 && year.to_string() == scenario_key)
        .ok_or_else(|| invalid("scenario key must be a positive decimal year"))?;
    let path = format!("scenarios/{scenario_key}.json");
    let scenario: ScenarioFile = read_file(&mut read, &path)?;
    check_version(&path, scenario.schema_version)?;
    let date = parse_date(scenario.start_date, "scenario start_date")?;
    if scenario.start_date.0 != year {
        return Err(invalid("scenario start year does not match the file key"));
    }
    let countries = key_ids(&scenario.countries, "country", CountryId::new)?;
    if countries.is_empty() {
        return Err(invalid("scenario has no countries"));
    }
    let characters = key_ids(&scenario.characters, "character", CharacterId::new)?;
    validate_slots(&scenario.human_slots, &countries)?;
    check_key(&scenario.rules.income_modifier, "income modifier")?;

    let traits = build_traits(scenario.traits)?;
    let mut character_states = BTreeMap::new();
    for (key, &id) in &characters {
        let path = format!("characters/{key}.json");
        let file: CharacterFile = read_file(&mut read, &path)?;
        check_version(&path, file.schema_version)?;
        require_file_key(&path, &file.key, key)?;
        check_key(&file.dynasty, "dynasty")?;
        check_name(&file.name, &path)?;
        let country = *countries
            .get(file.country.as_str())
            .ok_or_else(|| invalid(format!("{path}: unknown country {}", file.country)))?;
        let birth = parse_date(file.birth, &format!("{path} birth"))?;
        let death = file
            .death
            .map(|parts| parse_date(parts, &format!("{path} death")))
            .transpose()?;
        if death.is_some_and(|death| death <= birth) {
            return Err(invalid(format!("{path}: death must be after birth")));
        }

        let mut modifier_store = ModifierStore::default();
        let mut trait_keys = BTreeSet::new();
        for key in file.traits {
            let (source, modifiers) = traits
                .get(&key)
                .ok_or_else(|| invalid(format!("{path}: unknown trait {key}")))?;
            if !trait_keys.insert(key) {
                return Err(invalid(format!("{path}: duplicate character trait")));
            }
            modifier_store
                .insert_source(*source, None, modifiers.clone())
                .map_err(|error| {
                    invalid(format!("{path}: invalid trait modifier bundle: {error:?}"))
                })?;
        }
        character_states.insert(
            id,
            CharacterState {
                name: file.name,
                country,
                dynasty: file.dynasty,
                birth,
                death,
                stats: file.stats,
                traits: trait_keys,
                modifiers: modifier_store,
            },
        );
    }

    let mut country_states = BTreeMap::new();
    let mut country_polities = BTreeMap::new();
    let mut claimed = BTreeSet::new();
    let mut profiles = BTreeMap::new();
    for (tag, &id) in &countries {
        let path = format!("countries/{tag}.json");
        let file: CountryFile = read_file(&mut read, &path)?;
        check_version(&path, file.schema_version)?;
        require_file_key(&path, &file.tag, tag)?;
        check_name(&file.name, &path)?;
        check_key(&file.ai_profile, "AI profile")?;
        if !profiles.contains_key(&file.ai_profile) {
            let profile_path = format!("ai/profiles/{}.json", file.ai_profile);
            let profile: ProfileContentFile = read_file(&mut read, &profile_path)?;
            check_version(&profile_path, profile.schema_version)?;
            require_file_key(&profile_path, &profile.key, &file.ai_profile)?;
            let (key, profile) = ProfileFile {
                key: profile.key,
                weights: profile.weights,
                thresholds: profile.thresholds,
                preferred_tax_rate: profile.preferred_tax_rate,
                preferred_monthly_spending: profile.preferred_monthly_spending,
            }
            .restore();
            profiles.insert(key, profile);
        }
        let profile_key = AiProfileKey::new(file.ai_profile);
        let controller = match scenario.human_slots.get(*tag) {
            Some(&slot) => Controller::Human(PlayerId::new(slot)),
            None => Controller::Ai(profile_key.clone()),
        };

        let capital = graph
            .by_key(&file.capital)
            .ok_or_else(|| invalid(format!("{path}: unknown capital {}", file.capital)))?
            .id;
        let mut provinces = BTreeSet::new();
        for key in &file.start.provinces {
            let province = graph
                .by_key(key)
                .ok_or_else(|| invalid(format!("{path}: unknown province {key}")))?;
            if province.kind != ProvinceKind::Land {
                return Err(invalid(format!("{path}: owned province {key} is not land")));
            }
            if !provinces.insert(province.id) || !claimed.insert(province.id) {
                return Err(invalid(format!(
                    "{path}: province {key} claimed more than once"
                )));
            }
        }
        if !provinces.contains(&capital) {
            return Err(invalid(format!(
                "{path}: capital must be owned by its country"
            )));
        }

        let ruler = resolve_character(&characters, &character_states, &file.start.ruler, id, date)?;
        let heir = file
            .start
            .heir
            .as_ref()
            .map(|key| resolve_character(&characters, &character_states, key, id, date))
            .transpose()?;
        if heir == Some(ruler) {
            return Err(invalid(format!(
                "{path}: ruler and heir are the same character"
            )));
        }
        country_states.insert(
            id,
            CountryState::new(controller, BTreeSet::new(), false, ModifierStore::default()),
        );
        country_polities.insert(
            id,
            CountryPolity {
                tag: file.tag,
                name: file.name,
                color: file.color,
                capital,
                provinces,
                economy: EconomyState {
                    treasury: Fixed::from_raw(file.start.treasury),
                    policy: FiscalPolicy {
                        tax_rate: Fixed::from_raw(file.start.tax_rate),
                        monthly_spending: Fixed::from_raw(file.start.monthly_spending),
                    },
                    last_month: None,
                    bankrupt: false,
                },
                succession: SuccessionState {
                    ruler: Some(ruler),
                    heir,
                },
                ai_profile: profile_key,
            },
        );
    }

    GameState::new(
        SimTime::new(date, Speed::One, false),
        scenario.seed,
        graph,
        country_states,
        [],
    )
    .map_err(|error| invalid(format!("{path}: invalid initial state: {error:?}")))?
    .with_polity(PolityState {
        rules: scenario.rules.into(),
        countries: country_polities,
        characters: character_states,
        ai_profiles: profiles,
    })
    .map_err(|error| invalid(format!("{path}: invalid polity: {error:?}")))
}

fn read_file<T: de::DeserializeOwned>(
    read: &mut impl FnMut(&str) -> io::Result<Vec<u8>>,
    path: &str,
) -> Result<T> {
    let bytes = read(path).map_err(|error| ScenarioLoadError::Io {
        path: path.into(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| ScenarioLoadError::Json {
        path: path.into(),
        error,
    })
}

fn check_version(path: &str, version: u32) -> Result<()> {
    if version != SCENARIO_VERSION {
        return Err(invalid(format!(
            "{path}: unsupported schema_version {version}"
        )));
    }
    Ok(())
}

fn require_file_key(path: &str, actual: &str, expected: &str) -> Result<()> {
    if actual != expected {
        return Err(invalid(format!(
            "{path}: key {actual:?} does not match {expected:?}"
        )));
    }
    Ok(())
}

fn check_name(name: &str, path: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(invalid(format!("{path}: name is empty")));
    }
    Ok(())
}

fn check_key(key: &str, field: &str) -> Result<()> {
    if !key.starts_with(|ch: char| ch.is_ascii_lowercase())
        || !key.split('_').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        return Err(invalid(format!("invalid {field} key {key:?}")));
    }
    Ok(())
}

fn key_ids<'a, T>(
    keys: &'a [String],
    label: &str,
    make_id: impl Fn(u32) -> T,
) -> Result<BTreeMap<&'a str, T>> {
    let mut distinct = BTreeSet::new();
    for key in keys {
        check_key(key, label)?;
        if !distinct.insert(key.as_str()) {
            return Err(invalid(format!("duplicate {label} key {key}")));
        }
    }
    distinct
        .into_iter()
        .enumerate()
        .map(|(i, key)| {
            Ok((
                key,
                make_id(u32::try_from(i + 1).map_err(|_| invalid("too many content keys"))?),
            ))
        })
        .collect()
}

fn parse_date((year, month, day): DateFields, label: &str) -> Result<Date> {
    Date::new(year, month, day)
        .map_err(|error| invalid(format!("{label}: invalid date: {error:?}")))
}

fn validate_slots(
    slots: &BTreeMap<String, u32>,
    countries: &BTreeMap<&str, CountryId>,
) -> Result<()> {
    let mut used = BTreeSet::new();
    for (tag, &slot) in slots {
        if !countries.contains_key(tag.as_str()) {
            return Err(invalid(format!(
                "human slot references unknown country {tag}"
            )));
        }
        if !(1..=2).contains(&slot) || !used.insert(slot) {
            return Err(invalid(format!(
                "invalid or duplicate human player slot {slot}"
            )));
        }
    }
    Ok(())
}

fn build_traits(definitions: BTreeMap<String, Vec<ModifierFile>>) -> Result<TraitModifiers> {
    definitions
        .into_iter()
        .enumerate()
        .map(|(i, (key, modifiers))| {
            check_key(&key, "trait")?;
            if modifiers.is_empty() {
                return Err(invalid(format!("trait {key} has no modifiers")));
            }
            let source = ModifierSourceId::new(
                u32::try_from(i + 1).map_err(|_| invalid("too many traits"))?,
            );
            let mut entries = Vec::with_capacity(modifiers.len());
            for modifier in modifiers {
                check_key(modifier.key(), "trait modifier")?;
                entries.push(modifier.restore(source));
            }
            let mut check = ModifierStore::default();
            check
                .insert_source(source, None, entries.clone())
                .map_err(|error| {
                    invalid(format!("trait {key}: invalid modifier bundle: {error:?}"))
                })?;
            Ok((key, (source, entries)))
        })
        .collect()
}

fn resolve_character(
    keys: &BTreeMap<&str, CharacterId>,
    characters: &BTreeMap<CharacterId, CharacterState>,
    key: &str,
    country: CountryId,
    date: Date,
) -> Result<CharacterId> {
    let id = *keys
        .get(key)
        .ok_or_else(|| invalid(format!("unknown ruler/heir {key}")))?;
    let state = &characters[&id];
    if state.country != country {
        return Err(invalid(format!(
            "ruler/heir {key} belongs to another country"
        )));
    }
    if state.birth > date || state.death.is_some_and(|death| death <= date) {
        return Err(invalid(format!(
            "ruler/heir {key} is not alive at the start"
        )));
    }
    Ok(id)
}

fn unique_map<'de, D, V>(deserializer: D) -> std::result::Result<BTreeMap<String, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct Visitor<V>(PhantomData<V>);

    impl<'de, V: Deserialize<'de>> de::Visitor<'de> for Visitor<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object with unique keys")
        }

        fn visit_map<A: de::MapAccess<'de>>(
            self,
            mut entries: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = entries.next_entry::<String, V>()? {
                if values.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate object key {key:?}")));
                }
                values.insert(key, value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(Visitor(PhantomData))
}
