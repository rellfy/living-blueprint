use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const BLUEPRINT_DIRECTORY_NAME: &str = ".living-blueprint";

pub fn read_blueprint_file(repository_root: &Path, file_name: &str) -> Option<String> {
    fs::read_to_string(blueprint_file_path(repository_root, file_name)).ok()
}

pub fn write_blueprint_file(repository_root: &Path, file_name: &str, text: &str) -> io::Result<()> {
    let path = blueprint_file_path(repository_root, file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{text}\n"))
}

pub fn blueprint_file_path(repository_root: &Path, file_name: &str) -> PathBuf {
    repository_root
        .join(BLUEPRINT_DIRECTORY_NAME)
        .join(file_name)
}

pub fn check_is_blueprint_path(path: &Path) -> bool {
    path.components().next() == Some(Component::Normal(BLUEPRINT_DIRECTORY_NAME.as_ref()))
}

pub fn to_portable_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_blueprint_paths() {
        assert!(check_is_blueprint_path(Path::new(
            ".living-blueprint/arrangement.json"
        )));
        assert!(!check_is_blueprint_path(Path::new("src/main.rs")));
    }

    #[test]
    fn portable_paths_use_forward_slashes() {
        assert_eq!(to_portable_path(Path::new("src/main.rs")), "src/main.rs");
    }
}
