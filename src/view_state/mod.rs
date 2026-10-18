use crate::blueprint_directory::{
    blueprint_file_path, read_blueprint_file, to_portable_path, write_blueprint_file,
};
use crate::repository::Detail;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

const VIEW_FILE_NAME: &str = "view.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    pub camera: CameraView,
    pub details: BTreeMap<PathBuf, Detail>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraView {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Serialize, Deserialize)]
struct ViewFile {
    version: u32,
    camera: CameraView,
    details: BTreeMap<String, Detail>,
}

pub fn load_view_state(repository_root: &Path) -> Option<ViewState> {
    let text = read_blueprint_file(repository_root, VIEW_FILE_NAME)?;
    match parse_view_state(&text) {
        Ok(view_state) => Some(view_state),
        Err(message) => {
            let path = blueprint_file_path(repository_root, VIEW_FILE_NAME);
            eprintln!("ignoring {}: {message}", path.display());
            None
        }
    }
}

pub fn save_view_state(repository_root: &Path, view_state: &ViewState) -> io::Result<()> {
    write_blueprint_file(
        repository_root,
        VIEW_FILE_NAME,
        &serialize_view_state(view_state),
    )
}

fn parse_view_state(text: &str) -> Result<ViewState, String> {
    let file: ViewFile = serde_json::from_str(text).map_err(|error| error.to_string())?;
    if file.version != FORMAT_VERSION {
        return Err(format!("unsupported version {}", file.version));
    }
    let details = file
        .details
        .into_iter()
        .map(|(path, detail)| (PathBuf::from(path), detail))
        .collect();
    Ok(ViewState {
        camera: file.camera,
        details,
    })
}

fn serialize_view_state(view_state: &ViewState) -> String {
    let details = view_state
        .details
        .iter()
        .map(|(path, detail)| (to_portable_path(path), *detail))
        .collect();
    let file = ViewFile {
        version: FORMAT_VERSION,
        camera: view_state.camera,
        details,
    };
    serde_json::to_string_pretty(&file).expect("view state serialises to JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let view_state = ViewState {
            camera: CameraView {
                x: 120.0,
                y: -40.0,
                zoom: 0.25,
            },
            details: BTreeMap::from([
                (PathBuf::from("src"), Detail::Expanded),
                (PathBuf::from("src/main.rs"), Detail::Outline),
            ]),
        };
        let text = serialize_view_state(&view_state);
        assert!(text.contains("\"outline\""));
        assert_eq!(parse_view_state(&text), Ok(view_state));
    }

    #[test]
    fn rejects_unknown_versions() {
        let text = r#"{"version": 2, "camera": {"x": 0, "y": 0, "zoom": 1}, "details": {}}"#;
        assert!(parse_view_state(text).is_err());
    }
}
