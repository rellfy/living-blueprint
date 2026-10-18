use crate::repository::view::BodyView;
use crate::repository::{Detail, Node, NodeId};
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
const INDENT: f64 = GRID_UNIT * 2.0;
const DIRECTORY_HEADER_RATIO: f64 = 0.03;
const SUMMARY_TEXT_SCALE: f64 = 1.5;
const COLLAPSED_TEXT_SCALE: f64 = 2.0;
const MIN_COLLAPSED_COLUMNS: f64 = 12.0;

struct Measurement {
    size: Size,
    header_height: f64,
    child_offsets: Vec<Vec2>,
}

pub fn apply_layout(nodes: &mut [Node], char_advance: f64) {
    let mut measurements: Vec<Option<Measurement>> = (0..nodes.len()).map(|_| None).collect();
    measure(nodes, NodeId(0), char_advance, &mut measurements);
    place(nodes, NodeId(0), Point::ZERO, &measurements);
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

fn measure(
    nodes: &[Node],
    id: NodeId,
    char_advance: f64,
    measurements: &mut Vec<Option<Measurement>>,
) -> Size {
    let node = &nodes[id.0];
    let measurement = match (node.get_is_directory(), node.detail) {
        (true, Detail::Expanded) => {
            let child_sizes: Vec<Size> = node
                .children
                .iter()
                .map(|child| measure(nodes, *child, char_advance, measurements))
                .collect();
            measure_directory(&child_sizes)
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

fn measure_directory(child_sizes: &[Size]) -> Measurement {
    let content_width = child_sizes
        .iter()
        .map(|size| size.width)
        .fold(0.0, f64::max);
    let header_height =
        snap((content_width * DIRECTORY_HEADER_RATIO).max(MIN_DIRECTORY_HEADER_HEIGHT));
    let mut next_y = header_height;
    let child_offsets = child_sizes
        .iter()
        .map(|size| {
            let offset = Vec2::new(INDENT, next_y);
            next_y += size.height + GRID_UNIT;
            offset
        })
        .collect();
    let bottom = match child_sizes.is_empty() {
        true => header_height + GRID_UNIT,
        false => next_y,
    };
    Measurement {
        size: Size::new(INDENT + content_width + GRID_UNIT, bottom),
        header_height,
        child_offsets,
    }
}

fn snap_size(size: Size) -> Size {
    Size::new(snap(size.width), snap(size.height))
}

fn snap(value: f64) -> f64 {
    (value / GRID_UNIT - 1e-6).ceil().max(1.0) * GRID_UNIT
}
