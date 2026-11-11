use crate::CountryId;

/// A SplitMix64 generator with its entire state stored in this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Current generator state. Restoring with this value resumes the sequence.
    pub const fn state(self) -> u64 {
        self.state
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix(self.state)
    }
}

/// Derives an AI RNG without advancing the simulation RNG. A country gets
/// the same stream for a given game seed and tick, independent of call order.
pub fn derive_ai_rng(seed: u64, tick: u64, country: CountryId) -> SeededRng {
    const AI_DOMAIN: u64 = u64::from_le_bytes(*b"AI RNG01");
    let game = mix(seed ^ AI_DOMAIN);
    let turn = mix(game ^ tick);
    SeededRng::new(mix(turn ^ u64::from(country.get())))
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
