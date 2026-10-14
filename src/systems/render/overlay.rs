use crate::resources::agent_feed::{AgentFeed, ENTRY_LIFETIME_SECONDS, FeedEntry, FeedKind};
use crate::resources::prompt::Prompt;
use crate::resources::text::ColorBrush;
use crate::resources::text::{FontRole, TextEngine};
use crate::systems::render::FrameContext;
use crate::systems::render::palette::{
    ACCENT, FAILURE, INK, INK_DIM, ORB_GLOW, PANEL, SHADOW, SUCCESS,
};
use crate::systems::render::text::draw_layout;
use parley::{Affinity, Cursor, Layout, Selection};
use std::ops::Range;
use vello::Scene;
use vello::kurbo::{Affine, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

const PROMPT_MAX_WIDTH_PX: f64 = 760.0;
const PROMPT_HEIGHT_PX: f64 = 56.0;
const PROMPT_MAX_LINES: usize = 8;
const PROMPT_BOTTOM_MARGIN_PX: f64 = 40.0;
const PROMPT_SIDE_MARGIN_PX: f64 = 24.0;
const PROMPT_RADIUS_PX: f64 = 16.0;
const PROMPT_FONT_PX: f32 = 18.0;
const PROMPT_TEXT_INSET_PX: f64 = 22.0;
const PROMPT_SLIDE_PX: f64 = 24.0;
const HINT_FONT_PX: f32 = 13.0;
const FEED_SIDE_MARGIN_PX: f64 = 24.0;
const FEED_BOTTOM_MARGIN_PX: f64 = 24.0;
const FEED_PADDING_PX: f64 = 12.0;
const FEED_GAP_PX: f64 = 8.0;
const FEED_RADIUS_PX: f64 = 10.0;
const FEED_FONT_PX: f32 = 14.0;
const FEED_ACCENT_PX: f64 = 3.0;
const FEED_ACCENT_INSET_PX: f64 = 6.0;
const FEED_SLIDE_RATE: f64 = 12.0;
const FEED_ENTRY_SLIDE_PX: f64 = 18.0;
const FEED_FADE_IN_SECONDS: f64 = 0.35;
const FEED_FADE_OUT_SECONDS: f64 = 1.5;
const SEND_HINT: &str = "enter to send · esc to clear";

pub struct OverlayPainter<'a> {
    pub scene: &'a mut Scene,
    pub text_engine: &'a mut TextEngine,
    pub frame: &'a FrameContext,
}

