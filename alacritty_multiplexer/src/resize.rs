//! Resize operations on the layout tree.

use crate::error::{MuxError, MuxResult};
use crate::layout::{LayoutNode, PaneId};

/// Minimum ratio for the smaller child after a resize.
const MIN_RATIO: f32 = 0.1;
/// Maximum ratio for the larger child after a resize.
const MAX_RATIO: f32 = 0.9;

/// Resize the split that contains `target` by `delta`.
///
/// `delta` is added to the ratio of the split whose first child contains
/// `target`. Positive values grow the first child; negative values shrink it.
/// The ratio is clamped to `[MIN_RATIO, MAX_RATIO]`.
pub fn resize_pane(tree: &mut LayoutNode, target: PaneId, delta: f32) -> MuxResult<()> {
    if resize_inner(tree, target, delta) {
        Ok(())
    } else {
        Err(MuxError::PaneNotFound(target.0))
    }
}

fn resize_inner(node: &mut LayoutNode, target: PaneId, delta: f32) -> bool {
    match node {
        LayoutNode::Leaf { .. } => false,
        LayoutNode::Split { ratio, first, second, .. } => {
            let in_first = first.find_pane(target);
            let in_second = second.find_pane(target);

            if in_first && !in_second {
                *ratio = (*ratio + delta).clamp(MIN_RATIO, MAX_RATIO);
                return true;
            }
            if in_second && !in_first {
                *ratio = (*ratio - delta).clamp(MIN_RATIO, MAX_RATIO);
                return true;
            }

            // Target may be deeper in one subtree.
            resize_inner(first, target, delta) || resize_inner(second, target, delta)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Direction;

    fn leaf(id: u32) -> LayoutNode {
        LayoutNode::Leaf { pane_id: PaneId(id) }
    }

    fn vsplit(a: LayoutNode, b: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: Direction::Vertical,
            ratio: 0.5,
            first: Box::new(a),
            second: Box::new(b),
        }
    }

    fn get_ratio(node: &LayoutNode) -> f32 {
        match node {
            LayoutNode::Split { ratio, .. } => *ratio,
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn resize_grows_first_child() {
        let mut tree = vsplit(leaf(1), leaf(2));
        resize_pane(&mut tree, PaneId(1), 0.1).unwrap();
        assert!((get_ratio(&tree) - 0.6).abs() < 0.001);
    }

    #[test]
    fn resize_shrinks_for_second_child() {
        let mut tree = vsplit(leaf(1), leaf(2));
        resize_pane(&mut tree, PaneId(2), 0.1).unwrap();
        // Growing pane 2 means shrinking ratio (first child gets smaller).
        assert!((get_ratio(&tree) - 0.4).abs() < 0.001);
    }

    #[test]
    fn resize_clamps_min() {
        let mut tree = vsplit(leaf(1), leaf(2));
        resize_pane(&mut tree, PaneId(2), 1.0).unwrap();
        assert!(get_ratio(&tree) >= MIN_RATIO);
    }

    #[test]
    fn resize_clamps_max() {
        let mut tree = vsplit(leaf(1), leaf(2));
        resize_pane(&mut tree, PaneId(1), 1.0).unwrap();
        assert!(get_ratio(&tree) <= MAX_RATIO);
    }

    #[test]
    fn resize_not_found() {
        let mut tree = vsplit(leaf(1), leaf(2));
        let result = resize_pane(&mut tree, PaneId(99), 0.1);
        assert!(result.is_err());
    }
}
