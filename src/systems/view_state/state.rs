use crate::view_state::ViewState;

#[derive(Default)]
pub struct ViewStateSync {
    pub last_saved: Option<ViewState>,
    pub seconds_since_check: f64,
}
