use std::iter::once;
use vello::kurbo::{Point, Rect, Size, Vec2};

const OVERLAP_EPSILON: f64 = 1e-6;

pub struct ArrangedChild {
    pub size: Size,
    pub desired: Option<Vec2>,
    pub is_pinned: bool,
}

pub fn resolve_offsets(children: &[ArrangedChild], gap: f64) -> Vec<Vec2> {
    let mut offsets = vec![Vec2::ZERO; children.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(children.len());
    for index in placement_order(children) {
        let child = &children[index];
        let desired = match (child.desired, child.is_pinned) {
            (Some(desired), true) => float_off_covered_sibling(children, index, desired, gap),
            (Some(desired), false) => desired,
            (None, _) => Vec2::new(0.0, compute_bottom(&placed, gap)),
        };
        let offset = find_free_offset(child.size, snap(desired, gap), &placed, gap);
        placed.push(Rect::from_origin_size(offset.to_point(), child.size));
        offsets[index] = offset;
    }
    offsets
}

fn placement_order(children: &[ArrangedChild]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..children.len()).collect();
    order.sort_by(|first, second| {
        let rank = |index: usize| {
            let child = &children[index];
            let desired = child.desired.unwrap_or(Vec2::new(f64::MAX, f64::MAX));
            (
                !child.is_pinned,
                child.desired.is_none(),
                desired.y,
                desired.x,
            )
        };
        let (first, second) = (rank(*first), rank(*second));
        first
            .0
            .cmp(&second.0)
            .then(first.1.cmp(&second.1))
            .then(first.2.total_cmp(&second.2))
            .then(first.3.total_cmp(&second.3))
    });
    order
}

fn float_off_covered_sibling(
    children: &[ArrangedChild],
    pinned_index: usize,
    desired: Vec2,
    gap: f64,
) -> Vec2 {
    let size = children[pinned_index].size;
    let center = desired.to_point() + size.to_vec2() * 0.5;
    let covered = children
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != pinned_index)
        .filter_map(|(_, child)| {
            Some(Rect::from_origin_size(
                child.desired?.to_point(),
                child.size,
            ))
        })
        .find(|rect| rect.contains(center));
    let Some(covered) = covered else {
        return desired;
    };
    let sides = [
        Vec2::new(covered.x1 + gap, desired.y),
        Vec2::new(covered.x0 - gap - size.width, desired.y),
        Vec2::new(desired.x, covered.y1 + gap),
        Vec2::new(desired.x, covered.y0 - gap - size.height),
    ];
    sides
        .into_iter()
        .filter(|side| side.x >= 0.0 && side.y >= 0.0)
        .min_by(|first, second| {
            (*first - desired)
                .hypot2()
                .total_cmp(&(*second - desired).hypot2())
        })
        .unwrap_or(desired)
}

fn find_free_offset(size: Size, desired: Vec2, placed: &[Rect], gap: f64) -> Vec2 {
    let neighbours = placed.iter().flat_map(|rect| {
        [
            Vec2::new(rect.x1 + gap, desired.y),
            Vec2::new(rect.x0 - gap - size.width, desired.y),
            Vec2::new(desired.x, rect.y1 + gap),
            Vec2::new(desired.x, rect.y0 - gap - size.height),
        ]
    });
    once(desired)
        .chain(neighbours)
        .filter(|candidate| candidate.x >= 0.0 && candidate.y >= 0.0)
        .filter(|candidate| check_is_free(candidate.to_point(), size, placed, gap))
        .min_by(|first, second| {
            (*first - desired)
                .hypot2()
                .total_cmp(&(*second - desired).hypot2())
        })
        .unwrap_or(Vec2::new(desired.x, compute_bottom(placed, gap)))
}

fn check_is_free(origin: Point, size: Size, placed: &[Rect], gap: f64) -> bool {
    let rect = Rect::from_origin_size(origin, size);
    placed.iter().all(|other| {
        let reach = gap - OVERLAP_EPSILON;
        rect.x1 + reach <= other.x0
            || other.x1 + reach <= rect.x0
            || rect.y1 + reach <= other.y0
            || other.y1 + reach <= rect.y0
    })
}

fn compute_bottom(placed: &[Rect], gap: f64) -> f64 {
    placed.iter().map(|rect| rect.y1 + gap).fold(0.0, f64::max)
}

fn snap(offset: Vec2, unit: f64) -> Vec2 {
    Vec2::new(
        ((offset.x / unit).round() * unit).max(0.0),
        ((offset.y / unit).round() * unit).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAP: f64 = 25.0;

    fn child(x: f64, y: f64, is_pinned: bool) -> ArrangedChild {
        ArrangedChild {
            size: Size::new(100.0, 100.0),
            desired: Some(Vec2::new(x, y)),
            is_pinned,
        }
    }

    fn rects(children: &[ArrangedChild], offsets: &[Vec2]) -> Vec<Rect> {
        children
            .iter()
            .zip(offsets)
            .map(|(child, offset)| Rect::from_origin_size(offset.to_point(), child.size))
            .collect()
    }

    fn assert_apart(rects: &[Rect]) {
        for (index, rect) in rects.iter().enumerate() {
            assert!(check_is_free(
                rect.origin(),
                rect.size(),
                &rects[index + 1..],
                GAP
            ));
        }
    }

    #[test]
    fn keeps_free_positions() {
        let children = [child(0.0, 0.0, false), child(200.0, 0.0, false)];
        let offsets = resolve_offsets(&children, GAP);
        assert_eq!(offsets, vec![Vec2::new(0.0, 0.0), Vec2::new(200.0, 0.0)]);
    }

    #[test]
    fn pinned_child_pushes_overlapping_sibling_out() {
        let children = [child(0.0, 0.0, false), child(50.0, 125.0, true)];
        let offsets = resolve_offsets(&children, GAP);
        assert_eq!(offsets[1], Vec2::new(50.0, 125.0));
        assert_apart(&rects(&children, &offsets));
    }

    #[test]
    fn dropping_onto_a_sibling_floats_beside_it() {
        let children = [child(0.0, 0.0, false), child(25.0, 25.0, true)];
        let offsets = resolve_offsets(&children, GAP);
        assert_eq!(offsets[0], Vec2::ZERO);
        assert_ne!(offsets[1], Vec2::new(25.0, 25.0));
        assert_apart(&rects(&children, &offsets));
    }

    #[test]
    fn unplaced_children_go_below() {
        let children = [
            child(0.0, 0.0, false),
            ArrangedChild {
                size: Size::new(100.0, 100.0),
                desired: None,
                is_pinned: false,
            },
        ];
        let offsets = resolve_offsets(&children, GAP);
        assert_eq!(offsets[1], Vec2::new(0.0, 125.0));
    }

    #[test]
    fn offsets_stay_on_grid_and_inside_parent() {
        let children = [child(-60.0, 13.0, true), child(0.0, 0.0, false)];
        for offset in resolve_offsets(&children, GAP) {
            assert!(offset.x >= 0.0 && offset.y >= 0.0);
            assert_eq!(offset.x % GAP, 0.0);
            assert_eq!(offset.y % GAP, 0.0);
        }
    }
}
