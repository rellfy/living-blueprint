use crate::State;
use crate::repository::Repository;
use crate::view_state::{CameraView, ViewState, load_view_state, save_view_state};
use vello::kurbo::Point;

pub mod state;

const AUTOSAVE_INTERVAL_SECONDS: f64 = 5.0;

pub fn restore_view_state(state: &mut State, repository: &mut Repository) -> bool {
    let Some(view_state) = load_view_state(&state.repository_root) else {
        return false;
    };
    repository.restore_details(&view_state.details);
    let camera = view_state.camera;
    state
        .camera
        .restore(Point::new(camera.x, camera.y), camera.zoom);
    state.view_state_sync.last_saved = Some(view_state);
    true
}

pub fn update_view_state(state: &mut State) {
    let sync = &mut state.view_state_sync;
    sync.seconds_since_check += state.delta_time;
    if sync.seconds_since_check < AUTOSAVE_INTERVAL_SECONDS {
        return;
    }
    sync.seconds_since_check = 0.0;
    save_current_view_state(state);
}

pub fn save_current_view_state(state: &mut State) {
    let Some(view_state) = capture_view_state(state) else {
        return;
    };
    if state.view_state_sync.last_saved.as_ref() == Some(&view_state) {
        return;
    }
    if let Err(error) = save_view_state(&state.repository_root, &view_state) {
        eprintln!("failed to save view state: {error}");
        return;
    }
    state.view_state_sync.last_saved = Some(view_state);
}

fn capture_view_state(state: &State) -> Option<ViewState> {
    let repository = state.repository.as_ref()?;
    let center = state.camera.center();
    Some(ViewState {
        camera: CameraView {
            x: center.x,
            y: center.y,
            zoom: state.camera.zoom(),
        },
        details: repository.details(),
    })
}
