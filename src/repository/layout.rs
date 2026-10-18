use crate::arrangement::Arrangement;
use crate::arrangement::resolve::{ArrangedChild, resolve_offsets};
use crate::repository::view::BodyView;
use crate::repository::{Detail, Node, NodeId};
use std::path::Path;
use vello::kurbo::{Point, Rect, Size, Vec2};

pub const CODE_FONT_SIZE: f64 = 14.0;
pub const LINE_HEIGHT: f64 = 20.0;
pub const FILE_PADDING: f64 = 16.0;
pub const FILE_HEADER_HEIGHT: f64 = 50.0;
pub const FILE_HEADER_LABEL_SIZE: f64 = 16.0;
pub const LABEL_CHARACTER_WIDTH_RATIO: f64 = 0.6;
const MIN_FILE_COLUMNS: u32 = 24;
pub const MAX_FILE_COLUMNS: u32 = 100;
const BINARY_FILE_LINES: f64 = 2.0;
pub const MIN_DIRECTORY_HEADER_HEIGHT: f64 = 50.0;
pub const GRID_UNIT: f64 = 25.0;
const TARGET_ASPECT: f64 = 16.0 / 9.0;
const DIRECTORY_HEADER_RATIO: f64 = 0.03;
const SUMMARY_TEXT_SCALE: f64 = 1.5;
const COLLAPSED_TEXT_SCALE: f64 = 2.0;
const MIN_COLLAPSED_COLUMNS: f64 = 12.0;

struct Measurement {
    size: Size,
    header_height: f64,
    child_offsets: Vec<Vec2>,
}

struct ArrangementContext<'a> {
    arrangement: &'a Arrangement,
    pinned: Option<&'a Path>,
}

pub fn apply_layout(
    nodes: &mut [Node],
    char_advance: f64,
    arrangement: &Arrangement,
    pinned: Option<&Path>,
) {
    let context = ArrangementContext {
        arrangement,
        pinned,
    };
    let mut measurements: Vec<Option<Measurement>> = (0..nodes.len()).map(|_| None).collect();
    measure(nodes, NodeId(0), char_advance, &context, &mut measurements);
    place(nodes, NodeId(0), Point::ZERO, &measurements);
}

pub fn content_origin(node: &Node) -> Point {
    node.target_rect.origin() + Vec2::new(GRID_UNIT, node.header_height)
}

pub fn text_scale(node: &Node) -> f64 {
    match (node.get_is_directory(), node.detail) {
        (true, Detail::Collapsed) => SUMMARY_TEXT_SCALE,
        (true, _) => 1.0,
        (false, Detail::Summary) => SUMMARY_TEXT_SCALE,
        (false, Detail::Collapsed) => COLLAPSED_TEXT_SCALE,
        (false, _) => 1.0,
    }
}

pub fn plan_columns(nodes: &mut [Node], char_advance: f64) {
    plan_node(nodes, NodeId(0), char_advance);
}

fn plan_node(nodes: &mut [Node], id: NodeId, char_advance: f64) -> Size {
    if !nodes[id.0].get_is_directory() {
        return snap_size(measure_file(&nodes[id.0], char_advance).size);
    }
    let children = nodes[id.0].children.clone();
    let child_sizes: Vec<Size> = children
        .iter()
        .map(|child| plan_node(nodes, *child, char_advance))
        .collect();
    let column_count = choose_column_count(&child_sizes);
    nodes[id.0].column_count = column_count;
    snap_size(measure_directory(&child_sizes, column_count).size)
}

fn choose_column_count(child_sizes: &[Size]) -> usize {
    (1..=child_sizes.len().max(1))
        .min_by(|first, second| {
            compute_fit_cost(child_sizes, *first).total_cmp(&compute_fit_cost(child_sizes, *second))
        })
        .unwrap_or(1)
}

fn compute_fit_cost(child_sizes: &[Size], column_count: usize) -> f64 {
    let size = measure_directory(child_sizes, column_count).size;
    size.width.max(size.height * TARGET_ASPECT)
}

