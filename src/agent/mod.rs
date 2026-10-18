use std::path::{Path, PathBuf};

pub mod process;
pub mod transcript;

pub const PATH_INPUT_KEYS: &[&str] = &["file_path", "path", "notebook_path"];

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Started,
    Prompt(String),
    Text(String),
    ToolUse {
        tool: String,
        paths: Vec<PathBuf>,
    },
    Finished {
        is_success: bool,
        summary: Option<String>,
    },
    Failed(String),
}

pub fn make_relative(path: &Path, working_directory: &Path) -> PathBuf {
    match path.strip_prefix(working_directory) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => path.to_path_buf(),
    }
}
