use crate::resources::agent_session::{AgentSession, AgentStatus};
use crate::resources::prompt::Prompt;
use crate::resources::text::ColorBrush;
use crate::resources::text::{FontRole, TextEngine};
use crate::systems::render::FrameContext;
use crate::systems::render::palette::{FAILURE, INK, INK_DIM, ORB_GLOW, PANEL, SHADOW, SUCCESS};
use crate::systems::render::text::draw_layout;
use parley::Layout;
use vello::Scene;
use vello::kurbo::{Affine, Circle, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

const PROMPT_MAX_WIDTH_PX: f64 = 760.0;
const PROMPT_HEIGHT_PX: f64 = 56.0;
const PROMPT_BOTTOM_MARGIN_PX: f64 = 40.0;
const PROMPT_SIDE_MARGIN_PX: f64 = 24.0;
const PROMPT_RADIUS_PX: f64 = 16.0;
const PROMPT_FONT_PX: f32 = 18.0;
const PROMPT_TEXT_INSET_PX: f64 = 22.0;
const PROMPT_SLIDE_PX: f64 = 24.0;
const HINT_FONT_PX: f32 = 13.0;
const STATUS_TOP_MARGIN_PX: f64 = 24.0;
const STATUS_HEIGHT_PX: f64 = 38.0;
const STATUS_FONT_PX: f32 = 14.0;
const STATUS_MAX_CHARACTERS: usize = 90;
const SEND_HINT: &str = "enter to send · esc to clear";

pub struct OverlayPainter<'a> {
    pub scene: &'a mut Scene,
    pub text_engine: &'a mut TextEngine,
    pub frame: &'a FrameContext,
}