fn measure(
    nodes: &[Node],
    id: NodeId,
    char_advance: f64,
    context: &ArrangementContext,
    measurements: &mut Vec<Option<Measurement>>,
) -> Size {
    let node = &nodes[id.0];
    let measurement = match (node.get_is_directory(), node.detail) {
        (true, Detail::Expanded) => {
            let child_sizes: Vec<Size> = node
                .children
                .iter()
                .map(|child| measure(nodes, *child, char_advance, context, measurements))
                .collect();
            let child_paths = node
                .children
                .iter()
                .map(|child| nodes[child.0].path.as_path());
            match context.arrangement.check_is_arranged(child_paths) {
                true => measure_arranged_directory(nodes, node, &child_sizes, context),
                false => measure_directory(&child_sizes, node.column_count),
            }
        }
        (true, _) => measure_collapsed_directory(node, char_advance),
        (false, _) => measure_file(node, char_advance),
    };
    let size = snap_size(measurement.size);
    measurements[id.0] = Some(Measurement {
        size,
        ..measurement
    });
    size
}

fn place(nodes: &mut [Node], id: NodeId, origin: Point, measurements: &[Option<Measurement>]) {
    let Some(measurement) = &measurements[id.0] else {
        return;
    };
    let rect = Rect::from_origin_size(origin, measurement.size);
    nodes[id.0].target_rect = rect;
    nodes[id.0].header_height = measurement.header_height;
    let children = nodes[id.0].children.clone();
    if nodes[id.0].get_is_collapsed_directory() {
        children.iter().for_each(|child| hide(nodes, *child, rect));
        return;
    }
    for (child, offset) in children.iter().zip(&measurement.child_offsets) {
        place(nodes, *child, origin + *offset, measurements);
    }
}

fn hide(nodes: &mut [Node], id: NodeId, rect: Rect) {
    nodes[id.0].target_rect = rect;
    let children = nodes[id.0].children.clone();
    children.iter().for_each(|child| hide(nodes, *child, rect));
}

fn measure_file(node: &Node, char_advance: f64) -> Measurement {
    let scale = text_scale(node);
    let body_size = match (node.body(), node.detail) {
        (Some(body), _) => measure_body(body, char_advance),
        (None, Detail::Collapsed) => Size::ZERO,
        (None, _) => Size::new(
            MIN_FILE_COLUMNS as f64 * char_advance,
            BINARY_FILE_LINES * LINE_HEIGHT + FILE_PADDING,
        ),
    } * scale;
    let label_width = node.name.chars().count() as f64
        * FILE_HEADER_LABEL_SIZE
        * LABEL_CHARACTER_WIDTH_RATIO
        * scale;
    let min_columns = match node.detail {
        Detail::Expanded => MIN_FILE_COLUMNS as f64,
        _ => MIN_COLLAPSED_COLUMNS,
    };
    let width = body_size
        .width
        .max(label_width)
        .max(min_columns * char_advance * scale);
    let header_height = snap(FILE_HEADER_HEIGHT * scale);
    Measurement {
        size: Size::new(width + FILE_PADDING * 2.0, header_height + body_size.height),
        header_height,
        child_offsets: Vec::new(),
    }
}

fn measure_collapsed_directory(node: &Node, char_advance: f64) -> Measurement {
    let scale = text_scale(node);
    let header_height = snap(MIN_DIRECTORY_HEADER_HEIGHT * scale);
    let body_size = node
        .body()
        .map_or(Size::ZERO, |body| measure_body(body, char_advance))
        * scale;
    let label_width =
        (node.name.chars().count() + 1) as f64 * header_height * 0.5 * LABEL_CHARACTER_WIDTH_RATIO
            + header_height * 0.45;
    Measurement {
        size: Size::new(
            body_size.width.max(label_width) + FILE_PADDING * 2.0,
            header_height + body_size.height,
        ),
        header_height,
        child_offsets: Vec::new(),
    }
}

