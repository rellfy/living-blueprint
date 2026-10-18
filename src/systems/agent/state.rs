use crate::agent::process::AgentProcess;
use crate::agent::transcript::watcher::TranscriptWatcher;
use std::collections::HashSet;
use std::path::Path;

pub struct AgentState {
    pub process: Option<AgentProcess>,
    pub spawned_session_ids: HashSet<String>,
    pub transcript_watcher: TranscriptWatcher,
}

impl AgentState {
    pub fn new(repository_root: &Path) -> Self {
        Self {
            process: None,
            spawned_session_ids: HashSet::new(),
            transcript_watcher: TranscriptWatcher::start(repository_root.to_path_buf()),
        }
    }
}
