use std::io::{Error, ErrorKind};
use std::{
    collections::{BTreeMap, BTreeSet},
    str,
};

use strategy_data::{
    load_scenario_from_reader, parse_event_yaml, parse_ideas_yaml, parse_provinces,
    parse_reforms_yaml, parse_technology_branch_yaml, TechnologyDefinition,
};
use strategy_sim::GameState;

include!(concat!(env!("OUT_DIR"), "/strategy_content.rs"));

fn bytes_at(path: &str) -> std::io::Result<&'static [u8]> {
    CONTENT
        .binary_search_by_key(&path, |&(key, _)| key)
        .map(|index| CONTENT[index].1)
        .map_err(|_| Error::new(ErrorKind::NotFound, format!("missing scenario file {path}")))
}

/// Reads the same packaged technology definitions used by the simulation.
/// The layout position is presentation data and is not part of a saved game.
pub fn embedded_technologies() -> Result<BTreeMap<String, TechnologyDefinition>, String> {
    let mut technologies = BTreeMap::new();
    for &(path, source) in CONTENT
        .iter()
        .filter(|(path, _)| path.starts_with("techs/"))
    {
        let yaml = str::from_utf8(source).map_err(|error| format!("{path}: {error}"))?;
        let branch = path
            .trim_start_matches("techs/")
            .trim_end_matches(".yaml")
            .trim_end_matches(".yml");
        for definition in
            parse_technology_branch_yaml(yaml, branch).map_err(|error| error.to_string())?
        {
            if technologies
                .insert(definition.key.clone(), definition)
                .is_some()
            {
                return Err(format!("duplicate packaged technology key in {path}"));
            }
        }
    }
    Ok(technologies)
}

/// Loads the legacy 1440 scenario on the full Europe province graph.
pub fn load_embedded_scenario() -> Result<GameState, String> {
    load_embedded_scenario_for_key("1440", true)
}

/// Loads the modern 2022 scenario on the full Europe province graph.
pub fn load_embedded_modern_scenario() -> Result<GameState, String> {
    load_embedded_scenario_for_key("2022", true)
}

fn load_embedded_scenario_for_key(
    scenario_key: &str,
    load_national_ideas: bool,
) -> Result<GameState, String> {
    let map = bytes_at("map/europe/provinces.json").map_err(|error| error.to_string())?;
    let graph = parse_provinces(map).map_err(|error| error.to_string())?;
    let mut state = load_scenario_from_reader(graph, scenario_key, |path| {
        bytes_at(path).map(<[u8]>::to_vec)
    })
    .map_err(|error| error.to_string())?;
    let technologies: BTreeMap<_, _> = embedded_technologies()?
        .into_iter()
        .map(|(key, definition)| (key, definition.into()))
        .collect();
    if !technologies.is_empty() {
        state = state
            .with_technologies(technologies)
            .map_err(|error| format!("invalid technology catalog: {error:?}"))?;
    }
    if load_national_ideas {
        let mut ideas = BTreeMap::new();
        let active_tags: BTreeSet<_> = state
            .countries()
            .filter_map(|(country, _)| state.country_polity(country))
            .map(|polity| polity.tag.clone())
            .collect();
        for &(path, source) in CONTENT
            .iter()
            .filter(|(path, _)| path.starts_with("ideas/"))
        {
            let country = path
                .trim_start_matches("ideas/")
                .trim_end_matches(".yaml")
                .trim_end_matches(".yml");
            if country != "common" && !active_tags.contains(country) {
                continue;
            }
            let yaml = str::from_utf8(source).map_err(|error| format!("{path}: {error}"))?;
            for definition in
                parse_ideas_yaml(yaml, country).map_err(|error| format!("{path}: {error}"))?
            {
                if ideas
                    .insert(definition.key.clone(), definition.into())
                    .is_some()
                {
                    return Err(format!("duplicate packaged idea key in {path}"));
                }
            }
        }
        if !ideas.is_empty() {
            state = state
                .with_ideas(ideas)
                .map_err(|error| format!("invalid idea catalog: {error:?}"))?;
        }
    }
    if let Some((path, source)) = CONTENT
        .iter()
        .find(|(path, _)| *path == "reforms/governments.yaml")
    {
        let yaml = str::from_utf8(source).map_err(|error| format!("{path}: {error}"))?;
        let governments: BTreeSet<_> = state
            .countries()
            .filter_map(|(country, _)| state.country_polity(country))
            .map(|polity| polity.government.as_str())
            .collect();
        let reforms: BTreeMap<_, _> = parse_reforms_yaml(yaml)
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter(|(_, definition)| governments.contains(definition.government.as_str()))
            .collect();
        if !reforms.is_empty() {
            state = state
                .with_reforms(reforms)
                .map_err(|error| format!("invalid reform catalog: {error:?}"))?;
        }
    }
    let mut events = BTreeMap::new();
    for &(path, source) in CONTENT
        .iter()
        .filter(|(path, _)| path.starts_with("events/"))
    {
        let yaml = str::from_utf8(source).map_err(|error| format!("{path}: {error}"))?;
        let event = parse_event_yaml(yaml).map_err(|error| format!("{path}: {error}"))?;
        let key = event.key.clone();
        if events.insert(key.clone(), event).is_some() {
            return Err(format!("duplicate packaged event key {key}"));
        }
    }
    if events.is_empty() {
        Ok(state)
    } else {
        state
            .with_events(events)
            .map_err(|error| format!("invalid event catalog: {error:?}"))
    }
}