fn measure_body(body: BodyView, char_advance: f64) -> Size {
    let columns = body
        .line_spans
        .iter()
        .map(|span| span.length)
        .max()
        .unwrap_or(0)
        .min(MAX_FILE_COLUMNS);
    let line_count = body.line_spans.len().max(1) as f64;
    Size::new(
        columns as f64 * char_advance,
        line_count * LINE_HEIGHT + FILE_PADDING,
    )
}

fn measure_directory(child_sizes: &[Size], column_count: usize) -> Measurement {
    let (content_size, offsets) = pack_grid(child_sizes, column_count);
    wrap_directory_content(content_size, offsets)
}

fn measure_arranged_directory(
    nodes: &[Node],
    node: &Node,
    child_sizes: &[Size],
    context: &ArrangementContext,
) -> Measurement {
    let children: Vec<ArrangedChild> = node
        .children
        .iter()
        .zip(child_sizes)
        .map(|(child, size)| {
            let path = nodes[child.0].path.as_path();
            ArrangedChild {
                size: *size,
                desired: context.arrangement.position(path),
                is_pinned: context.pinned == Some(path),
            }
        })
        .collect();
    let offsets = resolve_offsets(&children, GRID_UNIT);
    let content_size =
        offsets
            .iter()
            .zip(child_sizes)
            .fold(Size::ZERO, |content, (offset, size)| {
                Size::new(
                    content.width.max(offset.x + size.width),
                    content.height.max(offset.y + size.height),
                )
            });
    wrap_directory_content(content_size, offsets)
}

fn wrap_directory_content(content_size: Size, offsets: Vec<Vec2>) -> Measurement {
    let header_height =
        snap((content_size.width * DIRECTORY_HEADER_RATIO).max(MIN_DIRECTORY_HEADER_HEIGHT));
    let padding = GRID_UNIT;
    let content_origin = Vec2::new(padding, header_height);
    Measurement {
        size: Size::new(
            content_size.width + padding * 2.0,
            header_height + content_size.height + padding,
        ),
        header_height,
        child_offsets: offsets
            .into_iter()
            .map(|offset| offset + content_origin)
            .collect(),
    }
}

fn pack_grid(sizes: &[Size], column_count: usize) -> (Size, Vec<Vec2>) {
    let column_count = column_count.clamp(1, sizes.len().max(1));
    let mut placed: Vec<Rect> = Vec::with_capacity(sizes.len());
    for (index, size) in sizes.iter().enumerate() {
        let left = (index % column_count != 0).then(|| placed[index - 1]);
        let above = index.checked_sub(column_count).map(|above| placed[above]);
        let row_start = index - index % column_count;
        let x = left.map_or(0.0, |left| left.x1 + GRID_UNIT);
        let span = (x, x + size.width);
        let y = placed[..row_start]
            .iter()
            .filter(|rect| check_overlaps_horizontally(**rect, span))
            .chain(above.iter())
            .map(|rect| rect.y1 + GRID_UNIT)
            .fold(0.0, f64::max);
        placed.push(Rect::from_origin_size(Point::new(x, y), *size));
    }
    let content_size = placed.iter().fold(Size::ZERO, |size, rect| {
        Size::new(size.width.max(rect.x1), size.height.max(rect.y1))
    });
    let offsets = placed.iter().map(|rect| rect.origin().to_vec2()).collect();
    (content_size, offsets)
}

fn check_overlaps_horizontally(rect: Rect, span: (f64, f64)) -> bool {
    rect.x0 < span.1 + GRID_UNIT && span.0 < rect.x1 + GRID_UNIT
}

fn snap_size(size: Size) -> Size {
    Size::new(snap(size.width), snap(size.height))
}

fn snap(value: f64) -> f64 {
    (value / GRID_UNIT - 1e-6).ceil().max(1.0) * GRID_UNIT
}
