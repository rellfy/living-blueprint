use derive_getters::Getters;
use glam::Vec2;

const MIN_ZOOM: f32 = 0.05;
const MAX_ZOOM: f32 = 20.0;

/// 2D camera over the map.
#[derive(Getters)]
pub struct Camera {
    #[getter(copy)]
    position: Vec2,
    #[getter(copy)]
    zoom: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }
}
