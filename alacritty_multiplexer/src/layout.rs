//! Binary split tree for pane arrangement.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::rect::Rect;

/// Unique identifier for a pane (monotonic counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u32);

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Top/bottom split.
    Horizontal,
    /// Left/right split.
    Vertical,
}

/// A node in the binary layout tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutNode {
    /// A terminal leaf containing a single pane.
    Leaf {
        /// The pane occupying this leaf.
        pane_id: PaneId,
    },
    /// A split producing two children.
    Split {
        /// Direction of the split.
        direction: Direction,
        /// Ratio allocated to the first child (0.0–1.0).
        ratio: f32,
        /// First child (top or left).
        first: Box<LayoutNode>,
        /// Second child (bottom or right).
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    /// Whether the tree contains a pane with the given id.
    pub fn find_pane(&self, id: PaneId) -> bool {
        match self {
            LayoutNode::Leaf { pane_id } => *pane_id == id,
            LayoutNode::Split { first, second, .. } => first.find_pane(id) || second.find_pane(id),
        }
    }

    /// Collect all pane ids in depth-first order.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        match self {
            LayoutNode::Leaf { pane_id } => vec![*pane_id],
            LayoutNode::Split { first, second, .. } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            },
        }
    }

    /// Number of panes in this tree.
    pub fn pane_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Compute the screen rectangle for every pane.
    pub fn calculate_rects(&self, area: Rect) -> HashMap<PaneId, Rect> {
        let mut result = HashMap::new();
        self.calculate_rects_inner(area, &mut result);
        result
    }

    fn calculate_rects_inner(&self, area: Rect, out: &mut HashMap<PaneId, Rect>) {
        match self {
            LayoutNode::Leaf { pane_id } => {
                out.insert(*pane_id, area);
            },
            LayoutNode::Split { direction, ratio, first, second } => {
                let (a, b) = match direction {
                    Direction::Horizontal => area.split_horizontal(*ratio),
                    Direction::Vertical => area.split_vertical(*ratio),
                };
                first.calculate_rects_inner(a, out);
                second.calculate_rects_inner(b, out);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u32) -> LayoutNode {
        LayoutNode::Leaf { pane_id: PaneId(id) }
    }

    fn split(dir: Direction, a: LayoutNode, b: LayoutNode) -> LayoutNode {
        LayoutNode::Split { direction: dir, ratio: 0.5, first: Box::new(a), second: Box::new(b) }
    }

    #[test]
    fn find_pane_in_leaf() {
        let tree = leaf(1);
        assert!(tree.find_pane(PaneId(1)));
        assert!(!tree.find_pane(PaneId(2)));
    }

    #[test]
    fn find_pane_in_split() {
        let tree = split(Direction::Vertical, leaf(1), leaf(2));
        assert!(tree.find_pane(PaneId(1)));
        assert!(tree.find_pane(PaneId(2)));
        assert!(!tree.find_pane(PaneId(3)));
    }

    #[test]
    fn pane_ids_order() {
        let tree =
            split(Direction::Vertical, leaf(1), split(Direction::Horizontal, leaf(2), leaf(3)));
        assert_eq!(tree.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
    }

    #[test]
    fn pane_count_nested() {
        let tree =
            split(Direction::Horizontal, split(Direction::Vertical, leaf(1), leaf(2)), leaf(3));
        assert_eq!(tree.pane_count(), 3);
    }

    #[test]
    fn calculate_rects_single_pane() {
        let tree = leaf(1);
        let area = Rect::new(0, 0, 80, 24);
        let rects = tree.calculate_rects(area);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[&PaneId(1)], area);
    }

    #[test]
    fn calculate_rects_tiles_area() {
        let tree = split(Direction::Vertical, leaf(1), leaf(2));
        let area = Rect::new(0, 0, 80, 24);
        let rects = tree.calculate_rects(area);

        let r1 = rects[&PaneId(1)];
        let r2 = rects[&PaneId(2)];
        assert_eq!(r1.width + r2.width, area.width);
        assert_eq!(r1.height, area.height);
        assert_eq!(r2.height, area.height);
    }

    #[test]
    fn calculate_rects_deep_nesting() {
        let tree = split(
            Direction::Horizontal,
            split(Direction::Vertical, leaf(1), leaf(2)),
            split(Direction::Vertical, leaf(3), leaf(4)),
        );
        let area = Rect::new(0, 0, 100, 50);
        let rects = tree.calculate_rects(area);
        assert_eq!(rects.len(), 4);

        // Sum of all pane areas should equal total area.
        let total: u32 = rects.values().map(|r| r.width as u32 * r.height as u32).sum();
        assert_eq!(total, area.width as u32 * area.height as u32);
    }
}
