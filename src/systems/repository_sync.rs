use crate::State;
use crate::repository::Repository;
use crate::resources::repository_sync::SyncUpdate;
use crate::systems::input::viewport_size;
use std::path::PathBuf;

pub fn update_repository(state: &mut State) {
    let Some(update) = state.repository_sync.poll() else {
        return;
    };
    match update {
        SyncUpdate::Loaded(repository, changed_paths) => {
            install_repository(state, repository, changed_paths)
        }
        SyncUpdate::Failed(error) => {
            state.load_error = Some(format!("Failed to read repository: {error}"))
        }
    }
}

fn install_repository(state: &mut State, mut repository: Repository, changed_paths: Vec<PathBuf>) {
    state.repository_sync.watch_directories(&repository);
    if let Some(previous) = &state.repository {
        repository.inherit(previous);
        state.render_state.cache.migrate(previous, &repository);
    }
    state.semantic_zoom.reset();
    if state.repository.is_none() {
        let readable_zoom = state.window.scale_factor();
        state
            .camera
            .jump_to_at_most(repository.bounds(), viewport_size(state), readable_zoom);
    }
    if state.agent_session.get_is_working() {
        let changed_file = changed_paths
            .into_iter()
            .rev()
            .find(|path| repository.find_by_path(path).is_some());
        if let Some(path) = changed_file {
            state.agent_session.focus_path = Some(path);
        }
    }
    state.repository = Some(repository);
    state.load_error = None;
    state.hovered = None;
}
