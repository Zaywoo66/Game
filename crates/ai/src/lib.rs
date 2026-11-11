//! Host-side AI decision entry point.

use strategy_sim::{derive_ai_rng, Command, CountryId, GameState, SeededRng};

/// Fresh context for one country's decision on one simulation tick.
pub struct AiContext {
    rng: SeededRng,
}

impl AiContext {
    pub fn new(state: &GameState, country: CountryId) -> Self {
        Self {
            rng: derive_ai_rng(state.seed(), state.time().tick().get(), country),
        }
    }

    pub fn rng(&mut self) -> &mut SeededRng {
        &mut self.rng
    }
}

/// Reason codes will be introduced with the decisions that use them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {}

/// M1 stub. The host validates any future commands through the simulation.
pub fn decide(
    _state: &GameState,
    _country: CountryId,
    _ctx: &mut AiContext,
) -> Vec<(Command, Reason)> {
    Vec::new()
}