impl OverlayPainter<'_> {
    pub fn draw_prompt(&mut self, prompt: &Prompt) {
        let visibility = prompt.visibility;
        if visibility <= 0.01 {
            return;
        }
        let scale = self.frame.scale_factor;
        let viewport = self.frame.viewport;
        let width =
            (viewport.width - PROMPT_SIDE_MARGIN_PX * 2.0 * scale).min(PROMPT_MAX_WIDTH_PX * scale);
        let height = PROMPT_HEIGHT_PX * scale;
        let slide = (1.0 - ease_out(visibility)) * PROMPT_SLIDE_PX * scale;
        let top = viewport.height - PROMPT_BOTTOM_MARGIN_PX * scale - height + slide;
        let rect = Rect::from_origin_size(((viewport.width - width) * 0.5, top), (width, height));
        let alpha = visibility as f32;
        self.draw_panel(rect, PROMPT_RADIUS_PX * scale, alpha);
        let text_rect = rect.inset(-PROMPT_TEXT_INSET_PX * scale);
        let hint = self.text_engine.layout_label(
            SEND_HINT,
            HINT_FONT_PX * scale as f32,
            FontRole::Interface,
            400.0,
        );
        let hint_origin = Point::new(
            text_rect.x1 - hint.width() as f64,
            rect.center().y - hint.height() as f64 * 0.5,
        );
        draw_layout(
            self.scene,
            &hint,
            Affine::translate(hint_origin.to_vec2()),
            INK_DIM.multiply_alpha(alpha * 0.8),
        );
        let text_clip = Rect::new(text_rect.x0, rect.y0, hint_origin.x - 16.0 * scale, rect.y1);
        let layout = self.text_engine.layout_label(
            prompt.text(),
            PROMPT_FONT_PX * scale as f32,
            FontRole::Interface,
            450.0,
        );
        let overflow = (layout.width() as f64 - text_clip.width() + 4.0 * scale).max(0.0);
        let text_origin = Point::new(
            text_clip.x0 - overflow,
            rect.center().y - layout.height() as f64 * 0.5,
        );
        self.scene
            .push_clip_layer(Fill::NonZero, Affine::IDENTITY, &text_clip);
        draw_layout(
            self.scene,
            &layout,
            Affine::translate(text_origin.to_vec2()),
            INK.multiply_alpha(alpha),
        );
        self.draw_caret(&layout, text_origin, prompt.caret_phase, alpha);
        self.scene.pop_layer();
    }

    pub fn draw_status(&mut self, session: &AgentSession) {
        let visibility = session.status_visibility;
        if visibility <= 0.01 {
            return;
        }
        let scale = self.frame.scale_factor;
        let alpha = visibility as f32;
        let message = status_message(session);
        let layout = self.text_engine.layout_label(
            &message,
            STATUS_FONT_PX * scale as f32,
            FontRole::Interface,
            500.0,
        );
        let height = STATUS_HEIGHT_PX * scale;
        let dot_space = height * 0.9;
        let width = layout.width() as f64 + dot_space + height * 0.6;
        let slide = (1.0 - ease_out(visibility)) * -PROMPT_SLIDE_PX * scale;
        let rect = Rect::from_origin_size(
            (
                (self.frame.viewport.width - width) * 0.5,
                STATUS_TOP_MARGIN_PX * scale + slide,
            ),
            (width, height),
        );
        self.draw_panel(rect, height * 0.5, alpha);
        let dot_center = Point::new(rect.x0 + height * 0.55, rect.center().y);
        let pulse = (session.orb.phase * 3.0).sin() * 0.5 + 0.5;
        let dot_color = status_color(session.status);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            dot_color.multiply_alpha(alpha * (0.6 + 0.4 * pulse as f32)),
            None,
            &Circle::new(dot_center, 4.5 * scale),
        );
        let text_origin = Point::new(
            rect.x0 + dot_space,
            rect.center().y - layout.height() as f64 * 0.5,
        );
        draw_layout(
            self.scene,
            &layout,
            Affine::translate(text_origin.to_vec2()),
            INK.multiply_alpha(alpha),
        );
    }

    pub fn draw_notice(&mut self, message: &str, alpha: f32) {
        let scale = self.frame.scale_factor;
        let layout =
            self.text_engine
                .layout_label(message, 20.0 * scale as f32, FontRole::Interface, 500.0);
        let origin = Point::new(
            (self.frame.viewport.width - layout.width() as f64) * 0.5,
            (self.frame.viewport.height - layout.height() as f64) * 0.5,
        );
        draw_layout(
            self.scene,
            &layout,
            Affine::translate(origin.to_vec2()),
            INK.multiply_alpha(alpha),
        );
    }

    fn draw_panel(&mut self, rect: Rect, radius: f64, alpha: f32) {
        let scale = self.frame.scale_factor;
        self.scene.draw_blurred_rounded_rect(
            Affine::translate((0.0, 8.0 * scale)),
            rect,
            SHADOW.multiply_alpha(0.55 * alpha),
            radius,
            18.0 * scale,
        );
        let shape = RoundedRect::from_rect(rect, radius);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            PANEL.multiply_alpha(0.92 * alpha),
            None,
            &shape,
        );
        self.scene.stroke(
            &Stroke::new(1.2 * scale),
            Affine::IDENTITY,
            INK.multiply_alpha(0.55 * alpha),
            None,
            &shape,
        );
    }

    fn draw_caret(
        &mut self,
        layout: &Layout<ColorBrush>,
        text_origin: Point,
        phase: f64,
        alpha: f32,
    ) {
        let blink = match (phase * 2.0).fract() < 0.55 {
            true => 1.0,
            false => 0.15,
        };
        let scale = self.frame.scale_factor;
        let x = text_origin.x + layout.width() as f64 + 2.0 * scale;
        let caret_height = PROMPT_FONT_PX as f64 * 1.15 * scale;
        let center_y = text_origin.y + layout.height() as f64 * 0.5;
        let caret = Rect::new(
            x,
            center_y - caret_height * 0.5,
            x + 2.0 * scale,
            center_y + caret_height * 0.5,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            ORB_GLOW.multiply_alpha(alpha * blink),
            None,
            &caret,
        );
    }
}

fn status_message(session: &AgentSession) -> String {
    let prefix = match session.status {
        AgentStatus::Working => "claude is working",
        AgentStatus::Succeeded => "claude finished",
        AgentStatus::Failed => "claude failed",
        AgentStatus::Idle => "claude",
    };
    let message = match &session.last_message {
        Some(detail) => format!("{prefix} — {detail}"),
        None => prefix.to_string(),
    };
    truncate(&message, STATUS_MAX_CHARACTERS)
}

fn status_color(status: AgentStatus) -> Color {
    match status {
        AgentStatus::Succeeded => SUCCESS,
        AgentStatus::Failed => FAILURE,
        _ => ORB_GLOW,
    }
}

fn truncate(text: &str, max_characters: usize) -> String {
    match text.chars().count() > max_characters {
        true => format!("{}…", text.chars().take(max_characters).collect::<String>()),
        false => text.to_string(),
    }
}

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