impl OverlayPainter<'_> {
    pub fn draw_prompt(&mut self, prompt: &mut Prompt) {
        let visibility = prompt.visibility;
        if visibility <= 0.01 {
            return;
        }
        let scale = self.frame.scale_factor;
        let layout = self.layout_prompt_text(prompt.text());
        let expansion = compute_prompt_expansion(&layout);
        let rect = self.compute_prompt_rect(visibility, expansion);
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
            rect.y1 - (PROMPT_HEIGHT_PX * scale + hint.height() as f64) * 0.5,
        );
        draw_layout(
            self.scene,
            &hint,
            Affine::translate(hint_origin.to_vec2()),
            INK_DIM.multiply_alpha(alpha * 0.8),
        );
        let text_clip = Rect::new(text_rect.x0, rect.y0, hint_origin.x - 16.0 * scale, rect.y1);
        let view_height = first_line_height(&layout) + expansion;
        let caret = caret_geometry(&layout, prompt.caret());
        prompt.scroll_x = compute_scroll(
            prompt.scroll_x,
            caret.x0..caret.x0,
            layout.width() as f64,
            text_clip.width() - 4.0 * scale,
        );
        prompt.scroll_y = compute_scroll(
            prompt.scroll_y,
            caret.y0..caret.y1,
            layout.height() as f64,
            view_height,
        );
        let text_origin = Point::new(
            text_clip.x0 - prompt.scroll_x,
            rect.center().y - view_height * 0.5 - prompt.scroll_y,
        );
        self.scene
            .push_clip_layer(Fill::NonZero, Affine::IDENTITY, &text_clip);
        self.draw_selection(&layout, text_origin, prompt, alpha);
        draw_layout(
            self.scene,
            &layout,
            Affine::translate(text_origin.to_vec2()),
            INK.multiply_alpha(alpha),
        );
        self.draw_caret(
            caret.with_origin(text_origin + caret.origin().to_vec2()),
            prompt.caret_phase,
            alpha,
        );
        self.scene.pop_layer();
    }

    pub fn draw_feed(&mut self, feed: &mut AgentFeed, prompt: &Prompt) {
        let prompt_layout = self.layout_prompt_text(prompt.text());
        let prompt_expansion = compute_prompt_expansion(&prompt_layout);
        let scale = self.frame.scale_factor;
        let padding = FEED_PADDING_PX * scale;
        let left = FEED_SIDE_MARGIN_PX * scale;
        let blend = 1.0 - (-FEED_SLIDE_RATE * self.frame.delta_time).exp();
        let widest_card = feed
            .entries
            .iter_mut()
            .map(|entry| self.measure_entry(entry).0)
            .fold(0.0, f64::max)
            + padding * 2.0
            + FEED_ACCENT_PX * scale;
        let mut bottom = self.frame.viewport.height
            - FEED_BOTTOM_MARGIN_PX * scale
            - self.compute_prompt_clearance(
                left + widest_card,
                prompt.visibility,
                prompt_expansion,
            );
        for entry in feed.entries.iter_mut().rev() {
            let (text_width, text_height) = self.measure_entry(entry);
            let card_height = text_height + padding * 2.0;
            let target_top = bottom - card_height;
            let top = match entry.offset_y {
                Some(current) => current + (target_top - current) * blend,
                None => target_top + FEED_ENTRY_SLIDE_PX * scale,
            };
            entry.offset_y = Some(top);
            bottom = target_top - FEED_GAP_PX * scale;
            if top + card_height < 0.0 {
                continue;
            }
            let card = Rect::from_origin_size(
                (left, top),
                (
                    text_width + padding * 2.0 + FEED_ACCENT_PX * scale,
                    card_height,
                ),
            );
            self.draw_feed_card(entry, card, feed_entry_alpha(entry.age_seconds));
        }
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

    fn layout_prompt_text(&mut self, text: &str) -> Layout<ColorBrush> {
        self.text_engine.layout_label(
            text,
            PROMPT_FONT_PX * self.frame.scale_factor as f32,
            FontRole::Interface,
            450.0,
        )
    }

    fn compute_prompt_rect(&self, visibility: f64, expansion: f64) -> Rect {
        let scale = self.frame.scale_factor;
        let viewport = self.frame.viewport;
        let width =
            (viewport.width - PROMPT_SIDE_MARGIN_PX * 2.0 * scale).min(PROMPT_MAX_WIDTH_PX * scale);
        let height = PROMPT_HEIGHT_PX * scale + expansion;
        let slide = (1.0 - ease_out(visibility)) * PROMPT_SLIDE_PX * scale;
        let top = viewport.height - PROMPT_BOTTOM_MARGIN_PX * scale - height + slide;
        Rect::from_origin_size(((viewport.width - width) * 0.5, top), (width, height))
    }

    fn compute_prompt_clearance(
        &self,
        feed_right: f64,
        prompt_visibility: f64,
        prompt_expansion: f64,
    ) -> f64 {
        let prompt_rect = self.compute_prompt_rect(1.0, prompt_expansion);
        if feed_right < prompt_rect.x0 {
            return 0.0;
        }
        let clearance = self.frame.viewport.height - prompt_rect.y0
            + FEED_GAP_PX * self.frame.scale_factor
            - FEED_BOTTOM_MARGIN_PX * self.frame.scale_factor;
        clearance * ease_out(prompt_visibility)
    }

    fn measure_entry(&mut self, entry: &mut FeedEntry) -> (f64, f64) {
        let scale = self.frame.scale_factor;
        let is_stale = entry
            .layout
            .as_ref()
            .is_none_or(|(layout_scale, _)| *layout_scale != scale);
        if is_stale {
            let (_, _, role) = feed_style(entry.kind);
            let layout = self.text_engine.layout_label(
                &entry.text,
                FEED_FONT_PX * scale as f32,
                role,
                450.0,
            );
            entry.layout = Some((scale, layout));
        }
        let (_, layout) = entry.layout.as_ref().expect("feed layout was just built");
        (layout.width() as f64, layout.height() as f64)
    }

    fn draw_feed_card(&mut self, entry: &FeedEntry, card: Rect, alpha: f32) {
        let Some((_, layout)) = entry.layout.as_ref() else {
            return;
        };
        let scale = self.frame.scale_factor;
        let (accent, text_color, _) = feed_style(entry.kind);
        self.draw_panel(card, FEED_RADIUS_PX * scale, alpha * 0.9);
        let accent_bar = RoundedRect::new(
            card.x0 + FEED_ACCENT_INSET_PX * scale,
            card.y0 + FEED_ACCENT_INSET_PX * scale * 2.0,
            card.x0 + (FEED_ACCENT_INSET_PX + FEED_ACCENT_PX) * scale,
            card.y1 - FEED_ACCENT_INSET_PX * scale * 2.0,
            FEED_ACCENT_PX * scale,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            accent.multiply_alpha(alpha),
            None,
            &accent_bar,
        );
        let padding = FEED_PADDING_PX * scale;
        let text_origin = Point::new(
            card.x0 + padding + FEED_ACCENT_PX * scale,
            card.y0 + padding,
        );
        draw_layout(
            self.scene,
            layout,
            Affine::translate(text_origin.to_vec2()),
            text_color.multiply_alpha(alpha),
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

    fn draw_selection(
        &mut self,
        layout: &Layout<ColorBrush>,
        text_origin: Point,
        prompt: &Prompt,
        alpha: f32,
    ) {
        if !prompt.get_has_selection() {
            return;
        }
        let selection = Selection::new(
            Cursor::from_byte_index(layout, prompt.anchor(), Affinity::Downstream),
            Cursor::from_byte_index(layout, prompt.caret(), Affinity::Downstream),
        );
        let transform = Affine::translate(text_origin.to_vec2());
        for (bounds, _) in selection.geometry(layout) {
            let highlight = Rect::new(bounds.x0, bounds.y0, bounds.x1, bounds.y1);
            self.scene.fill(
                Fill::NonZero,
                transform,
                ORB_GLOW.multiply_alpha(alpha * 0.35),
                None,
                &highlight,
            );
        }
    }

    fn draw_caret(&mut self, caret_bounds: Rect, phase: f64, alpha: f32) {
        let blink = match (phase * 2.0).fract() < 0.55 {
            true => 1.0,
            false => 0.15,
        };
        let scale = self.frame.scale_factor;
        let caret_height = PROMPT_FONT_PX as f64 * 1.15 * scale;
        let center_y = caret_bounds.center().y;
        let caret = Rect::new(
            caret_bounds.x0,
            center_y - caret_height * 0.5,
            caret_bounds.x0 + 2.0 * scale,
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

fn caret_geometry(layout: &Layout<ColorBrush>, caret: usize) -> Rect {
    let bounds = Cursor::from_byte_index(layout, caret, Affinity::Downstream).geometry(layout, 0.0);
    Rect::new(bounds.x0, bounds.y0, bounds.x1, bounds.y1)
}

fn compute_prompt_expansion(layout: &Layout<ColorBrush>) -> f64 {
    let line_height = first_line_height(layout);
    let max_expansion = line_height * (PROMPT_MAX_LINES - 1) as f64;
    (layout.height() as f64 - line_height).clamp(0.0, max_expansion)
}

fn first_line_height(layout: &Layout<ColorBrush>) -> f64 {
    layout
        .lines()
        .next()
        .map_or(0.0, |line| line.metrics().line_height as f64)
}

fn compute_scroll(current: f64, caret: Range<f64>, content_size: f64, view_size: f64) -> f64 {
    let view_size = view_size.max(0.0);
    let max_scroll = (content_size - view_size).max(0.0);
    let scroll = current.max(caret.end - view_size).min(caret.start);
    scroll.clamp(0.0, max_scroll)
}

fn feed_entry_alpha(age_seconds: f64) -> f32 {
    let fade_in = (age_seconds / FEED_FADE_IN_SECONDS).clamp(0.0, 1.0);
    let fade_out = ((ENTRY_LIFETIME_SECONDS - age_seconds) / FEED_FADE_OUT_SECONDS).clamp(0.0, 1.0);
    (ease_out(fade_in) * fade_out) as f32
}

fn feed_style(kind: FeedKind) -> (Color, Color, FontRole) {
    match kind {
        FeedKind::Prompt => (ACCENT, INK, FontRole::Interface),
        FeedKind::Message => (ORB_GLOW, INK, FontRole::Interface),
        FeedKind::ToolUse => (INK_DIM, INK_DIM, FontRole::Code),
        FeedKind::Success => (SUCCESS, INK, FontRole::Interface),
        FeedKind::Failure => (FAILURE, INK, FontRole::Interface),
    }
}

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}
