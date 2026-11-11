//! Authoritative game orchestration for the desktop host.

mod autoplay;
mod content;
mod host;

pub use autoplay::{advance_autoplay, new_autoplay, AutoplaySummary};
pub use content::load_embedded_scenario;
pub use host::{tick_host, tick_host_with_log, AiDecision, AiDecisionLog};
