use std::{env, process::ExitCode};

use strategy_app::{advance_autoplay, new_autoplay};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let ticks = args
        .next()
        .ok_or("usage: ai-smoke <ticks> <seed>...")?
        .parse::<u64>()
        .map_err(|_| "ticks must be a positive integer")?;
    let seeds = args
        .map(|seed| seed.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "seeds must be unsigned integers")?;
    if ticks == 0 || seeds.is_empty() {
        return Err("usage: ai-smoke <ticks> <seed>...".into());
    }
    for seed in seeds {
        let mut game = new_autoplay(seed)?;
        let end = game
            .time()
            .date()
            .checked_add_days(ticks)
            .ok_or("date overflow")?;
        let summary = advance_autoplay(&mut game, end)?;
        let hash = summary
            .final_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!(
            "seed={} ticks={} wars_per_decade={:?} treaties={} cessions={} countries={} largest_country={} median_tick_ns={} state_hash={}",
            summary.seed, summary.ticks, summary.wars_per_decade, summary.treaties,
            summary.cessions, summary.country_count, summary.largest_country,
            summary.median_tick_ns, hash
        );
    }
    Ok(())
}
