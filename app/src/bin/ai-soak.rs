use std::{env, process::ExitCode};

use strategy_app::{run_ai_soak, SOAK_TSV_HEADER};

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
    let years = args
        .next()
        .ok_or("usage: ai-soak <years> <seed>...")?
        .parse::<u32>()
        .map_err(|_| "years must be a positive integer")?;
    let seeds = args
        .map(|seed| seed.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "seeds must be unsigned integers")?;
    println!("{SOAK_TSV_HEADER}");
    for row in run_ai_soak(years, &seeds)? {
        println!("{}", row.tsv());
    }
    Ok(())
}
