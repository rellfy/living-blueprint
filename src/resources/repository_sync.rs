use crate::arrangement::storage::load_arrangement;
use crate::blueprint_directory::BLUEPRINT_DIRECTORY_NAME;
use crate::repository::Repository;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(250);
const IGNORED_DIRECTORY_NAMES: [&str; 4] =
    [".git", "target", "node_modules", BLUEPRINT_DIRECTORY_NAME];

pub struct RepositorySync {
    root: PathBuf,
    char_advance: f64,
    watcher: Option<RecommendedWatcher>,
    watch_events: Receiver<notify::Result<Event>>,
    watched_directories: HashSet<PathBuf>,
    pending_load: Option<Receiver<io::Result<Repository>>>,
    last_change_time: Option<Instant>,
    changed_paths: Vec<PathBuf>,
}

pub enum SyncUpdate {
    Loaded(Repository, Vec<PathBuf>),
    Failed(String),
}

impl RepositorySync {
    pub fn start(root: PathBuf, char_advance: f64) -> Self {
        let (sender, watch_events) = channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .ok();
        let mut sync = Self {
            root,
            char_advance,
            watcher,
            watch_events,
            watched_directories: HashSet::new(),
            pending_load: None,
            last_change_time: None,
            changed_paths: Vec::new(),
        };
        sync.begin_load();
        sync
    }

    pub fn get_is_loading(&self) -> bool {
        self.pending_load.is_some()
    }

    pub fn poll(&mut self) -> Option<SyncUpdate> {
        self.drain_watch_events();
        self.begin_load_when_settled();
        self.take_loaded()
    }

    pub fn watch_directories(&mut self, repository: &Repository) {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        let mut directories: HashSet<PathBuf> = repository.directory_paths().collect();
        directories.extend(
            self.watched_directories
                .iter()
                .filter(|directory| check_is_awaiting_files(directory))
                .cloned(),
        );
        for stale in self.watched_directories.difference(&directories) {
            let _ = watcher.unwatch(stale);
        }
        for fresh in directories.difference(&self.watched_directories) {
            let _ = watcher.watch(fresh, RecursiveMode::NonRecursive);
        }
        self.watched_directories = directories;
    }

    fn drain_watch_events(&mut self) {
        while let Ok(event) = self.watch_events.try_recv() {
            let Ok(event) = event else {
                continue;
            };
            if matches!(event.kind, EventKind::Access(_)) {
                continue;
            }
            let relevant_paths: Vec<PathBuf> = event
                .paths
                .iter()
                .filter_map(|path| path.strip_prefix(&self.root).ok())
                .filter(|path| !check_is_ignored(path))
                .map(Path::to_path_buf)
                .collect();
            if relevant_paths.is_empty() {
                continue;
            }
            self.watch_new_directories(&relevant_paths);
            self.changed_paths.extend(relevant_paths);
            self.last_change_time = Some(Instant::now());
        }
    }

    fn watch_new_directories(&mut self, relative_paths: &[PathBuf]) {
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        let new_directories: Vec<PathBuf> = relative_paths
            .iter()
            .map(|path| self.root.join(path))
            .filter(|path| path.is_dir() && !self.watched_directories.contains(path))
            .flat_map(|path| collect_directories(&path))
            .collect();
        for directory in new_directories {
            let _ = watcher.watch(&directory, RecursiveMode::NonRecursive);
            self.watched_directories.insert(directory);
        }
    }

    fn begin_load_when_settled(&mut self) {
        let Some(last_change_time) = self.last_change_time else {
            return;
        };
        if last_change_time.elapsed() < DEBOUNCE || self.pending_load.is_some() {
            return;
        }
        self.last_change_time = None;
        self.begin_load();
    }

    fn begin_load(&mut self) {
        let (sender, receiver) = channel();
        let root = self.root.clone();
        let char_advance = self.char_advance;
        thread::spawn(move || {
            let repository = Repository::load(&root, char_advance).map(|mut repository| {
                repository.set_arrangement(load_arrangement(&root));
                repository
            });
            let _ = sender.send(repository);
        });
        self.pending_load = Some(receiver);
    }

    fn take_loaded(&mut self) -> Option<SyncUpdate> {
        let result = self.pending_load.as_ref()?.try_recv().ok()?;
        self.pending_load = None;
        match result {
            Ok(repository) => Some(SyncUpdate::Loaded(
                repository,
                std::mem::take(&mut self.changed_paths),
            )),
            Err(error) => Some(SyncUpdate::Failed(error.to_string())),
        }
    }
}

fn collect_directories(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return vec![directory.to_path_buf()];
    };
    let descendants = entries
        .flatten()
        .filter(|entry| {
            !IGNORED_DIRECTORY_NAMES.contains(&entry.file_name().to_string_lossy().as_ref())
        })
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .flat_map(|path| collect_directories(&path));
    std::iter::once(directory.to_path_buf())
        .chain(descendants)
        .collect()
}

fn check_is_awaiting_files(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries.flatten().all(|entry| entry.path().is_dir())
}

fn check_is_ignored(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => {
            IGNORED_DIRECTORY_NAMES.contains(&name.to_string_lossy().as_ref())
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const POLL_ATTEMPTS: usize = 60;
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    #[test]
    fn loads_files_created_in_new_directories() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/sync-new-directories");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.rs"), "fn a() {}\n").unwrap();
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .unwrap();
        let root = root.canonicalize().unwrap();
        let mut sync = RepositorySync::start(root.clone(), 8.0);
        wait_until_loaded(&mut sync, Path::new("a.rs"));
        fs::create_dir_all(root.join("new")).unwrap();
        wait_until_loaded(&mut sync, Path::new("a.rs"));
        fs::write(root.join("new/b.rs"), "fn b() {}\n").unwrap();
        wait_until_loaded(&mut sync, Path::new("new/b.rs"));
        let _ = fs::remove_dir_all(&root);
    }

    fn wait_until_loaded(sync: &mut RepositorySync, path: &Path) {
        for _ in 0..POLL_ATTEMPTS {
            if let Some(SyncUpdate::Loaded(repository, _)) = sync.poll() {
                sync.watch_directories(&repository);
                if repository.find_by_path(path).is_some() {
                    return;
                }
            }
            thread::sleep(POLL_INTERVAL);
        }
        panic!("{} was never loaded", path.display());
    }
}
