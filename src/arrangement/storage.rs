use crate::arrangement::{Arrangement, Position};
use crate::blueprint_directory::{
    blueprint_file_path, read_blueprint_file, to_portable_path, write_blueprint_file,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const ARRANGEMENT_FILE_NAME: &str = "arrangement.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ArrangementFile {
    version: u32,
    positions: BTreeMap<String, Position>,
}

pub fn load_arrangement(repository_root: &Path) -> Arrangement {
    let Some(text) = read_blueprint_file(repository_root, ARRANGEMENT_FILE_NAME) else {
        return Arrangement::default();
    };
    match parse_arrangement(&text) {
        Ok(arrangement) => arrangement,
        Err(message) => {
            let path = blueprint_file_path(repository_root, ARRANGEMENT_FILE_NAME);
            eprintln!("ignoring {}: {message}", path.display());
            Arrangement::default()
        }
    }
}

pub fn save_arrangement(repository_root: &Path, arrangement: &Arrangement) -> io::Result<()> {
    write_blueprint_file(
        repository_root,
        ARRANGEMENT_FILE_NAME,
        &serialize_arrangement(arrangement),
    )
}

fn parse_arrangement(text: &str) -> Result<Arrangement, String> {
    let file: ArrangementFile = serde_json::from_str(text).map_err(|error| error.to_string())?;
    if file.version != FORMAT_VERSION {
        return Err(format!("unsupported version {}", file.version));
    }
    let positions = file
        .positions
        .into_iter()
        .map(|(path, position)| (PathBuf::from(path), position))
        .collect();
    Ok(Arrangement { positions })
}

fn serialize_arrangement(arrangement: &Arrangement) -> String {
    let positions = arrangement
        .positions
        .iter()
        .map(|(path, position)| (to_portable_path(path), *position))
        .collect();
    let file = ArrangementFile {
        version: FORMAT_VERSION,
        positions,
    };
    serde_json::to_string_pretty(&file).expect("arrangement serialises to JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use vello::kurbo::Vec2;

    #[test]
    fn round_trips_through_json() {
        let mut arrangement = Arrangement::default();
        arrangement.set_position(Path::new("src/main.rs"), Vec2::new(450.0, 125.0));
        arrangement.set_position(Path::new("src/agent"), Vec2::ZERO);
        let text = serialize_arrangement(&arrangement);
        assert_eq!(parse_arrangement(&text), Ok(arrangement));
    }

    #[test]
    fn writes_one_sorted_entry_per_line() {
        let mut arrangement = Arrangement::default();
        arrangement.set_position(Path::new("b.rs"), Vec2::ZERO);
        arrangement.set_position(Path::new("a.rs"), Vec2::ZERO);
        let text = serialize_arrangement(&arrangement);
        let first = text.find("\"a.rs\"").unwrap();
        let second = text.find("\"b.rs\"").unwrap();
        assert!(first < second);
        assert!(text.starts_with("{\n  \"version\": 1"));
    }

    #[test]
    fn saves_and_loads_from_the_repository_root() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/arrangement-storage");
        let _ = fs::remove_dir_all(&root);
        let mut arrangement = Arrangement::default();
        arrangement.set_position(Path::new("src/main.rs"), Vec2::new(450.0, 125.0));
        save_arrangement(&root, &arrangement).unwrap();
        assert!(root.join(".living-blueprint/arrangement.json").exists());
        assert_eq!(load_arrangement(&root), arrangement);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_file_loads_empty() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/arrangement-missing");
        assert_eq!(load_arrangement(&root), Arrangement::default());
    }

    #[test]
    fn rejects_unknown_versions() {
        let text = r#"{"version": 2, "positions": {}}"#;
        assert!(parse_arrangement(text).is_err());
    }
}
