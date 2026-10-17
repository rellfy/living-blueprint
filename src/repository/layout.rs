use crate::repository::view::BodyView;
use crate::repository::{Detail, Node, NodeId};
use vello::kurbo::{Point, Rect, Size, Vec2};

pub const CODE_FONT_SIZE: f64 = 14.0;
pub const LINE_HEIGHT: f64 = 20.0;
pub const FILE_PADDING: f64 = 16.0;
pub const FILE_HEADER_HEIGHT: f64 = 44.0;
pub const FILE_HEADER_LABEL_SIZE: f64 = 16.0;
pub const LABEL_CHARACTER_WIDTH_RATIO: f64 = 0.6;
const MIN_FILE_COLUMNS: u32 = 24;
pub const MAX_FILE_COLUMNS: u32 = 100;
const BINARY_FILE_LINES: f64 = 2.0;
pub const MIN_DIRECTORY_HEADER_HEIGHT: f64 = 56.0;
const MIN_COLLAPSED_COLUMNS: f64 = 12.0;
const DIRECTORY_HEADER_RATIO: f64 = 0.03;
const DIRECTORY_PADDING_RATIO: f64 = 0.45;
const MAX_CHILD_GAP: f64 = 40.0;
const MIN_CHILD_GAP: f64 = 12.0;
const CHILD_GAP_RATIO: f64 = 0.4;
const ROW_WIDTH_RATIO: f64 = 1.6;

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
    let size = measurement.size;
    measurements[id.0] = Some(measurement);
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
    let body_size = match (node.body(), node.detail) {
        (Some(body), _) => measure_body(body, char_advance),
        (None, Detail::Collapsed) => Size::ZERO,
        (None, _) => Size::new(
            MIN_FILE_COLUMNS as f64 * char_advance,
            BINARY_FILE_LINES * LINE_HEIGHT + FILE_PADDING,
        ),
    };
    let label_width =
        node.name.chars().count() as f64 * FILE_HEADER_LABEL_SIZE * LABEL_CHARACTER_WIDTH_RATIO;
    let min_columns = match node.detail {
        Detail::Expanded => MIN_FILE_COLUMNS as f64,
        _ => MIN_COLLAPSED_COLUMNS,
    };
    let width = body_size
        .width
        .max(label_width)
        .max(min_columns * char_advance);
    Measurement {
        size: Size::new(
            width + FILE_PADDING * 2.0,
            FILE_HEADER_HEIGHT + body_size.height,
        ),
        header_height: FILE_HEADER_HEIGHT,
        child_offsets: Vec::new(),
    }
}

fn measure_collapsed_directory(node: &Node, char_advance: f64) -> Measurement {
    let header_height = MIN_DIRECTORY_HEADER_HEIGHT;
    let body_size = node
        .body()
        .map_or(Size::ZERO, |body| measure_body(body, char_advance));
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
    let (content_size, offsets) = pack_rows(child_sizes);
    let header_height =
        (content_size.width * DIRECTORY_HEADER_RATIO).max(MIN_DIRECTORY_HEADER_HEIGHT);
    let padding = header_height * DIRECTORY_PADDING_RATIO;
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

fn pack_rows(sizes: &[Size]) -> (Size, Vec<Vec2>) {
    let row_width = compute_row_width(sizes);
    let gap = compute_gap(sizes);
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut cursor = Vec2::ZERO;
    let mut row_height: f64 = 0.0;
    let mut content_width: f64 = 0.0;
    for size in sizes {
        let is_row_full = cursor.x > 0.0 && cursor.x + size.width > row_width;
        if is_row_full {
            cursor = Vec2::new(0.0, cursor.y + row_height + gap);
            row_height = 0.0;
        }
        offsets.push(cursor);
        cursor.x += size.width + gap;
        row_height = row_height.max(size.height);
        content_width = content_width.max(cursor.x - gap);
    }
    (Size::new(content_width, cursor.y + row_height), offsets)
}

fn compute_row_width(sizes: &[Size]) -> f64 {
    let total_area: f64 = sizes.iter().map(|size| size.area()).sum();
    let widest = sizes.iter().map(|size| size.width).fold(0.0, f64::max);
    (total_area.sqrt() * ROW_WIDTH_RATIO).max(widest)
}

fn compute_gap(sizes: &[Size]) -> f64 {
    let smallest = sizes
        .iter()
        .map(|size| size.height)
        .fold(f64::INFINITY, f64::min);
    (smallest * CHILD_GAP_RATIO).clamp(MIN_CHILD_GAP, MAX_CHILD_GAP)
}
