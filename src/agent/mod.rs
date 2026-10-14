use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum AgentEvent {
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

pub struct AgentProcess {}

impl AgentProcess {
    pub fn spawn(_prompt: &str, _working_directory: &Path) -> Self {
        Self {}
    }

    pub fn poll_events(&mut self) -> Vec<AgentEvent> {
        Vec::new()
    }

    pub fn get_is_running(&self) -> bool {
        false
    }
}
