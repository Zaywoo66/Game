use strategy_sim::{Date, ProvinceKind};

use crate::{advance_autoplay, new_autoplay};

/// One year of a headless AI run. Timing fields measure the local host and
/// deliberately do not participate in the deterministic state comparison.
#[derive(Clone, Debug)]
pub struct SoakYear {
    pub seed: u64,
    pub year: u32,
    pub ticks: u64,
    pub country_count: usize,
    pub largest_land_provinces: usize,
    pub total_land_provinces: usize,
    pub median_tick_ns: u128,
    pub p95_tick_ns: u128,
    pub peak_tick_ns: u128,
    pub state_hash: [u8; 32],
}

impl SoakYear {
    pub fn largest_share_basis_points(&self) -> usize {
        self.largest_land_provinces.saturating_mul(10_000) / self.total_land_provinces.max(1)
    }

    pub fn tsv(&self) -> String {
        let hash = self
            .state_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.seed,
            self.year,
            self.ticks,
            self.country_count,
            self.largest_land_provinces,
            self.total_land_provinces,
            self.largest_share_basis_points(),
            self.median_tick_ns,
            self.p95_tick_ns,
            self.peak_tick_ns,
            hash
        )
    }
}

pub const SOAK_TSV_HEADER: &str = "seed\tyear\tticks\tcountries\tlargest_land_provinces\ttotal_land_provinces\tlargest_share_basis_points\tmedian_tick_ns\tp95_tick_ns\tpeak_tick_ns\tstate_hash";

/// Run each seed from the authored opening, using the existing daily invariant
/// checks. A failure includes the seed/year context and stops the report.
pub fn run_ai_soak(years: u32, seeds: &[u64]) -> Result<Vec<SoakYear>, String> {
    if years == 0 || seeds.is_empty() {
        return Err("soak requires a positive year count and at least one seed".into());
    }
    let mut rows = Vec::new();
    for &seed in seeds {
        let mut game = new_autoplay(seed)?;
        let total_land_provinces = game
            .province_graph()
            .iter()
            .filter(|province| province.kind == ProvinceKind::Land)
            .count();
        if total_land_provinces == 0 {
            return Err("soak map has no land provinces".into());
        }
        for _ in 0..years {
            let year = game
                .time()
                .date()
                .components()
                .0
                .checked_add(1)
                .ok_or("soak year overflow")?;
            let end = Date::new(year, 1, 1)
                .map_err(|error| format!("invalid soak end date: {error:?}"))?;
            let summary = advance_autoplay(&mut game, end)
                .map_err(|error| format!("seed {seed}, year {year}: {error}"))?;
            rows.push(SoakYear {
                seed,
                year,
                ticks: summary.ticks,
                country_count: summary.country_count,
                largest_land_provinces: summary.largest_country,
                total_land_provinces,
                median_tick_ns: summary.median_tick_ns,
                p95_tick_ns: summary.p95_tick_ns,
                peak_tick_ns: summary.peak_tick_ns,
                state_hash: summary.final_hash,
            });
        }
    }
    Ok(rows)
}
