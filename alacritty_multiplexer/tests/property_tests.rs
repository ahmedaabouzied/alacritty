//! Property-based tests for layout invariants.

use proptest::prelude::*;

use alacritty_multiplexer::layout::{Direction, LayoutNode, PaneId};
use alacritty_multiplexer::rect::Rect;
use alacritty_multiplexer::resize::resize_pane;
use alacritty_multiplexer::split::{close_pane, split_pane};

/// Generate a random Direction.
fn arb_direction() -> impl Strategy<Value = Direction> {
    prop_oneof![Just(Direction::Horizontal), Just(Direction::Vertical),]
}

/// Build a random layout tree by performing 1..max_splits sequential splits.
fn arb_layout(max_splits: u32) -> impl Strategy<Value = (LayoutNode, Vec<PaneId>)> {
    (1..=max_splits).prop_flat_map(|n| {
        proptest::collection::vec(arb_direction(), n as usize).prop_map(move |dirs| {
            let mut tree = LayoutNode::Leaf { pane_id: PaneId(0) };
            let mut ids = vec![PaneId(0)];
            let mut next_id = 1u32;
            for dir in &dirs {
                let target = ids[ids.len() / 2]; // Split a middle pane.
                let new_id = PaneId(next_id);
                next_id += 1;
                // Clone so we keep the tree on failure (split takes ownership).
                let (new_tree, pid) = split_pane(tree.clone(), target, *dir, new_id).unwrap();
                tree = new_tree;
                ids.push(pid);
            }
            (tree, ids)
        })
    })
}

proptest! {
    /// All pane rects must tile the total area exactly (no gaps, no overlaps).
    #[test]
    fn rects_tile_area(
        (tree, _ids) in arb_layout(8),
        w in 20u16..200,
        h in 10u16..100,
    ) {
        let area = Rect::new(0, 0, w, h);
        let rects = tree.calculate_rects(area);

        // Number of rects matches pane count.
        prop_assert_eq!(rects.len(), tree.pane_count());

        // Total area of all rects sums to the enclosing area.
        let total: u32 = rects.values().map(|r| r.width as u32 * r.height as u32).sum();
        prop_assert_eq!(total, w as u32 * h as u32);

        // No rect extends outside the enclosing area.
        for r in rects.values() {
            prop_assert!(r.x >= area.x);
            prop_assert!(r.y >= area.y);
            prop_assert!(r.x + r.width <= area.x + area.width);
            prop_assert!(r.y + r.height <= area.y + area.height);
        }
    }

    /// Rects never overlap pairwise.
    #[test]
    fn rects_no_overlap(
        (tree, _ids) in arb_layout(6),
    ) {
        let area = Rect::new(0, 0, 100, 50);
        let rects = tree.calculate_rects(area);
        let ids: Vec<PaneId> = rects.keys().copied().collect();

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let r1 = &rects[&ids[i]];
                let r2 = &rects[&ids[j]];
                let no_h = r1.x + r1.width <= r2.x || r2.x + r2.width <= r1.x;
                let no_v = r1.y + r1.height <= r2.y || r2.y + r2.height <= r1.y;
                prop_assert!(
                    no_h || no_v,
                    "Panes {:?} and {:?} overlap: {:?} vs {:?}",
                    ids[i], ids[j], r1, r2
                );
            }
        }
    }

    /// After resizing, all ratios stay within [0.1, 0.9].
    #[test]
    fn resize_keeps_ratio_bounds(
        (mut tree, ids) in arb_layout(5),
        delta in -1.0f32..1.0,
        idx in 0usize..100,
    ) {
        let target = ids[idx % ids.len()];
        let _ = resize_pane(&mut tree, target, delta);
        check_ratios(&tree);
    }

    /// Navigation cycling always lands on a valid pane.
    #[test]
    fn navigation_cycling_valid(
        (tree, ids) in arb_layout(6),
        steps in 1usize..20,
    ) {
        let order = tree.pane_ids();
        prop_assert!(!order.is_empty());

        let mut current = order[0];
        for _ in 0..steps {
            let pos = order.iter().position(|&id| id == current).unwrap();
            current = order[(pos + 1) % order.len()];
        }
        prop_assert!(ids.contains(&current));
    }

    /// Split then close returns to original pane count.
    #[test]
    fn split_close_roundtrip(
        (tree, ids) in arb_layout(5),
        dir in arb_direction(),
    ) {
        let original_count = tree.pane_count();
        let target = ids[0];
        let new_id = PaneId(1000);

        if let Ok((new_tree, created_id)) = split_pane(tree, target, dir, new_id) {
            prop_assert_eq!(new_tree.pane_count(), original_count + 1);

            if let Ok(Some(restored)) = close_pane(new_tree, created_id) {
                prop_assert_eq!(restored.pane_count(), original_count);
            }
        }
    }

    /// Horizontal split preserves width, vertical split preserves height.
    #[test]
    fn split_rect_dimension_preserved(
        ratio in 0.1f32..0.9,
        w in 10u16..200,
        h in 10u16..100,
    ) {
        let r = Rect::new(0, 0, w, h);

        let (top, bottom) = r.split_horizontal(ratio);
        prop_assert_eq!(top.width, w);
        prop_assert_eq!(bottom.width, w);
        prop_assert_eq!(top.height + bottom.height, h);

        let (left, right) = r.split_vertical(ratio);
        prop_assert_eq!(left.height, h);
        prop_assert_eq!(right.height, h);
        prop_assert_eq!(left.width + right.width, w);
    }
}

/// Recursively check that all split ratios are within bounds.
fn check_ratios(node: &LayoutNode) {
    match node {
        LayoutNode::Leaf { .. } => {},
        LayoutNode::Split { ratio, first, second, .. } => {
            assert!(*ratio >= 0.1 && *ratio <= 0.9, "ratio out of bounds: {ratio}");
            check_ratios(first);
            check_ratios(second);
        },
    }
}
