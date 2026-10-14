use std::collections::VecDeque;
use std::time::{Duration, Instant};
use vello::kurbo::{Point, Vec2};

const DRAG_THRESHOLD: f64 = 4.0;
const VELOCITY_WINDOW: Duration = Duration::from_millis(80);
const MIN_VELOCITY_SPAN_SECONDS: f64 = 0.012;
const MAX_RELEASE_IDLE: Duration = Duration::from_millis(60);
const MAX_RELEASE_SPEED: f64 = 2500.0;
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(350);
const DOUBLE_CLICK_DISTANCE: f64 = 6.0;

pub struct Pointer {
    pub position: Point,
    press_position: Option<Point>,
    is_dragging: bool,
    samples: VecDeque<(Instant, Point)>,
    last_click: Option<(Instant, Point)>,
}

pub enum PointerRelease {
    Click,
    DoubleClick(Point),
    Drag(Vec2),
    None,
}

impl Pointer {
    pub fn new() -> Self {
        Self {
            position: Point::ZERO,
            press_position: None,
            is_dragging: false,
            samples: VecDeque::new(),
            last_click: None,
        }
    }

    pub fn move_to(&mut self, position: Point) -> Option<Vec2> {
        let delta = position - self.position;
        self.position = position;
        let press_position = self.press_position?;
        self.record_sample(position);
        self.is_dragging = self.is_dragging || (position - press_position).hypot() > DRAG_THRESHOLD;
        match self.is_dragging {
            true => Some(delta),
            false => None,
        }
    }

    pub fn press(&mut self) {
        self.press_position = Some(self.position);
        self.is_dragging = false;
        self.samples.clear();
        self.record_sample(self.position);
    }

    pub fn release(&mut self) -> PointerRelease {
        let was_pressed = self.press_position.take().is_some();
        let was_dragging = std::mem::take(&mut self.is_dragging);
        match (was_pressed, was_dragging) {
            (false, _) => PointerRelease::None,
            (true, false) => self.register_click(),
            (true, true) => PointerRelease::Drag(self.compute_release_velocity()),
        }
    }

    pub fn get_is_dragging(&self) -> bool {
        self.is_dragging
    }

    fn register_click(&mut self) -> PointerRelease {
        let now = Instant::now();
        let is_double_click = self.last_click.is_some_and(|(time, position)| {
            now.duration_since(time) < DOUBLE_CLICK_INTERVAL
                && (self.position - position).hypot() < DOUBLE_CLICK_DISTANCE
        });
        match is_double_click {
            true => {
                self.last_click = None;
                PointerRelease::DoubleClick(self.position)
            }
            false => {
                self.last_click = Some((now, self.position));
                PointerRelease::Click
            }
        }
    }

    fn record_sample(&mut self, position: Point) {
        let now = Instant::now();
        self.samples.push_back((now, position));
        while self
            .samples
            .front()
            .is_some_and(|(time, _)| now.duration_since(*time) > VELOCITY_WINDOW)
        {
            self.samples.pop_front();
        }
    }

    fn compute_release_velocity(&self) -> Vec2 {
        let (Some((first_time, first_position)), Some((last_time, last_position))) =
            (self.samples.front(), self.samples.back())
        else {
            return Vec2::ZERO;
        };
        if last_time.elapsed() > MAX_RELEASE_IDLE {
            return Vec2::ZERO;
        }
        let span = last_time.duration_since(*first_time).as_secs_f64();
        if span < MIN_VELOCITY_SPAN_SECONDS {
            return Vec2::ZERO;
        }
        let velocity = (*last_position - *first_position) / span;
        let speed = velocity.hypot();
        match speed > MAX_RELEASE_SPEED {
            true => velocity * (MAX_RELEASE_SPEED / speed),
            false => velocity,
        }
    }
}
