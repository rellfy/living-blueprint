use vello::kurbo::{Affine, Point, Rect, Size, Vec2};

const MIN_ZOOM: f64 = 0.0002;
const MAX_ZOOM: f64 = 6.0;
const SMOOTHING_RATE: f64 = 16.0;
const INERTIA_DECAY_RATE: f64 = 7.0;
const MIN_INERTIA_SPEED: f64 = 8.0;
const SETTLED_ZOOM_RATIO: f64 = 0.0005;
const FIT_MARGIN_RATIO: f64 = 0.88;

pub struct Camera {
    center: Point,
    zoom: f64,
    target_center: Point,
    target_zoom: f64,
    zoom_anchor: Option<ZoomAnchor>,
    screen_velocity: Vec2,
}

#[derive(Clone, Copy)]
struct ZoomAnchor {
    screen_point: Point,
    world_point: Point,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            center: Point::ZERO,
            zoom: 0.05,
            target_center: Point::ZERO,
            target_zoom: 0.05,
            zoom_anchor: None,
            screen_velocity: Vec2::ZERO,
        }
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn center(&self) -> Point {
        self.center
    }

    pub fn restore(&mut self, center: Point, zoom: f64) {
        self.center = center;
        self.target_center = center;
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.target_zoom = self.zoom;
        self.zoom_anchor = None;
        self.screen_velocity = Vec2::ZERO;
    }

    pub fn update(&mut self, delta_time: f64, viewport: Size) {
        let blend = 1.0 - (-SMOOTHING_RATE * delta_time).exp();
        self.zoom = (self.zoom.ln() + (self.target_zoom.ln() - self.zoom.ln()) * blend).exp();
        match self.zoom_anchor {
            Some(anchor) => self.follow_anchor(anchor, viewport),
            None => self.center = self.center.lerp(self.target_center, blend),
        }
        self.apply_inertia(delta_time);
        self.release_settled_anchor();
    }

    pub fn zoom_at(&mut self, screen_point: Point, factor: f64, viewport: Size) {
        let world_point = self.screen_to_world(screen_point, viewport);
        self.target_zoom = (self.target_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.zoom_anchor = Some(ZoomAnchor {
            screen_point,
            world_point,
        });
        self.screen_velocity = Vec2::ZERO;
    }

    pub fn pan_by_screen(&mut self, screen_delta: Vec2) {
        self.center -= screen_delta / self.zoom;
        self.target_center = self.center;
        self.target_zoom = self.zoom;
        self.zoom_anchor = None;
        self.screen_velocity = Vec2::ZERO;
    }

    pub fn release_with_velocity(&mut self, screen_velocity: Vec2) {
        self.screen_velocity = screen_velocity;
    }

    pub fn fly_to(&mut self, world_rect: Rect, viewport: Size) {
        self.target_zoom = fit_zoom(world_rect, viewport);
        self.target_center = world_rect.center();
        self.zoom_anchor = None;
        self.screen_velocity = Vec2::ZERO;
    }

    pub fn jump_to(&mut self, world_rect: Rect, viewport: Size) {
        self.fly_to(world_rect, viewport);
        self.zoom = self.target_zoom;
        self.center = self.target_center;
    }

    pub fn jump_to_at_most(&mut self, world_rect: Rect, viewport: Size, max_zoom: f64) {
        self.jump_to(world_rect, viewport);
        self.zoom = self.zoom.min(max_zoom);
        self.target_zoom = self.zoom;
    }

    pub fn transform(&self, viewport: Size) -> Affine {
        Affine::translate(viewport.to_vec2() * 0.5)
            * Affine::scale(self.zoom)
            * Affine::translate(-self.center.to_vec2())
    }

    pub fn screen_to_world(&self, screen_point: Point, viewport: Size) -> Point {
        self.center + (screen_point.to_vec2() - viewport.to_vec2() * 0.5) / self.zoom
    }

    pub fn visible_world_rect(&self, viewport: Size) -> Rect {
        let half_extent = viewport.to_vec2() * 0.5 / self.zoom;
        Rect::from_points(self.center - half_extent, self.center + half_extent)
    }

    fn follow_anchor(&mut self, anchor: ZoomAnchor, viewport: Size) {
        let screen_offset = anchor.screen_point.to_vec2() - viewport.to_vec2() * 0.5;
        self.center = anchor.world_point - screen_offset / self.zoom;
        self.target_center = self.center;
    }

    fn apply_inertia(&mut self, delta_time: f64) {
        if self.screen_velocity.hypot() < MIN_INERTIA_SPEED {
            self.screen_velocity = Vec2::ZERO;
            return;
        }
        self.center -= self.screen_velocity * delta_time / self.zoom;
        self.target_center = self.center;
        self.screen_velocity *= (-INERTIA_DECAY_RATE * delta_time).exp();
    }

    fn release_settled_anchor(&mut self) {
        let is_settled = (self.zoom / self.target_zoom - 1.0).abs() < SETTLED_ZOOM_RATIO;
        if is_settled {
            self.zoom_anchor = None;
        }
    }
}

fn fit_zoom(world_rect: Rect, viewport: Size) -> f64 {
    let horizontal = viewport.width / world_rect.width().max(1.0);
    let vertical = viewport.height / world_rect.height().max(1.0);
    (horizontal.min(vertical) * FIT_MARGIN_RATIO).clamp(MIN_ZOOM, MAX_ZOOM)
}
