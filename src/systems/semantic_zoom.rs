use crate::State;
use crate::repository::{NodeId, Repository};
use crate::systems::input::viewport_size;

pub fn zoom_semantically(state: &mut State, is_expanding: bool) {
    let viewport = viewport_size(state);
    let world_point = state
        .camera
        .screen_to_world(state.pointer.position, viewport);
    let Some(repository) = state.repository.as_mut() else {
        return;
    };
    let Some(target) = repository.hit_test(world_point) else {
        return;
    };
    apply_step(repository, target, is_expanding);
}

pub fn update_semantic_zoom(state: &mut State) {
    let Some(repository) = state.repository.as_mut() else {
        return;
    };
    repository.animate(state.delta_time);
}

fn apply_step(repository: &mut Repository, target: NodeId, is_expanding: bool) -> bool {
    match is_expanding {
        true => repository.expand(target),
        false => repository.compress(target),
    }
}
