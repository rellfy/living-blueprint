use std::path::PathBuf;
use vello::kurbo::Vec2;

#[derive(Default)]
pub struct DragState {
    pub dragged: Option<DraggedNode>,
}

pub struct DraggedNode {
    pub path: PathBuf,
    pub grab_offset: Vec2,
}
