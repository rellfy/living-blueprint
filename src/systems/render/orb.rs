use crate::resources::agent_session::Orb;
use crate::systems::render::FrameContext;
use crate::systems::render::palette::{ORB_CORE, ORB_GLOW};
use vello::Scene;
use vello::kurbo::{Affine, Circle, Rect, Stroke};
use vello::peniko::Fill;

const CORE_RADIUS_PX: f64 = 9.0;
const GLOW_RADIUS_PX: f64 = 26.0;
const GLOW_STD_DEV_PX: f64 = 14.0;
const RING_RADIUS_PX: f64 = 16.0;
const PULSE_SPEED: f64 = 2.4;

pub fn draw_orb(scene: &mut Scene, orb: &Orb, frame: &FrameContext) {
    if orb.visibility <= 0.01 {
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
        ORB_GLOW.multiply_alpha(0.55 * alpha),
        glow_radius,
        GLOW_STD_DEV_PX * scale,
    );
    let ring_progress = (orb.phase * PULSE_SPEED * 0.5).fract();
    scene.stroke(
        &Stroke::new(1.5 * scale),
        Affine::IDENTITY,
        ORB_GLOW.multiply_alpha(alpha * (1.0 - ring_progress) as f32),
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
