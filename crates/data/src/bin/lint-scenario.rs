use std::{env, path::Path, process::ExitCode};

use strategy_data::{load_provinces, load_scenario};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let [scenario, provinces] = args.as_slice() else {
        eprintln!("Usage: lint-scenario <scenario.json> <provinces.json>");
        return ExitCode::from(2);
    };
    let graph = match load_provinces(provinces) {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("{}: {error}", Path::new(provinces).display());
            return ExitCode::FAILURE;
        }
    };
    match load_scenario(graph, scenario) {
        Ok(state) => {
            let count = state.countries().count();
            println!(
                "{}: valid scenario with {count} countries",
                Path::new(scenario).display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", Path::new(scenario).display());
            ExitCode::FAILURE
        }
    }
}
