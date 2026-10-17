use crate::State;
use crate::repository::{NodeId, Repository};
use crate::systems::input::viewport_size;
use vello::kurbo::{Point, Rect, Vec2};

#[derive(Default)]
pub struct SemanticZoomState {
    anchor: Option<LayoutAnchor>,
}

#[derive(Clone, Copy)]
struct LayoutAnchor {
    id: NodeId,
    fraction: Vec2,
    screen_point: Point,
}

impl SemanticZoomState {
    pub fn release_anchor(&mut self) {
        self.anchor = None;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn zoom_semantically(state: &mut State, is_expanding: bool) {
    let viewport = viewport_size(state);
    let screen_point = state.pointer.position;
    let world_point = state.camera.screen_to_world(screen_point, viewport);
    let Some(repository) = state.repository.as_mut() else {
        return;
    };
    let Some(target) = repository.hit_test(world_point) else {
        return;
    };
    let rect = repository.node(target).rect;
    if !apply_step(repository, target, is_expanding) {
        return;
    }
    state.semantic_zoom.anchor = Some(LayoutAnchor {
        id: target,
        fraction: compute_fraction(rect, world_point),
        screen_point,
    });
}

pub fn update_semantic_zoom(state: &mut State) {
    let viewport = viewport_size(state);
    let Some(repository) = state.repository.as_mut() else {
        return;
    };
    let is_animating = repository.animate(state.delta_time);
    let Some(anchor) = state.semantic_zoom.anchor else {
        return;
    };
    let rect = repository.node(anchor.id).rect;
    let anchored_point = rect.origin()
        + Vec2::new(
            rect.width() * anchor.fraction.x,
            rect.height() * anchor.fraction.y,
        );
    let current_point = state.camera.screen_to_world(anchor.screen_point, viewport);
    state.camera.shift_world(anchored_point - current_point);
    if !is_animating {
        state.semantic_zoom.anchor = None;
    }
}

fn apply_step(repository: &mut Repository, target: NodeId, is_expanding: bool) -> bool {
    match is_expanding {
        true => repository.expand(target),
        false => repository.compress(target),
    }
}

fn compute_fraction(rect: Rect, point: Point) -> Vec2 {
    let offset = point - rect.origin();
    Vec2::new(
        (offset.x / rect.width().max(1.0)).clamp(0.0, 1.0),
        (offset.y / rect.height().max(1.0)).clamp(0.0, 1.0),
    )
}
