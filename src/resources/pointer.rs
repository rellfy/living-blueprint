use std::time::Instant;
use vello::kurbo::{Point, Vec2};

const DRAG_THRESHOLD: f64 = 4.0;
const VELOCITY_SMOOTHING: f64 = 0.35;
const MAX_RELEASE_IDLE_SECONDS: f64 = 0.08;

pub struct Pointer {
    pub position: Point,
    press_position: Option<Point>,
    is_dragging: bool,
    velocity: Vec2,
    last_move_time: Instant,
}

pub enum PointerRelease {
    Click(Point),
    Drag(Vec2),
    None,
}

impl Pointer {
    pub fn new() -> Self {
        Self {
            position: Point::ZERO,
            press_position: None,
            is_dragging: false,
            velocity: Vec2::ZERO,
            last_move_time: Instant::now(),
        }
    }

    pub fn move_to(&mut self, position: Point) -> Option<Vec2> {
        let delta = position - self.position;
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.last_move_time)
            .as_secs_f64()
            .max(1e-4);
        self.position = position;
        self.last_move_time = now;
        let press_position = self.press_position?;
        self.is_dragging = self.is_dragging || (position - press_position).hypot() > DRAG_THRESHOLD;
        if !self.is_dragging {
            return None;
        }
        self.velocity = self.velocity.lerp(delta / elapsed, VELOCITY_SMOOTHING);
        Some(delta)
    }

    pub fn press(&mut self) {
        self.press_position = Some(self.position);
        self.is_dragging = false;
        self.velocity = Vec2::ZERO;
    }

    pub fn release(&mut self) -> PointerRelease {
        let was_pressed = self.press_position.take().is_some();
        let was_dragging = std::mem::take(&mut self.is_dragging);
        let idle_seconds = self.last_move_time.elapsed().as_secs_f64();
        match (was_pressed, was_dragging) {
            (false, _) => PointerRelease::None,
            (true, false) => PointerRelease::Click(self.position),
            (true, true) if idle_seconds > MAX_RELEASE_IDLE_SECONDS => {
                PointerRelease::Drag(Vec2::ZERO)
            }
            (true, true) => PointerRelease::Drag(self.velocity),
        }
    }

    pub fn get_is_dragging(&self) -> bool {
        self.is_dragging
    }
}
