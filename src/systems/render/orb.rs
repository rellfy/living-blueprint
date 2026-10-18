use crate::repository::Repository;
use crate::resources::agent_sessions::{AgentSession, AgentSessions, AgentStatus};
use crate::systems::render::FrameContext;
use crate::systems::render::palette::{ORB_CORE, ORB_GLOWS};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use vello::Scene;
use vello::kurbo::{Affine, Circle, Point, Rect, Stroke, Vec2};
use vello::peniko::{Color, Fill};

const CORE_RADIUS_PX: f64 = 9.0;
const GLOW_RADIUS_PX: f64 = 26.0;
const GLOW_STD_DEV_PX: f64 = 14.0;
const RING_RADIUS_PX: f64 = 16.0;
const PULSE_SPEED: f64 = 2.4;
const FOLLOW_RATE: f64 = 2.2;
const FADE_RATE: f64 = 4.0;
const LINGER_SECONDS: f64 = 1.5;
const WANDER_SPEED: f64 = 0.6;
const WANDER_RATIO: f64 = 0.18;
const HOME_HEIGHT_PX: f64 = 44.0;
const HOME_SPACING_PX: f64 = 56.0;
const HOME_WANDER_PX: f64 = 8.0;
const ROOT_LABEL_INSET_RATIO: f64 = 0.45;
const MIN_VISIBILITY: f64 = 0.01;
const PHASE_SPREAD_SECONDS: u64 = 1000;

#[derive(Default)]
pub struct Orbs {
    orbs: HashMap<String, Orb>,
    spawned_count: usize,
}

pub struct Orb {
    pub position: Point,
    pub visibility: f64,
    pub phase: f64,
    pub glow: Color,
    pub slot: usize,
    pub is_placed: bool,
}

enum OrbAnchor {
    Focus(Rect),
    Home(Point),
}

impl Orbs {
    pub fn update(
        &mut self,
        sessions: &AgentSessions,
        repository: Option<&Repository>,
        frame: &FrameContext,
    ) {
        for session_id in sessions.sessions.keys() {
            if self.orbs.contains_key(session_id) {
                continue;
            }
            let glow = ORB_GLOWS[self.spawned_count % ORB_GLOWS.len()];
            let slot = self.find_free_slot();
            self.spawned_count += 1;
            self.orbs.insert(
                session_id.clone(),
                Orb::new(spread_phase(session_id), glow, slot),
            );
        }
        let home_origin = compute_home_origin(repository, frame);
        for (session_id, orb) in &mut self.orbs {
            let session = sessions.sessions.get(session_id);
            let anchor = match session.and_then(|session| find_focus_rect(session, repository)) {
                Some(rect) => OrbAnchor::Focus(rect),
                None => OrbAnchor::Home(compute_home(home_origin, orb.slot, frame)),
            };
            let is_shown = session.is_some_and(check_is_shown);
            orb.animate(is_shown, anchor, frame);
        }
        self.orbs.retain(|session_id, orb| {
            sessions.sessions.contains_key(session_id) || orb.visibility > MIN_VISIBILITY
        });
    }

    pub fn draw(&self, scene: &mut Scene, frame: &FrameContext) {
        for orb in self.orbs.values() {
            draw_orb(scene, orb, frame);
        }
    }

    fn find_free_slot(&self) -> usize {
        (0..)
            .find(|slot| !self.orbs.values().any(|orb| orb.slot == *slot))
            .expect("a free slot always exists")
    }
}

impl Orb {
    pub fn new(phase: f64, glow: Color, slot: usize) -> Self {
        Self {
            position: Point::ZERO,
            visibility: 0.0,
            phase,
            glow,
            slot,
            is_placed: false,
        }
    }

    fn animate(&mut self, is_shown: bool, anchor: OrbAnchor, frame: &FrameContext) {
        let delta_time = frame.delta_time;
        let target = self.compute_target(anchor, frame);
        self.phase += delta_time;
        self.visibility = approach(self.visibility, is_shown, delta_time);
        if !self.is_placed {
            self.position = target;
            self.is_placed = self.visibility > 0.0;
            return;
        }
        let blend = 1.0 - (-FOLLOW_RATE * delta_time).exp();
        self.position = self.position.lerp(target, blend);
        self.is_placed = self.visibility > MIN_VISIBILITY;
    }

