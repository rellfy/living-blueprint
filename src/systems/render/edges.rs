use crate::repository::layout::LINE_HEIGHT;
use crate::repository::{Edge, NodeId, Repository};
use crate::systems::render::palette::{ACCENT, INK};
use crate::systems::render::{FrameContext, check_overlaps, smoothstep};
use std::collections::HashSet;
use vello::Scene;
use vello::kurbo::{BezPath, Circle, Point, Rect, Shape, Stroke};
use vello::peniko::Fill;

const EDGE_STROKE_PX: f64 = 1.0;
const HIGHLIGHT_STROKE_PX: f64 = 2.0;
const AMBIENT_ALPHA: f32 = 0.10;
const DIMMED_ALPHA: f32 = 0.035;
const HIGHLIGHT_ALPHA: f32 = 0.9;
const ENDPOINT_RADIUS_PX: f64 = 3.0;
const CONTROL_RATIO: f64 = 0.45;
const CODE_FADE_START_PX: f64 = 4.0;
const CODE_FADE_END_PX: f64 = 8.0;

pub fn draw_edges(
    scene: &mut Scene,
    repository: &Repository,
    frame: &FrameContext,
    hovered: Option<NodeId>,
) {
    let mut ambient = BezPath::new();
    let mut highlighted = BezPath::new();
    let mut endpoints = BezPath::new();
    let endpoint_radius = ENDPOINT_RADIUS_PX * frame.scale_factor / frame.zoom;
    let focused_file = hovered.filter(|id| !repository.node(*id).get_is_expanded_directory());
    for edge in collect_visible_edges(repository) {
        let Some((start, end)) = compute_endpoints(repository, &edge) else {
            continue;
        };
        if !check_overlaps(Rect::from_points(start, end), frame.visible) {
            continue;
        }
        let is_highlighted = focused_file.is_some_and(|file| {
            repository.check_is_within(edge.from, file) || repository.check_is_within(edge.to, file)
        });
        let target = match is_highlighted {
            true => &mut highlighted,
            false => &mut ambient,
        };
        append_curve(target, start, end);
        if is_highlighted {
            endpoints.extend(Circle::new(end, endpoint_radius).path_elements(0.1));
        }
    }
    let code_visibility = smoothstep(
        CODE_FADE_START_PX,
        CODE_FADE_END_PX,
        LINE_HEIGHT * frame.zoom,
    ) as f32;
    let ambient_alpha = match focused_file.is_some() {
        true => DIMMED_ALPHA,
        false => AMBIENT_ALPHA,
    } * (1.0 - code_visibility);
    let stroke_scale = frame.scale_factor / frame.zoom;
    scene.stroke(
        &Stroke::new(EDGE_STROKE_PX * stroke_scale),
        frame.transform,
        INK.multiply_alpha(ambient_alpha),
        None,
        &ambient,
    );
    scene.stroke(
        &Stroke::new(HIGHLIGHT_STROKE_PX * stroke_scale),
        frame.transform,
        ACCENT.multiply_alpha(HIGHLIGHT_ALPHA),
        None,
        &highlighted,
    );
    scene.fill(Fill::NonZero, frame.transform, ACCENT, None, &endpoints);
}

fn collect_visible_edges(repository: &Repository) -> Vec<Edge> {
    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    repository
        .edges()
        .iter()
        .map(|edge| Edge {
            from: repository.visible_ancestor(edge.from),
            to: repository.visible_ancestor(edge.to),
            weight: edge.weight,
        })
        .filter(|edge| edge.from != edge.to && seen.insert((edge.from, edge.to)))
        .collect()
}

fn compute_endpoints(repository: &Repository, edge: &Edge) -> Option<(Point, Point)> {
    let from = repository.node(edge.from).rect;
    let to = repository.node(edge.to).rect;
    if from.is_zero_area() || to.is_zero_area() {
        return None;
    }
    let is_target_right = to.center().x >= from.center().x;
    let start_x = match is_target_right {
        true => from.x1,
        false => from.x0,
    };
    let end_x = match is_target_right {
        true => to.x0,
        false => to.x1,
    };
    Some((
        Point::new(start_x, from.y0 + from.height().min(400.0) * 0.5),
        Point::new(end_x, to.y0 + to.height().min(400.0) * 0.5),
    ))
}

fn append_curve(path: &mut BezPath, start: Point, end: Point) {
    let delta = end - start;
    let direction = match delta.x >= 0.0 {
        true => 1.0,
        false => -1.0,
    };
    let control_offset = direction * (delta.x.abs() * CONTROL_RATIO).max(delta.y.abs() * 0.25);
    path.move_to(start);
    path.curve_to(
        Point::new(start.x + control_offset, start.y),
        Point::new(end.x - control_offset, end.y),
        end,
    );
}
