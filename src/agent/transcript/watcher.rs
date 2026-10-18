use crate::agent::AgentEvent;
use crate::agent::transcript::parse_transcript_line;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{sleep, spawn};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const CONFIG_DIRECTORY_VARIABLE: &str = "CLAUDE_CONFIG_DIR";
const HOME_DIRECTORY_VARIABLES: &[&str] = &["HOME", "USERPROFILE"];
const DEFAULT_CONFIG_DIRECTORY_NAME: &str = ".claude";
const PROJECTS_DIRECTORY_NAME: &str = "projects";
const SUBAGENTS_DIRECTORY_NAME: &str = "subagents";
const TRANSCRIPT_EXTENSION: &str = "jsonl";

pub struct TranscriptWatcher {
    receiver: Receiver<(String, AgentEvent)>,
}

#[derive(Default)]
struct Transcript {
    offset: u64,
    pending: Vec<u8>,
}

impl TranscriptWatcher {
    pub fn start(repository_root: PathBuf) -> Self {
        let (sender, receiver) = channel();
        spawn(move || watch_transcripts(repository_root, sender));
        Self { receiver }
    }

    pub fn poll_events(&self) -> Vec<(String, AgentEvent)> {
        self.receiver.try_iter().collect()
    }
}

fn watch_transcripts(repository_root: PathBuf, sender: Sender<(String, AgentEvent)>) {
    let Some(projects_directory) = find_projects_directory() else {
        return;
    };
    let project_prefix = escape_path(&repository_root);
    let mut transcripts: HashMap<PathBuf, Transcript> =
        list_transcripts(&projects_directory, &project_prefix)
            .into_iter()
            .map(|path| (path.clone(), skip_existing(&path)))
            .collect();
    loop {
        sleep(POLL_INTERVAL);
        for path in list_transcripts(&projects_directory, &project_prefix) {
            let session_id = read_session_id(&path);
            let transcript = transcripts.entry(path.clone()).or_default();
            for line in read_new_lines(&path, transcript) {
                for event in parse_transcript_line(&line, &repository_root) {
                    if sender.send((session_id.clone(), event)).is_err() {
                        return;
                    }
                }
            }
        }
    }
}

fn find_projects_directory() -> Option<PathBuf> {
    let config_directory = env::var_os(CONFIG_DIRECTORY_VARIABLE)
        .map(PathBuf::from)
        .or_else(|| {
            HOME_DIRECTORY_VARIABLES
                .iter()
                .find_map(|variable| env::var_os(variable))
                .map(|home| PathBuf::from(home).join(DEFAULT_CONFIG_DIRECTORY_NAME))
        })?;
    Some(config_directory.join(PROJECTS_DIRECTORY_NAME))
}

fn list_transcripts(projects_directory: &Path, project_prefix: &str) -> Vec<PathBuf> {
    list_directory(projects_directory)
        .into_iter()
        .filter(|path| check_is_project_directory(path, project_prefix))
        .flat_map(|project| {
            let subagents = list_directory(&project)
                .into_iter()
                .filter(|path| path.is_dir())
                .flat_map(|session| list_directory(&session.join(SUBAGENTS_DIRECTORY_NAME)));
            list_directory(&project).into_iter().chain(subagents)
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == TRANSCRIPT_EXTENSION)
        })
        .collect()
}

fn check_is_project_directory(path: &Path, project_prefix: &str) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let is_subdirectory_project = name
        .strip_prefix(project_prefix)
        .is_some_and(|rest| rest.starts_with('-'));
    name == project_prefix || is_subdirectory_project
}

fn read_new_lines(path: &Path, transcript: &mut Transcript) -> Vec<String> {
    let Ok(length) = fs::metadata(path).map(|metadata| metadata.len()) else {
        return Vec::new();
    };
    if length < transcript.offset {
        *transcript = Transcript::default();
    }
    if length == transcript.offset {
        return Vec::new();
    }
    let Ok(bytes) = read_range(path, transcript.offset, length) else {
        return Vec::new();
    };
    transcript.offset += bytes.len() as u64;
    transcript.pending.extend(bytes);
    let Some(end) = transcript.pending.iter().rposition(|byte| *byte == b'\n') else {
        return Vec::new();
    };
    let complete: Vec<u8> = transcript.pending.drain(..=end).collect();
    String::from_utf8_lossy(&complete)
        .lines()
        .map(str::to_string)
        .collect()
}

fn read_range(path: &Path, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(end - start).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn skip_existing(path: &Path) -> Transcript {
    Transcript {
        offset: fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        pending: Vec::new(),
    }
}

fn list_directory(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    entries.flatten().map(|entry| entry.path()).collect()
}

fn read_session_id(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn escape_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|character| match character.is_ascii_alphanumeric() {
            true => character,
            false => '-',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_claude_project_directories() {
        assert_eq!(
            escape_path(Path::new("/run/media/user/git/living-blueprint")),
            "-run-media-user-git-living-blueprint"
        );
    }

    #[test]
    fn matches_root_and_subdirectory_projects_only() {
        let prefix = "-repo";
        assert!(check_is_project_directory(Path::new("/p/-repo"), prefix));
        assert!(check_is_project_directory(
            Path::new("/p/-repo-src"),
            prefix
        ));
        assert!(!check_is_project_directory(
            Path::new("/p/-repository"),
            prefix
        ));
    }
}
