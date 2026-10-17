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
const COLUMN_RATIO: f64 = 1.6;
const MIN_COLLAPSED_COLUMNS: f64 = 12.0;
const DIRECTORY_HEADER_RATIO: f64 = 0.03;
const DIRECTORY_PADDING_RATIO: f64 = 0.45;
const MAX_CHILD_GAP: f64 = 40.0;
const MIN_CHILD_GAP: f64 = 12.0;
const CHILD_GAP_RATIO: f64 = 0.4;

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
    let header_height = snap(MIN_DIRECTORY_HEADER_HEIGHT);
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
    let (content_size, offsets) = pack_table(child_sizes);
    let header_height =
        snap(content_size.width * DIRECTORY_HEADER_RATIO).max(snap(MIN_DIRECTORY_HEADER_HEIGHT));
    let padding = snap(header_height * DIRECTORY_PADDING_RATIO);
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

fn pack_table(sizes: &[Size]) -> (Size, Vec<Vec2>) {
    let column_count = compute_column_count(sizes.len());
    let gap = compute_gap(sizes);
    let mut column_widths = vec![0.0_f64; column_count];
    let mut row_heights = vec![0.0_f64; sizes.len().div_ceil(column_count)];
    for (index, size) in sizes.iter().enumerate() {
        let column = index % column_count;
        let row = index / column_count;
        column_widths[column] = column_widths[column].max(size.width);
        row_heights[row] = row_heights[row].max(size.height);
    }
    let column_offsets = accumulate_offsets(&column_widths, gap);
    let row_offsets = accumulate_offsets(&row_heights, gap);
    let offsets = (0..sizes.len())
        .map(|index| {
            Vec2::new(
                column_offsets[index % column_count],
                row_offsets[index / column_count],
            )
        })
        .collect();
    let content_size = Size::new(
        compute_span(&column_widths, gap),
        compute_span(&row_heights, gap),
    );
    (content_size, offsets)
}

pub fn compute_column_count(child_count: usize) -> usize {
    let column_count = (child_count as f64 * COLUMN_RATIO).sqrt().ceil() as usize;
    column_count.clamp(1, child_count.max(1))
}

fn accumulate_offsets(extents: &[f64], gap: f64) -> Vec<f64> {
    extents
        .iter()
        .scan(0.0, |cursor, extent| {
            let offset = *cursor;
            *cursor += extent + gap;
            Some(offset)
        })
        .collect()
}

fn compute_span(extents: &[f64], gap: f64) -> f64 {
    let total: f64 = extents.iter().sum();
    total + gap * extents.len().saturating_sub(1) as f64
}

fn compute_gap(sizes: &[Size]) -> f64 {
    let smallest = sizes
        .iter()
        .map(|size| size.height)
        .fold(f64::INFINITY, f64::min);
    snap((smallest * CHILD_GAP_RATIO).clamp(MIN_CHILD_GAP, MAX_CHILD_GAP))
}

fn snap_size(size: Size) -> Size {
    Size::new(snap(size.width), snap(size.height))
}

fn snap(value: f64) -> f64 {
    (value / GRID_UNIT - 1e-6).ceil().max(1.0) * GRID_UNIT
}
