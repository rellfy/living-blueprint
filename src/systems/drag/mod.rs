use crate::State;
use crate::arrangement::storage::save_arrangement;
use crate::systems::drag::state::DraggedNode;
use crate::systems::input::viewport_size;
use vello::kurbo::Point;

pub mod state;

pub fn begin_drag(state: &mut State) -> bool {
    let world_point = compute_world_pointer(state);
    let Some(repository) = &state.repository else {
        return false;
    };
    let Some(id) = repository.hit_test(world_point) else {
        return false;
    };
    let node = repository.node(id);
    if node.parent.is_none() {
        return false;
    }
    state.drag_state.dragged = Some(DraggedNode {
        path: node.path.clone(),
        grab_offset: world_point - node.target_rect.origin(),
    });
    true
}

pub fn update_drag(state: &mut State) {
    let world_point = compute_world_pointer(state);
    let Some(dragged) = &state.drag_state.dragged else {
        return;
    };
    let Some(repository) = state.repository.as_mut() else {
        return;
    };
    let Some(id) = repository.find_by_path(&dragged.path) else {
        return;
    };
    repository.drag_to(id, world_point - dragged.grab_offset);
    state.hovered = Some(id);
}

pub fn end_drag(state: &mut State) -> bool {
    if state.drag_state.dragged.take().is_none() {
        return false;
    }
    let Some(repository) = state.repository.as_mut() else {
        return true;
    };
    repository.drop_dragged();
    if let Err(error) = save_arrangement(&state.repository_root, repository.arrangement()) {
        eprintln!("failed to save arrangement: {error}");
    }
    true
}

pub fn get_is_dragging_node(state: &State) -> bool {
    state.drag_state.dragged.is_some()
}

fn compute_world_pointer(state: &State) -> Point {
    let viewport = viewport_size(state);
    state
        .camera
        .screen_to_world(state.pointer.position, viewport)
}
