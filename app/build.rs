use std::{env, fs, path::PathBuf};

fn main() {
    tauri_build::build();
    embed_game_data().expect("could not embed scenario data");
}

fn embed_game_data() -> std::io::Result<()> {
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest path")).join("../data");
    let mut paths = vec!["map/western_europe/provinces.json".to_owned()];
    for directory in ["scenarios", "countries", "characters", "ai/profiles"] {
        let path = root.join(directory);
        println!("cargo:rerun-if-changed={}", path.display());
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry.path().extension().is_some_and(|ext| ext == "json")
            {
                paths.push(format!(
                    "{directory}/{}",
                    entry.file_name().to_string_lossy()
                ));
            }
        }
    }
    let events = root.join("events");
    println!("cargo:rerun-if-changed={}", events.display());
    if events.is_dir() {
        for entry in fs::read_dir(events)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
            {
                paths.push(format!("events/{}", entry.file_name().to_string_lossy()));
            }
        }
    }
    paths.sort();
    let mut source = String::from("const CONTENT: &[(&str, &[u8])] = &[\n");
    for key in paths {
        let path = root.join(&key);
        println!("cargo:rerun-if-changed={}", path.display());
        source.push_str(&format!("    ({key:?}, include_bytes!({path:?})),\n"));
    }
    source.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("output path"));
    fs::write(output.join("strategy_content.rs"), source)
}
