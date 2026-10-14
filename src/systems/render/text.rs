use crate::resources::text::ColorBrush;
use parley::{GlyphRun, Layout, PositionedLayoutItem};
use std::ops::Range;
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};
use vello::{Glyph, Scene};

pub fn draw_layout(
    scene: &mut Scene,
    layout: &Layout<ColorBrush>,
    transform: Affine,
    color: Color,
) {
    if color.components[3] <= 0.0 {
        return;
    }
    for line in layout.lines() {
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                draw_glyph_run(scene, &glyph_run, transform, color);
            }
        }
    }
}

pub fn draw_layout_lines(
    scene: &mut Scene,
    layout: &Layout<ColorBrush>,
    transform: Affine,
    alpha: f32,
    line_range: Range<usize>,
) {
    if alpha <= 0.0 {
        return;
    }
    for index in line_range {
        let Some(line) = layout.get(index) else {
            break;
        };
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let color = glyph_run.style().brush.0.multiply_alpha(alpha);
                draw_glyph_run(scene, &glyph_run, transform, color);
            }
        }
    }
}

fn draw_glyph_run(
    scene: &mut Scene,
    glyph_run: &GlyphRun<'_, ColorBrush>,
    transform: Affine,
    color: Color,
) {
    let run = glyph_run.run();
    let coords: Vec<i16> = run
        .normalized_coords()
        .iter()
        .map(|coord| coord.to_bits())
        .collect();
    scene
        .draw_glyphs(run.font())
        .font_size(run.font_size())
        .transform(transform)
        .normalized_coords(&coords)
        .brush(color)
        .hint(false)
        .draw(
            Fill::NonZero,
            glyph_run.positioned_glyphs().map(|glyph| Glyph {
                id: glyph.id,
                x: glyph.x,
                y: glyph.y,
            }),
        );
}
