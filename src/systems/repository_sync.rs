use crate::State;
use crate::repository::Repository;
use crate::resources::repository_sync::SyncUpdate;
use crate::systems::input::viewport_size;
use crate::systems::view_state::restore_view_state;
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
    if state.repository.is_none() && !restore_view_state(state, &mut repository) {
        let readable_zoom = state.window.scale_factor();
        state
            .camera
            .jump_to_at_most(repository.bounds(), viewport_size(state), readable_zoom);
    }
    focus_working_session(state, &repository, changed_paths);
    state.repository = Some(repository);
    state.load_error = None;
    state.hovered = None;
}

fn focus_working_session(state: &mut State, repository: &Repository, changed_paths: Vec<PathBuf>) {
    let mut working_sessions = state
        .agent_sessions
        .sessions
        .values_mut()
        .filter(|session| session.get_is_working());
    let (Some(session), None) = (working_sessions.next(), working_sessions.next()) else {
        return;
    };
    let changed_file = changed_paths
        .into_iter()
        .rev()
        .find(|path| repository.find_by_path(path).is_some());
    if let Some(path) = changed_file {
        session.focus_path = Some(path);
    }
}
