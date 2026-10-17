use crate::repository::layout::{FILE_PADDING, LINE_HEIGHT, MAX_FILE_COLUMNS};
use crate::repository::{Detail, Node, NodeId, Repository};
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
    code_layouts: HashMap<BodyKey, Layout<ColorBrush>>,
    line_bars: HashMap<BodyKey, BezPath>,
    labels: HashMap<LabelKey, Layout<ColorBrush>>,
    code_layout_deadline: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BodyKey {
    id: NodeId,
    detail: Detail,
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
        self.code_layouts = migrate_entries(std::mem::take(&mut self.code_layouts), previous, next);
        self.line_bars = migrate_entries(std::mem::take(&mut self.line_bars), previous, next);
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
        let key = BodyKey {
            id,
            detail: node.detail,
        };
        if !self.code_layouts.contains_key(&key) {
            let deadline = *self
                .code_layout_deadline
                .get_or_insert_with(|| Instant::now() + CODE_LAYOUT_FRAME_BUDGET);
            if Instant::now() > deadline {
                return None;
            }
            let layout = build_code_layout(node, text_engine)?;
            self.code_layouts.insert(key, layout);
        }
        self.code_layouts.get(&key)
    }

    pub fn line_bars(&mut self, id: NodeId, node: &Node, char_advance: f64) -> &BezPath {
        let key = BodyKey {
            id,
            detail: node.detail,
        };
        self.line_bars
            .entry(key)
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
    let body = node.body()?;
    Some(text_engine.layout_code(
        body.text,
        body.highlights,
        |span| highlight_color(span.kind),
        INK,
    ))
}

fn build_line_bars(node: &Node, char_advance: f64) -> BezPath {
    let mut path = BezPath::new();
    let Some(body) = node.body() else {
        return path;
    };
    let bar_height = LINE_HEIGHT * LINE_BAR_HEIGHT_RATIO;
    for (index, span) in body.line_spans.iter().enumerate() {
        let length = span.length.min(MAX_FILE_COLUMNS);
        if length <= span.indent {
            continue;
        }
        let x0 = FILE_PADDING + span.indent as f64 * char_advance;
        let x1 = FILE_PADDING + length as f64 * char_advance;
        let y0 = index as f64 * LINE_HEIGHT + (LINE_HEIGHT - bar_height) * 0.5;
        path.extend(Rect::new(x0, y0, x1, y0 + bar_height).path_elements(0.1));
    }
    path
}

fn migrate_entries<T>(
    entries: HashMap<BodyKey, T>,
    previous: &Repository,
    next: &Repository,
) -> HashMap<BodyKey, T> {
    entries
        .into_iter()
        .filter_map(|(key, entry)| {
            let next_id = find_unchanged(previous, next, key)?;
            Some((
                BodyKey {
                    id: next_id,
                    detail: key.detail,
                },
                entry,
            ))
        })
        .collect()
}

fn find_unchanged(previous: &Repository, next: &Repository, key: BodyKey) -> Option<NodeId> {
    let previous_node = previous.node(key.id);
    let next_id = next.find_by_path(&previous_node.path)?;
    let next_node = next.node(next_id);
    let previous_text = previous_node.source_file()?.text.as_deref();
    let next_text = next_node.source_file()?.text.as_deref();
    match previous_text == next_text {
        true => Some(next_id),
        false => None,
    }
}
