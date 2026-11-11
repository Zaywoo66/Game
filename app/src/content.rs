use std::io::{Error, ErrorKind};

use strategy_data::{load_scenario_from_reader, parse_provinces};
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
    load_scenario_from_reader(graph, "1440", |path| bytes_at(path).map(<[u8]>::to_vec))
        .map_err(|error| error.to_string())
}
