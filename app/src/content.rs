use std::io::{Error, ErrorKind};
use std::{collections::BTreeMap, str};

use strategy_data::{load_scenario_from_reader, parse_event_yaml, parse_provinces};
use strategy_sim::GameState;

include!(concat!(env!("OUT_DIR"), "/strategy_content.rs"));

fn bytes_at(path: &str) -> std::io::Result<&'static [u8]> {
    CONTENT
        .binary_search_by_key(&path, |&(key, _)| key)
        .map(|index| CONTENT[index].1)
        .map_err(|_| Error::new(ErrorKind::NotFound, format!("missing scenario file {path}")))
}

/// Loads the playable Western Europe test scenario from packaged data.
pub fn load_embedded_scenario() -> Result<GameState, String> {
    let map = bytes_at("map/western_europe/provinces.json").map_err(|error| error.to_string())?;
    let graph = parse_provinces(map).map_err(|error| error.to_string())?;
    let state = load_scenario_from_reader(graph, "1440", |path| bytes_at(path).map(<[u8]>::to_vec))
        .map_err(|error| error.to_string())?;
    let mut events = BTreeMap::new();
    for &(path, source) in CONTENT.iter().filter(|(path, _)| path.starts_with("events/")) {
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
        state.with_events(events).map_err(|error| format!("invalid event catalog: {error:?}"))
    }
}
