//! Content loading and validation.

mod events;
mod ideas;
mod polity_format;
mod provinces;
mod saves;
mod scenarios;
mod technologies;
mod war_format;
mod yaml_tokens;

pub use events::{
    load_events_from_dir, parse_event_yaml, ConditionalAiWeight, EventDefinition, EventLoadError,
    EventOption,
};
pub use ideas::{load_ideas_from_dir, parse_ideas_yaml, IdeaDefinition, IdeaLoadError};
pub use provinces::{load_provinces, parse_provinces, ProvinceLoadError};
pub use saves::{load_game, save_game, state_hash, SaveError};
pub use scenarios::{load_scenario, load_scenario_from_reader, ScenarioLoadError};
pub use technologies::{
    load_technologies_from_dir, parse_technology_branch_yaml, TechnologyDefinition,
    TechnologyLoadError, TechnologyPosition,
};

fn is_content_key(key: &str) -> bool {
    key.starts_with(|ch: char| ch.is_ascii_lowercase())
        && key.split('_').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

/// With `#[serde(default)]`, the field may be omitted but cannot be null.
fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
