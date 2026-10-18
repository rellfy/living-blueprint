use crate::systems::render::palette::{GRID_MAJOR, GRID_MINOR};
use crate::systems::render::{FrameContext, smoothstep};
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Line, Point, Rect, Shape, Stroke};
use vello::peniko::Color;

const GRID_BASE: f64 = 5.0;
const MIN_MINOR_SPACING_PX: f64 = 14.0;
const FULL_MINOR_SPACING_PX: f64 = 40.0;
const MINOR_STROKE_PX: f64 = 1.0;
const MAJOR_STROKE_PX: f64 = 1.5;

pub fn draw_background(scene: &mut Scene, frame: &FrameContext) {
    draw_grid(scene, frame, frame.visible, GRID_MINOR, GRID_MAJOR);
}

pub fn draw_grid(
    scene: &mut Scene,
    frame: &FrameContext,
    bounds: Rect,
    minor: Color,
    major: Color,
) {
    let bounds = bounds.intersect(frame.visible);
    if bounds.is_zero_area() {
        return;
    }
    let minor_step = compute_minor_step(frame.zoom);
    let major_step = minor_step * GRID_BASE;
    let minor_fade = smoothstep(
        MIN_MINOR_SPACING_PX,
        FULL_MINOR_SPACING_PX,
        minor_step * frame.zoom,
    );
    let minor_color = major.lerp(minor, minor_fade as f32, Default::default());
    let minor_alpha = 0.35 + 0.65 * minor_fade as f32;
    scene.stroke(
        &Stroke::new(MINOR_STROKE_PX * frame.scale_factor),
        Affine::IDENTITY,
        minor_color.multiply_alpha(minor_alpha),
        None,
        &build_grid(frame, bounds, minor_step),
    );
    scene.stroke(
        &Stroke::new(MAJOR_STROKE_PX * frame.scale_factor),
        Affine::IDENTITY,
        major,
        None,
        &build_grid(frame, bounds, major_step),
    );
}

fn compute_minor_step(zoom: f64) -> f64 {
    let min_world_step = MIN_MINOR_SPACING_PX / zoom;
    GRID_BASE.powf((min_world_step.ln() / GRID_BASE.ln()).ceil())
}

fn build_grid(frame: &FrameContext, bounds: Rect, step: f64) -> BezPath {
    let mut path = BezPath::new();
    let screen_bounds = frame.transform.transform_rect_bbox(bounds);
    let mut x = (bounds.x0 / step).ceil() * step;
    while x <= bounds.x1 {
        let screen_x = snap(frame.transform * Point::new(x, 0.0)).x;
        path.extend(
            Line::new((screen_x, screen_bounds.y0), (screen_x, screen_bounds.y1))
                .path_elements(0.1),
        );
        x += step;
    }
    let mut y = (bounds.y0 / step).ceil() * step;
    while y <= bounds.y1 {
        let screen_y = snap(frame.transform * Point::new(0.0, y)).y;
        path.extend(
            Line::new((screen_bounds.x0, screen_y), (screen_bounds.x1, screen_y))
                .path_elements(0.1),
        );
        y += step;
    }
    path
}

fn snap(point: Point) -> Point {
    Point::new(point.x.round() + 0.5, point.y.round() + 0.5)
}
