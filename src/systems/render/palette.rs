use crate::analysis::HighlightKind;
use vello::peniko::Color;

pub const BACKGROUND: Color = Color::from_rgb8(22, 64, 122);
pub const GRID_MINOR: Color = Color::from_rgb8(39, 84, 145);
pub const GRID_MAJOR: Color = Color::from_rgb8(62, 110, 174);
pub const INK: Color = Color::from_rgb8(226, 238, 255);
pub const INK_DIM: Color = Color::from_rgb8(150, 182, 224);
pub const ACCENT: Color = Color::from_rgb8(255, 206, 120);
pub const ORB_CORE: Color = Color::from_rgb8(240, 250, 255);
pub const ORB_GLOW: Color = Color::from_rgb8(120, 220, 255);
pub const PANEL: Color = Color::from_rgb8(12, 36, 74);
pub const SHADOW: Color = Color::from_rgb8(4, 14, 34);
pub const SUCCESS: Color = Color::from_rgb8(140, 235, 170);
pub const FAILURE: Color = Color::from_rgb8(255, 140, 130);

pub fn highlight_color(kind: HighlightKind) -> Color {
    match kind {
        HighlightKind::Keyword => Color::from_rgb8(132, 212, 255),
        HighlightKind::String => Color::from_rgb8(255, 226, 150),
        HighlightKind::Comment => Color::from_rgb8(128, 160, 206),
        HighlightKind::Function => Color::from_rgb8(255, 255, 255),
        HighlightKind::Type => Color::from_rgb8(150, 236, 220),
        HighlightKind::Constant | HighlightKind::Number => Color::from_rgb8(255, 184, 164),
        HighlightKind::Attribute | HighlightKind::Macro => Color::from_rgb8(204, 186, 255),
        HighlightKind::Property => Color::from_rgb8(204, 222, 255),
        HighlightKind::Operator | HighlightKind::Punctuation => Color::from_rgb8(166, 192, 230),
        HighlightKind::Variable => INK,
    }
}
