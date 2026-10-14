use crate::repository::layout::{FILE_HEADER_HEIGHT, FILE_PADDING, LINE_HEIGHT, MAX_FILE_COLUMNS};
use crate::repository::{Node, NodeId, Repository};
use crate::resources::text::{ColorBrush, FontRole, TextEngine};
use crate::systems::render::palette::{INK, highlight_color};
use parley::Layout;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use vello::kurbo::{BezPath, Rect, Shape};

const CODE_LAYOUT_FRAME_BUDGET: Duration = Duration::from_millis(4);
const LINE_BAR_HEIGHT_RATIO: f64 = 0.55;

#[derive(Default)]
pub struct RenderCache {
    code_layouts: HashMap<NodeId, Layout<ColorBrush>>,
    line_bars: HashMap<NodeId, BezPath>,
    labels: HashMap<LabelKey, Layout<ColorBrush>>,
    code_layout_deadline: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LabelKey {
    text: String,
    role: FontRole,
    weight: u32,
}

pub const LABEL_FONT_SIZE: f32 = 32.0;

impl RenderCache {
    pub fn clear(&mut self) {
        self.code_layouts.clear();
        self.line_bars.clear();
    }

    pub fn migrate(&mut self, previous: &Repository, next: &Repository) {
        let code_layouts = std::mem::take(&mut self.code_layouts);
        for (id, layout) in code_layouts {
            let Some(next_id) = find_unchanged(previous, next, id, false) else {
                continue;
            };
            self.code_layouts.insert(next_id, layout);
        }
        let line_bars = std::mem::take(&mut self.line_bars);
        for (id, bars) in line_bars {
            let Some(next_id) = find_unchanged(previous, next, id, true) else {
                continue;
            };
            self.line_bars.insert(next_id, bars);
        }
    }

    pub fn begin_frame(&mut self) {
        self.code_layout_deadline = None;
    }

    pub fn code_layout(
        &mut self,
        id: NodeId,
        node: &Node,
        text_engine: &mut TextEngine,
    ) -> Option<&Layout<ColorBrush>> {
        if !self.code_layouts.contains_key(&id) {
            let deadline = *self
                .code_layout_deadline
                .get_or_insert_with(|| Instant::now() + CODE_LAYOUT_FRAME_BUDGET);
            if Instant::now() > deadline {
                return None;
            }
            let layout = build_code_layout(node, text_engine)?;
            self.code_layouts.insert(id, layout);
        }
        self.code_layouts.get(&id)
    }

    pub fn line_bars(&mut self, id: NodeId, node: &Node, char_advance: f64) -> &BezPath {
        self.line_bars
            .entry(id)
            .or_insert_with(|| build_line_bars(node, char_advance))
    }

    pub fn label(
        &mut self,
        text: &str,
        role: FontRole,
        weight: f32,
        text_engine: &mut TextEngine,
    ) -> &Layout<ColorBrush> {
        let key = LabelKey {
            text: text.to_string(),
            role,
            weight: weight as u32,
        };
        self.labels
            .entry(key)
            .or_insert_with(|| text_engine.layout_label(text, LABEL_FONT_SIZE, role, weight))
    }
}

fn build_code_layout(node: &Node, text_engine: &mut TextEngine) -> Option<Layout<ColorBrush>> {
    let source_file = node.source_file()?;
    let text = source_file.text.as_deref()?;
    let highlights = source_file
        .analysis
        .as_ref()
        .map(|analysis| analysis.highlights.as_slice())
        .unwrap_or_default();
    Some(text_engine.layout_code(text, highlights, |span| highlight_color(span.kind), INK))
}

fn build_line_bars(node: &Node, char_advance: f64) -> BezPath {
    let mut path = BezPath::new();
    let Some(source_file) = node.source_file() else {
        return path;
    };
    let origin = node.rect.origin();
    let bar_height = LINE_HEIGHT * LINE_BAR_HEIGHT_RATIO;
    for (index, span) in source_file.line_spans.iter().enumerate() {
        let length = span.length.min(MAX_FILE_COLUMNS);
        if length <= span.indent {
            continue;
        }
        let x0 = origin.x + FILE_PADDING + span.indent as f64 * char_advance;
        let x1 = origin.x + FILE_PADDING + length as f64 * char_advance;
        let y0 = origin.y
            + FILE_HEADER_HEIGHT
            + index as f64 * LINE_HEIGHT
            + (LINE_HEIGHT - bar_height) * 0.5;
        path.extend(Rect::new(x0, y0, x1, y0 + bar_height).path_elements(0.1));
    }
    path
}

fn find_unchanged(
    previous: &Repository,
    next: &Repository,
    id: NodeId,
    is_position_sensitive: bool,
) -> Option<NodeId> {
    let previous_node = previous.node(id);
    let next_id = next.find_by_path(&previous_node.path)?;
    let next_node = next.node(next_id);
    let previous_text = previous_node.source_file()?.text.as_deref();
    let next_text = next_node.source_file()?.text.as_deref();
    let is_moved = previous_node.rect.origin() != next_node.rect.origin();
    match previous_text == next_text && !(is_position_sensitive && is_moved) {
        true => Some(next_id),
        false => None,
    }
}
