use crate::repository::layout::LINE_HEIGHT;
use crate::repository::{Edge, NodeId, Repository};
use crate::systems::render::palette::{ACCENT, INK};
use crate::systems::render::routes::EdgeRoutes;
use crate::systems::render::{FrameContext, check_overlaps, smoothstep};
use std::collections::HashSet;
use vello::Scene;
use vello::kurbo::{BezPath, Cap, Circle, Join, Point, Rect, Shape, Stroke};
use vello::peniko::Fill;

const EDGE_STROKE_PX: f64 = 1.0;
const HIGHLIGHT_STROKE_PX: f64 = 2.0;
const AMBIENT_ALPHA: f32 = 0.10;
const DIMMED_ALPHA: f32 = 0.035;
const HIGHLIGHT_ALPHA: f32 = 0.9;
const ENDPOINT_RADIUS_PX: f64 = 3.0;
const CODE_FADE_START_PX: f64 = 4.0;
const CODE_FADE_END_PX: f64 = 8.0;

pub fn draw_edges(
    scene: &mut Scene,
    repository: &Repository,
    frame: &FrameContext,
    hovered: Option<NodeId>,
    routes: &mut EdgeRoutes,
) {
    let mut ambient = BezPath::new();
    let mut highlighted = BezPath::new();
    let mut endpoints = BezPath::new();
    let endpoint_radius = ENDPOINT_RADIUS_PX * frame.scale_factor / frame.zoom;
    let focused_file = focus_file(repository, hovered);
    for edge in collect_visible_edges(repository) {
        let Some(route) = routes.route(repository, edge.from, edge.to) else {
            continue;
        };
        if !check_overlaps(compute_bounds(route), frame.visible) {
            continue;
        }
        let is_highlighted =
            focused_file.is_some_and(|file| check_touches(repository, &edge, file));
        let target = match is_highlighted {
            true => &mut highlighted,
            false => &mut ambient,
        };
        append_route(target, route);
        let end = route.last().copied().unwrap_or_default();
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
        &create_sharp_stroke(EDGE_STROKE_PX * stroke_scale),
        frame.transform,
        INK.multiply_alpha(ambient_alpha),
        None,
        &ambient,
    );
    scene.stroke(
        &create_sharp_stroke(HIGHLIGHT_STROKE_PX * stroke_scale),
        frame.transform,
        ACCENT.multiply_alpha(HIGHLIGHT_ALPHA),
        None,
        &highlighted,
    );
    scene.fill(Fill::NonZero, frame.transform, ACCENT, None, &endpoints);
}

pub fn collect_linked_nodes(repository: &Repository, hovered: Option<NodeId>) -> HashSet<NodeId> {
    let Some(file) = focus_file(repository, hovered) else {
        return HashSet::new();
    };
    collect_visible_edges(repository)
        .into_iter()
        .filter_map(|edge| {
            match (
                repository.check_is_within(edge.from, file),
                repository.check_is_within(edge.to, file),
            ) {
                (true, false) => Some(edge.to),
                (false, true) => Some(edge.from),
                _ => None,
            }
        })
        .collect()
}

fn focus_file(repository: &Repository, hovered: Option<NodeId>) -> Option<NodeId> {
    hovered.filter(|id| !repository.node(*id).get_is_expanded_directory())
}

fn check_touches(repository: &Repository, edge: &Edge, file: NodeId) -> bool {
    repository.check_is_within(edge.from, file) || repository.check_is_within(edge.to, file)
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

fn compute_bounds(route: &[Point]) -> Rect {
    route
        .iter()
        .fold(Rect::from_points(route[0], route[0]), |bounds, point| {
            bounds.union_pt(*point)
        })
}

fn append_route(path: &mut BezPath, route: &[Point]) {
    let Some((first, rest)) = route.split_first() else {
        return;
    };
    path.move_to(*first);
    rest.iter().for_each(|point| path.line_to(*point));
}

fn create_sharp_stroke(width: f64) -> Stroke {
    Stroke::new(width)
        .with_join(Join::Miter)
        .with_caps(Cap::Butt)
}
