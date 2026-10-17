use crate::analysis::SymbolKind;
use crate::repository::layout::{
    FILE_HEADER_HEIGHT, FILE_HEADER_LABEL_SIZE, FILE_PADDING, LABEL_CHARACTER_WIDTH_RATIO,
    LINE_HEIGHT, text_scale,
};
use crate::repository::{Detail, Node, NodeId, Repository};
use crate::resources::text::{FontRole, TextEngine};
use crate::systems::render::cache::{LABEL_FONT_SIZE, RenderCache};
use crate::systems::render::palette::{ACCENT, INK, INK_DIM};
use crate::systems::render::text::{draw_layout, draw_layout_lines};
use crate::systems::render::{FrameContext, check_overlaps, smoothstep};
use std::collections::HashSet;
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Line, Point, Rect, RoundedRect, Shape, Stroke, Vec2};
use vello::peniko::{Color, Fill};

const MIN_NODE_SCREEN_SIZE: f64 = 2.0;
const NODE_STROKE_PX: f64 = 1.25;
const CORNER_RADIUS_PX: f64 = 6.0;
const DIRECTORY_FILL: Color = Color::from_rgba8(255, 255, 255, 7);
const FILE_FILL: Color = Color::from_rgba8(21, 36, 56, 240);
const FILE_FILL_HOVERED: Color = Color::from_rgba8(27, 45, 71, 245);
const COLLAPSED_DIRECTORY_FILL: Color = Color::from_rgba8(30, 74, 132, 235);
const COLLAPSED_DIRECTORY_FILL_HOVERED: Color = Color::from_rgba8(40, 90, 152, 240);
const CODE_FADE_START_PX: f64 = 5.0;
const CODE_FADE_END_PX: f64 = 9.0;
const BARS_FADE_START_PX: f64 = 0.3;
const BARS_FADE_END_PX: f64 = 0.9;
const DIRECTORY_LABEL_RATIO: f64 = 0.5;
const DIRECTORY_LABEL_HEADER_FIT: f64 = 0.7;
const MIN_LABEL_PX: f64 = 4.5;
const MAX_DIRECTORY_LABEL_PX: f64 = 30.0;
const MIN_DIRECTORY_LABEL_PX: f64 = 12.0;
const MAX_TILE_LABEL_PX: f64 = 24.0;
const DEFINITION_LABEL_PX: f64 = 11.0;
const MIN_DEFINITION_SPACING_PX: f64 = 14.0;
const MIN_DEFINITION_FILE_WIDTH_PX: f64 = 80.0;
const LINK_GLOW_PX: f64 = 10.0;
const LINK_GLOW_ALPHA: f32 = 0.55;

pub struct MapPainter<'a> {
    pub scene: &'a mut Scene,
    pub cache: &'a mut RenderCache,
    pub text_engine: &'a mut TextEngine,
    pub repository: &'a Repository,
    pub frame: &'a FrameContext,
    pub hovered: Option<NodeId>,
    pub linked: &'a HashSet<NodeId>,
}

struct LabelPlacement {
    origin: Point,
    world_size: f64,
    alpha: f32,
    color: Color,
    weight: f32,
}

