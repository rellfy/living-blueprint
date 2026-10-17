use crate::State;
use crate::repository::{NodeId, Repository};
use crate::systems::input::viewport_size;
use vello::kurbo::{Point, Rect, Vec2};

pub const PIXELS_PER_STEP: f64 = 60.0;

#[derive(Default)]
pub struct SemanticZoomState {
    pending_steps: f64,
    exhausted: Option<Exhaustion>,
    anchor: Option<LayoutAnchor>,
}

#[derive(Clone, Copy, PartialEq)]
struct Exhaustion {
    id: NodeId,
    is_expanding: bool,
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

    fn take_step(&mut self) -> Option<bool> {
        let is_expanding = self.pending_steps >= 1.0;
        let is_compressing = self.pending_steps <= -1.0;
        match (is_expanding, is_compressing) {
            (true, _) => {
                self.pending_steps -= 1.0;
                Some(true)
            }
            (_, true) => {
                self.pending_steps += 1.0;
                Some(false)
            }
            _ => None,
        }
    }
}

pub fn zoom_semantically(state: &mut State, steps: f64) -> bool {
    let viewport = viewport_size(state);
    let world_point = state
        .camera
        .screen_to_world(state.pointer.position, viewport);
    let screen_point = state.pointer.position;
    let zoom = &mut state.semantic_zoom;
    let Some(repository) = state.repository.as_mut() else {
        return false;
    };
    let Some(target) = repository.hit_test(world_point) else {
        zoom.pending_steps = 0.0;
        return false;
    };
    let exhaustion = Exhaustion {
        id: target,
        is_expanding: steps > 0.0,
    };
    if zoom.exhausted == Some(exhaustion) {
        return false;
    }
    zoom.exhausted = None;
    zoom.pending_steps += steps;
    let rect = repository.node(target).rect;
    while let Some(is_expanding) = zoom.take_step() {
        if !apply_step(repository, target, is_expanding) {
            zoom.pending_steps = 0.0;
            zoom.exhausted = Some(exhaustion);
            return false;
        }
        zoom.anchor = Some(LayoutAnchor {
            id: target,
            fraction: compute_fraction(rect, world_point),
            screen_point,
        });
    }
    true
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
