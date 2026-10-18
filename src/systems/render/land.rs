use crate::repository::{NodeId, Repository};
use crate::systems::render::background::draw_grid;
use crate::systems::render::palette::{BACKGROUND, GRID_MAJOR, GRID_MINOR};
use crate::systems::render::{FrameContext, check_overlaps};
use vello::Scene;
use vello::peniko::color::{AlphaColor, Oklch, Srgb};
use vello::peniko::{Color, Fill};

const BLUEPRINT_HUE: f64 = 258.0;
const GOLDEN_ANGLE: f64 = 137.508;
const CHROMA: f32 = 0.095;
const BACKGROUND_LIGHTNESS: f32 = 0.37;
const DEPTH_LIGHTNESS_STEP: f32 = 0.035;
const GRID_MINOR_LIGHTNESS: f32 = 0.06;
const GRID_MAJOR_LIGHTNESS: f32 = 0.13;
const HEADER_LIGHTNESS: f32 = 0.04;
const HEADER_HOVERED_LIGHTNESS: f32 = 0.08;
const HEADER_ALPHA: f32 = 0.94;
const MIN_LAND_SCREEN_SIZE: f64 = 2.0;

pub struct Land {
    pub background: Color,
    pub grid_minor: Color,
    pub grid_major: Color,
    pub header: Color,
    pub header_hovered: Color,
}

pub fn draw_lands(scene: &mut Scene, repository: &Repository, frame: &FrameContext) {
    for child in &repository.node(repository.root_id()).children {
        draw_land(scene, repository, *child, frame);
    }
}

impl Land {
    pub fn of(repository: &Repository, id: NodeId) -> Self {
        let depth = compute_depth(repository, id);
        if depth == 0 {
            return Self::blueprint();
        }
        let hue = compute_hue(repository, id);
        let depth_offset = match depth % 2 {
            0 => DEPTH_LIGHTNESS_STEP,
            _ => 0.0,
        };
        let lightness = BACKGROUND_LIGHTNESS + depth_offset;
        Self {
            background: oklch(lightness, hue),
            grid_minor: oklch(lightness + GRID_MINOR_LIGHTNESS, hue),
            grid_major: oklch(lightness + GRID_MAJOR_LIGHTNESS, hue),
            header: oklch(lightness + HEADER_LIGHTNESS, hue).with_alpha(HEADER_ALPHA),
            header_hovered: oklch(lightness + HEADER_HOVERED_LIGHTNESS, hue)
                .with_alpha(HEADER_ALPHA),
        }
    }

    fn blueprint() -> Self {
        Self {
            background: BACKGROUND,
            grid_minor: GRID_MINOR,
            grid_major: GRID_MAJOR,
            header: GRID_MINOR,
            header_hovered: GRID_MAJOR,
        }
    }
}

fn draw_land(scene: &mut Scene, repository: &Repository, id: NodeId, frame: &FrameContext) {
    let node = repository.node(id);
    if !node.get_is_expanded_directory() || !check_overlaps(node.rect, frame.visible) {
        return;
    }
    if node.rect.width().min(node.rect.height()) * frame.zoom < MIN_LAND_SCREEN_SIZE {
        return;
    }
    let land = Land::of(repository, id);
    scene.push_clip_layer(Fill::NonZero, frame.transform, &node.rect);
    scene.fill(
        Fill::NonZero,
        frame.transform,
        land.background,
        None,
        &node.rect,
    );
    draw_grid(scene, frame, node.rect, land.grid_minor, land.grid_major);
    scene.pop_layer();
    for child in &node.children {
        draw_land(scene, repository, *child, frame);
    }
}

fn compute_hue(repository: &Repository, id: NodeId) -> f64 {
    let Some(parent) = repository.node(id).parent else {
        return BLUEPRINT_HUE;
    };
    let sibling_index = repository
        .node(parent)
        .children
        .iter()
        .filter(|child| repository.node(**child).get_is_directory())
        .position(|child| *child == id)
        .unwrap_or_default();
    let step = GOLDEN_ANGLE * (sibling_index + 1) as f64;
    (compute_hue(repository, parent) + step).rem_euclid(360.0)
}

fn compute_depth(repository: &Repository, id: NodeId) -> usize {
    match repository.node(id).parent {
        Some(parent) => 1 + compute_depth(repository, parent),
        None => 0,
    }
}

fn oklch(lightness: f32, hue: f64) -> Color {
    AlphaColor::<Oklch>::new([lightness, CHROMA, hue as f32, 1.0]).convert::<Srgb>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hue_distance(first: f64, second: f64) -> f64 {
        let difference = (first - second).rem_euclid(360.0);
        difference.min(360.0 - difference)
    }

    #[test]
    fn golden_angle_keeps_first_siblings_apart() {
        let hues: Vec<f64> = (1..=4)
            .map(|index| (GOLDEN_ANGLE * index as f64).rem_euclid(360.0))
            .collect();
        for (index, hue) in hues.iter().enumerate() {
            for other in &hues[index + 1..] {
                assert!(hue_distance(*hue, *other) > 40.0);
            }
        }
    }
}
