use std::{env, path::Path, process::ExitCode};

use strategy_data::load_provinces;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next().filter(|_| args.next().is_none()) else {
        eprintln!("Usage: lint-provinces <provinces.json>");
        return ExitCode::from(2);
    };

    let path = Path::new(&path);
    match load_provinces(path) {
        Ok(graph) => {
            println!(
                "{}: {} provinces, {}",
                path.display(),
                graph.iter().len(),
                graph.identity()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            ExitCode::FAILURE
        }
    }
}