impl MapPainter<'_> {
    pub fn draw(&mut self) {
        self.draw_node(self.repository.root_id());
    }

    fn draw_node(&mut self, id: NodeId) {
        let node = self.repository.node(id);
        if !check_overlaps(node.rect, self.frame.visible) {
            return;
        }
        if node.rect.width().min(node.rect.height()) * self.frame.zoom < MIN_NODE_SCREEN_SIZE {
            return;
        }
        if self.linked.contains(&id) {
            self.draw_link_glow(node.rect);
        }
        match (node.get_is_directory(), node.detail) {
            (true, Detail::Expanded) => self.draw_directory(id, node),
            (true, _) => self.draw_collapsed_directory(id, node),
            (false, _) => self.draw_file(id, node),
        }
    }

    fn draw_directory(&mut self, id: NodeId, node: &Node) {
        let stroke_color = self.pick_stroke(id, INK.multiply_alpha(0.55));
        self.draw_box(node.rect, DIRECTORY_FILL, stroke_color);
        self.draw_separator(node.rect, node.header_height, 0.25);
        for child in &node.children {
            self.draw_node(*child);
        }
        self.draw_directory_label(node);
    }

    fn draw_collapsed_directory(&mut self, id: NodeId, node: &Node) {
        let (code_alpha, bars_alpha) = self.compute_body_alphas(node);
        let fill = match self.hovered == Some(id) {
            true => COLLAPSED_DIRECTORY_FILL_HOVERED,
            false => COLLAPSED_DIRECTORY_FILL,
        };
        let stroke = self.pick_stroke(id, INK.multiply_alpha(0.75));
        self.draw_box(node.rect, fill, stroke);
        self.draw_separator(node.rect, node.header_height, 0.3 * code_alpha as f32);
        self.draw_line_bars(id, node, bars_alpha);
        self.draw_code(id, node, code_alpha);
        self.draw_directory_label(node);
    }

    fn draw_file(&mut self, id: NodeId, node: &Node) {
        let (code_alpha, bars_alpha) = self.compute_body_alphas(node);
        let fill = match self.hovered == Some(id) {
            true => FILE_FILL_HOVERED,
            false => FILE_FILL,
        };
        let stroke = self.pick_stroke(id, INK.multiply_alpha(0.75));
        self.draw_box(node.rect, fill, stroke);
        let has_body = node.body().is_some();
        let separator_alpha = match has_body {
            true => 0.3 * code_alpha as f32,
            false => 0.0,
        };
        self.draw_separator(node.rect, node.header_height, separator_alpha);
        self.draw_line_bars(id, node, bars_alpha);
        self.draw_code(id, node, code_alpha);
        self.draw_header_label(node, code_alpha);
        let tile_label_bottom = self.draw_tile_label(node, 1.0 - code_alpha);
        if node.detail == Detail::Expanded {
            self.draw_definitions(node, bars_alpha, tile_label_bottom);
        }
    }

    fn pick_stroke(&self, id: NodeId, idle: Color) -> Color {
        let is_highlighted = self.hovered == Some(id) || self.linked.contains(&id);
        match is_highlighted {
            true => ACCENT,
            false => idle,
        }
    }

    fn draw_link_glow(&mut self, rect: Rect) {
        let std_dev = LINK_GLOW_PX * self.frame.scale_factor / self.frame.zoom;
        let radius = self.compute_corner_radius(rect);
        let mut outside = BezPath::new();
        outside.extend(
            rect.inflate(std_dev * 2.5, std_dev * 2.5)
                .path_elements(0.1),
        );
        outside.extend(RoundedRect::from_rect(rect, radius).path_elements(0.1));
        self.scene
            .push_clip_layer(Fill::EvenOdd, self.frame.transform, &outside);
        self.scene.draw_blurred_rounded_rect(
            self.frame.transform,
            rect,
            ACCENT.multiply_alpha(LINK_GLOW_ALPHA),
            radius,
            std_dev,
        );
        self.scene.pop_layer();
    }

    fn compute_corner_radius(&self, rect: Rect) -> f64 {
        (CORNER_RADIUS_PX / self.frame.zoom).min(rect.width().min(rect.height()) * 0.08)
    }

    fn compute_body_alphas(&self, node: &Node) -> (f64, f64) {
        let line_px = LINE_HEIGHT * text_scale(node) * self.frame.zoom;
        let code_alpha = smoothstep(CODE_FADE_START_PX, CODE_FADE_END_PX, line_px);
        let bars_alpha =
            smoothstep(BARS_FADE_START_PX, BARS_FADE_END_PX, line_px) * (1.0 - code_alpha);
        (code_alpha, bars_alpha)
    }

    fn body_rect(&self, node: &Node) -> Rect {
        Rect::new(
            node.rect.x0 + FILE_PADDING * 0.5,
            node.rect.y0 + node.header_height,
            node.rect.x1 - FILE_PADDING * 0.5,
            node.rect.y1,
        )
    }

    fn draw_box(&mut self, rect: Rect, fill: Color, stroke: Color) {
        let zoom = self.frame.zoom;
        let radius = self.compute_corner_radius(rect);
        let shape = RoundedRect::from_rect(rect, radius);
        self.scene
            .fill(Fill::NonZero, self.frame.transform, fill, None, &shape);
        self.scene.stroke(
            &Stroke::new(NODE_STROKE_PX * self.frame.scale_factor / zoom),
            self.frame.transform,
            stroke,
            None,
            &shape,
        );
    }

    fn draw_separator(&mut self, rect: Rect, header_height: f64, alpha: f32) {
        if alpha <= 0.01 {
            return;
        }
        let y = rect.y0 + header_height;
        self.scene.stroke(
            &Stroke::new(self.frame.scale_factor / self.frame.zoom),
            self.frame.transform,
            INK.multiply_alpha(alpha),
            None,
            &Line::new((rect.x0, y), (rect.x1, y)),
        );
    }

    fn draw_line_bars(&mut self, id: NodeId, node: &Node, alpha: f64) {
        if alpha <= 0.01 || node.body().is_none() {
            return;
        }
        let content = self.body_rect(node);
        let scale = text_scale(node);
        let bars_origin = (node.rect.x0 + FILE_PADDING * (1.0 - scale), content.y0);
        let bars_transform =
            self.frame.transform * Affine::translate(bars_origin) * Affine::scale(scale);
        let is_resizing = node.rect != node.target_rect;
        if is_resizing {
            self.scene
                .push_clip_layer(Fill::NonZero, self.frame.transform, &content);
        }
        let bars = self
            .cache
            .line_bars(id, node, self.text_engine.char_advance());
        self.scene.fill(
            Fill::NonZero,
            bars_transform,
            INK_DIM.multiply_alpha(0.5 * alpha as f32),
            None,
            bars,
        );
        if is_resizing {
            self.scene.pop_layer();
        }
    }

    fn draw_code(&mut self, id: NodeId, node: &Node, alpha: f64) {
        if alpha <= 0.01 {
            return;
        }
        let content = self.body_rect(node);
        let visible = content.intersect(self.frame.visible);
        if visible.is_zero_area() {
            return;
        }
        let scale = text_scale(node);
        let line_height = LINE_HEIGHT * scale;
        let first_line = ((visible.y0 - content.y0) / line_height).floor().max(0.0) as usize;
        let last_line = ((visible.y1 - content.y0) / line_height).ceil() as usize + 1;
        let Some(layout) = self.cache.code_layout(id, node, self.text_engine) else {
            return;
        };
        let text_transform = self.frame.transform
            * Affine::translate((node.rect.x0 + FILE_PADDING, content.y0))
            * Affine::scale(scale);
        self.scene
            .push_clip_layer(Fill::NonZero, self.frame.transform, &content);
        draw_layout_lines(
            self.scene,
            layout,
            text_transform,
            alpha as f32,
            first_line..last_line,
        );
        self.scene.pop_layer();
    }

    fn draw_definitions(&mut self, node: &Node, alpha: f64, min_world_y: f64) {
        let zoom = self.frame.zoom;
        if alpha <= 0.2 || node.rect.width() * zoom < MIN_DEFINITION_FILE_WIDTH_PX {
            return;
        }
        let Some(analysis) = node.source_file().and_then(|file| file.analysis.as_ref()) else {
            return;
        };
        let label_px = DEFINITION_LABEL_PX * self.frame.scale_factor;
        let mut last_screen_y = f64::NEG_INFINITY;
        for symbol in analysis
            .definitions
            .iter()
            .filter(|symbol| check_is_outline_symbol(symbol.kind))
        {
            let world_y = node.rect.y0 + FILE_HEADER_HEIGHT + symbol.line as f64 * LINE_HEIGHT;
            let screen_y = world_y * zoom;
            if screen_y - last_screen_y < MIN_DEFINITION_SPACING_PX * self.frame.scale_factor {
                continue;
            }
            if world_y < self.frame.visible.y0.max(min_world_y) || world_y > self.frame.visible.y1 {
                continue;
            }
            last_screen_y = screen_y;
            self.draw_label(
                &symbol.name,
                LabelPlacement {
                    origin: Point::new(
                        node.rect.x0 + FILE_PADDING,
                        world_y - label_px * 0.5 / zoom,
                    ),
                    world_size: label_px / zoom,
                    alpha: alpha as f32,
                    color: ACCENT,
                    weight: 600.0,
                },
            );
        }
    }

    fn draw_directory_label(&mut self, node: &Node) {
        let label = directory_label(node);
        let character_count = label.chars().count().max(1) as f64;
        let min_world_size = MIN_DIRECTORY_LABEL_PX * self.frame.scale_factor / self.frame.zoom;
        let preferred_size = (node.header_height * DIRECTORY_LABEL_RATIO).max(min_world_size);
        let fitting_size =
            node.rect.width() * 0.9 / (character_count * LABEL_CHARACTER_WIDTH_RATIO);
        let world_size = cap_world_size(
            preferred_size.min(fitting_size),
            MAX_DIRECTORY_LABEL_PX,
            self.frame,
        )
        .min(node.header_height * DIRECTORY_LABEL_HEADER_FIT);
        let inset = node.header_height * 0.45;
        let origin = Point::new(
            node.rect.x0 + inset,
            node.rect.y0 + ((node.header_height - world_size * 1.2) * 0.5).max(inset * 0.3),
        );
        self.draw_label(
            &label,
            LabelPlacement {
                origin,
                world_size,
                alpha: 0.95,
                color: INK,
                weight: 650.0,
            },
        );
    }

    fn draw_header_label(&mut self, node: &Node, alpha: f64) {
        let world_size = FILE_HEADER_LABEL_SIZE * text_scale(node);
        self.draw_label(
            &node.name,
            LabelPlacement {
                origin: Point::new(
                    node.rect.x0 + FILE_PADDING,
                    node.rect.y0 + (node.header_height - world_size * 1.2) * 0.5,
                ),
                world_size,
                alpha: alpha as f32,
                color: INK,
                weight: 600.0,
            },
        );
    }

    fn draw_tile_label(&mut self, node: &Node, alpha: f64) -> f64 {
        let rect = node.rect;
        if alpha <= 0.01 {
            return rect.y0;
        }
        let character_count = node.name.chars().count().max(1) as f64;
        let fitted_size = (rect.width() * 0.85 / (character_count * LABEL_CHARACTER_WIDTH_RATIO))
            .min(rect.height() * 0.3);
        let world_size = cap_world_size(fitted_size, MAX_TILE_LABEL_PX, self.frame);
        let layout = self
            .cache
            .label(&node.name, FontRole::Interface, 600.0, self.text_engine);
        let scale = world_size / LABEL_FONT_SIZE as f64;
        let label_size = Vec2::new(layout.width() as f64, layout.height() as f64) * scale;
        let anchor_y = rect.y0 + rect.height().min(rect.width() * 0.6) * 0.5;
        let origin = Point::new(
            rect.center().x - label_size.x * 0.5,
            anchor_y - label_size.y * 0.5,
        );
        self.draw_label(
            &node.name,
            LabelPlacement {
                origin,
                world_size,
                alpha: alpha as f32,
                color: INK,
                weight: 600.0,
            },
        );
        origin.y + label_size.y
    }

    fn draw_label(&mut self, text: &str, placement: LabelPlacement) {
        let screen_size = placement.world_size * self.frame.zoom;
        if placement.alpha <= 0.01 || screen_size < MIN_LABEL_PX * self.frame.scale_factor {
            return;
        }
        let layout = self.cache.label(
            text,
            FontRole::Interface,
            placement.weight,
            self.text_engine,
        );
        let transform = self.frame.transform
            * Affine::translate(placement.origin.to_vec2())
            * Affine::scale(placement.world_size / LABEL_FONT_SIZE as f64);
        draw_layout(
            self.scene,
            layout,
            transform,
            placement.color.multiply_alpha(placement.alpha),
        );
    }
}

fn cap_world_size(world_size: f64, max_screen_px: f64, frame: &FrameContext) -> f64 {
    world_size.min(max_screen_px * frame.scale_factor / frame.zoom)
}

fn directory_label(node: &Node) -> String {
    match node.parent {
        Some(_) => format!("{}/", node.name),
        None => node.name.clone(),
    }
}

fn check_is_outline_symbol(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function
            | SymbolKind::Method
            | SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Module
            | SymbolKind::Macro
            | SymbolKind::Type
    )
}
