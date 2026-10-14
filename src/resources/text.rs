use crate::analysis::HighlightSpan;
use crate::repository::layout::{CODE_FONT_SIZE, LINE_HEIGHT};
use parley::{
    FontContext, FontFamily, FontWeight, Layout, LayoutContext, LineHeight, StyleProperty,
};
use std::borrow::Cow;
use vello::peniko::Color;

const MONOSPACE_FAMILIES: &str =
    "JetBrains Mono, Fira Code, Cascadia Code, DejaVu Sans Mono, Menlo, Consolas, monospace";
const INTERFACE_FAMILIES: &str = "Inter, SF Pro Text, Segoe UI, Cantarell, Noto Sans, \
    DejaVu Sans, system-ui, sans-serif";
const ADVANCE_PROBE: &str = "0000000000";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorBrush(pub Color);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontRole {
    Code,
    Interface,
}

pub struct TextEngine {
    font_context: FontContext,
    layout_context: LayoutContext<ColorBrush>,
    char_advance: f64,
}

impl Default for ColorBrush {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}

impl TextEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
            char_advance: CODE_FONT_SIZE * 0.6,
        };
        engine.char_advance = engine.measure_char_advance();
        engine
    }

    pub fn char_advance(&self) -> f64 {
        self.char_advance
    }

    pub fn layout_code(
        &mut self,
        text: &str,
        highlights: &[HighlightSpan],
        color_of: impl Fn(&HighlightSpan) -> Color,
        default_color: Color,
    ) -> Layout<ColorBrush> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, false);
        builder.push_default(family(FontRole::Code));
        builder.push_default(StyleProperty::FontSize(CODE_FONT_SIZE as f32));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
            LINE_HEIGHT as f32,
        )));
        builder.push_default(StyleProperty::Brush(ColorBrush(default_color)));
        for span in highlights
            .iter()
            .filter(|span| span.byte_range.end <= text.len())
        {
            builder.push(
                StyleProperty::Brush(ColorBrush(color_of(span))),
                span.byte_range.clone(),
            );
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout
    }

    pub fn layout_label(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
        weight: f32,
    ) -> Layout<ColorBrush> {
        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, text, 1.0, false);
        builder.push_default(family(role));
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout
    }

    fn measure_char_advance(&mut self) -> f64 {
        let layout = self.layout_label(ADVANCE_PROBE, CODE_FONT_SIZE as f32, FontRole::Code, 400.0);
        match layout.width() > 0.0 {
            true => layout.width() as f64 / ADVANCE_PROBE.len() as f64,
            false => self.char_advance,
        }
    }
}

fn family(role: FontRole) -> StyleProperty<'static, ColorBrush> {
    let families = match role {
        FontRole::Code => MONOSPACE_FAMILIES,
        FontRole::Interface => INTERFACE_FAMILIES,
    };
    StyleProperty::FontFamily(FontFamily::Source(Cow::Borrowed(families)))
}
