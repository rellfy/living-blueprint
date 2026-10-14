use crate::repository::Repository;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(250);
const IGNORED_DIRECTORY_NAMES: [&str; 3] = [".git", "target", "node_modules"];

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
        let directories: HashSet<PathBuf> = repository.directory_paths().collect();
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
            self.changed_paths.extend(relevant_paths);
            self.last_change_time = Some(Instant::now());
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
            let _ = sender.send(Repository::load(&root, char_advance));
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

fn check_is_ignored(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => {
            IGNORED_DIRECTORY_NAMES.contains(&name.to_string_lossy().as_ref())
        }
        _ => false,
    })
}