    fn compute_target(&self, anchor: OrbAnchor, frame: &FrameContext) -> Point {
        let phase = self.phase * WANDER_SPEED;
        let wander = Vec2::new(
            phase.sin() + (phase * 0.37).cos() * 0.5,
            (phase * 1.3).cos() * 0.7,
        );
        match anchor {
            OrbAnchor::Focus(rect) => {
                let extent = rect.width().min(rect.height());
                let center = Point::new(rect.center().x, rect.y0 + extent * 0.5);
                center + wander * extent * WANDER_RATIO
            }
            OrbAnchor::Home(home) => home + wander * HOME_WANDER_PX * world_per_pixel(frame),
        }
    }
}

pub fn draw_orb(scene: &mut Scene, orb: &Orb, frame: &FrameContext) {
    if orb.visibility <= MIN_VISIBILITY {
        return;
    }
    let scale = frame.scale_factor;
    let center = frame.transform * orb.position;
    let alpha = orb.visibility as f32;
    let pulse = (orb.phase * PULSE_SPEED).sin() * 0.5 + 0.5;
    let glow_radius = GLOW_RADIUS_PX * scale * (1.0 + 0.15 * pulse);
    let glow_rect = Rect::from_center_size(center, (glow_radius * 2.0, glow_radius * 2.0));
    scene.draw_blurred_rounded_rect(
        Affine::IDENTITY,
        glow_rect,
        orb.glow.multiply_alpha(0.55 * alpha),
        glow_radius,
        GLOW_STD_DEV_PX * scale,
    );
    let ring_progress = (orb.phase * PULSE_SPEED * 0.5).fract();
    scene.stroke(
        &Stroke::new(1.5 * scale),
        Affine::IDENTITY,
        orb.glow
            .multiply_alpha(alpha * (1.0 - ring_progress) as f32),
        None,
        &Circle::new(center, RING_RADIUS_PX * scale * (1.0 + ring_progress * 1.6)),
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        ORB_CORE.multiply_alpha(alpha),
        None,
        &Circle::new(center, CORE_RADIUS_PX * scale * (0.9 + 0.1 * pulse)),
    );
}

fn compute_home_origin(repository: Option<&Repository>, frame: &FrameContext) -> Point {
    let Some(repository) = repository else {
        let margin = 2.0 * HOME_HEIGHT_PX * world_per_pixel(frame);
        return Point::new(frame.visible.x0, frame.visible.y0 + margin);
    };
    let root = repository.node(repository.root_id());
    Point::new(
        root.rect.x0 + root.header_height * ROOT_LABEL_INSET_RATIO,
        root.rect.y0,
    )
}

fn compute_home(origin: Point, slot: usize, frame: &FrameContext) -> Point {
    let offset = Vec2::new(HOME_SPACING_PX * (slot as f64 + 0.5), -HOME_HEIGHT_PX);
    origin + offset * world_per_pixel(frame)
}

fn world_per_pixel(frame: &FrameContext) -> f64 {
    frame.scale_factor / frame.zoom
}

fn check_is_shown(session: &AgentSession) -> bool {
    let is_recent = session.get_is_working() || session.idle_seconds < LINGER_SECONDS;
    is_recent && session.status != AgentStatus::Idle
}

fn find_focus_rect(session: &AgentSession, repository: Option<&Repository>) -> Option<Rect> {
    let repository = repository?;
    let id = repository.find_by_path(session.focus_path.as_ref()?)?;
    Some(repository.node(id).rect)
}

fn spread_phase(session_id: &str) -> f64 {
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    (hasher.finish() % PHASE_SPREAD_SECONDS) as f64
}

fn approach(value: f64, is_shown: bool, delta_time: f64) -> f64 {
    let target = match is_shown {
        true => 1.0,
        false => 0.0,
    };
    value + (target - value) * (1.0 - (-FADE_RATE * delta_time).exp())
}
