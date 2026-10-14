use crate::systems::render::palette::{GRID_MAJOR, GRID_MINOR};
use crate::systems::render::{FrameContext, smoothstep};
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Line, Point, Shape, Stroke};

const GRID_BASE: f64 = 5.0;
const MIN_MINOR_SPACING_PX: f64 = 14.0;
const FULL_MINOR_SPACING_PX: f64 = 40.0;
const MINOR_STROKE_PX: f64 = 1.0;
const MAJOR_STROKE_PX: f64 = 1.5;

pub fn draw_background(scene: &mut Scene, frame: &FrameContext) {
    let minor_step = compute_minor_step(frame.zoom);
    let major_step = minor_step * GRID_BASE;
    let minor_fade = smoothstep(
        MIN_MINOR_SPACING_PX,
        FULL_MINOR_SPACING_PX,
        minor_step * frame.zoom,
    );
    let minor_color = GRID_MAJOR.lerp(GRID_MINOR, minor_fade as f32, Default::default());
    let minor_alpha = 0.35 + 0.65 * minor_fade as f32;
    scene.stroke(
        &Stroke::new(MINOR_STROKE_PX * frame.scale_factor),
        Affine::IDENTITY,
        minor_color.multiply_alpha(minor_alpha),
        None,
        &build_grid(frame, minor_step),
    );
    scene.stroke(
        &Stroke::new(MAJOR_STROKE_PX * frame.scale_factor),
        Affine::IDENTITY,
        GRID_MAJOR,
        None,
        &build_grid(frame, major_step),
    );
}

fn compute_minor_step(zoom: f64) -> f64 {
    let min_world_step = MIN_MINOR_SPACING_PX / zoom;
    GRID_BASE.powf((min_world_step.ln() / GRID_BASE.ln()).ceil())
}

fn build_grid(frame: &FrameContext, step: f64) -> BezPath {
    let mut path = BezPath::new();
    let visible = frame.visible;
    let mut x = (visible.x0 / step).floor() * step;
    while x <= visible.x1 {
        let screen_x = snap(frame.transform * Point::new(x, 0.0)).x;
        path.extend(
            Line::new((screen_x, 0.0), (screen_x, frame.viewport.height)).path_elements(0.1),
        );
        x += step;
    }
    let mut y = (visible.y0 / step).floor() * step;
    while y <= visible.y1 {
        let screen_y = snap(frame.transform * Point::new(0.0, y)).y;
        path.extend(
            Line::new((0.0, screen_y), (frame.viewport.width, screen_y)).path_elements(0.1),
        );
        y += step;
    }
    path
}

fn snap(point: Point) -> Point {
    Point::new(point.x.round() + 0.5, point.y.round() + 0.5)
}
