//! Split and close operations on the layout tree.

use crate::error::{MuxError, MuxResult};
use crate::layout::{Direction, LayoutNode, PaneId};

/// Split the pane identified by `target` in the given `direction`.
///
/// Returns the updated tree and the new pane's id.
pub fn split_pane(
    tree: LayoutNode,
    target: PaneId,
    direction: Direction,
    new_id: PaneId,
) -> MuxResult<(LayoutNode, PaneId)> {
    let result = split_inner(tree, target, direction, new_id);
    match result {
        SplitResult::Replaced(node) => Ok((node, new_id)),
        SplitResult::NotFound(_) => Err(MuxError::PaneNotFound(target.0)),
    }
}

enum SplitResult {
    Replaced(LayoutNode),
    NotFound(LayoutNode),
}

fn split_inner(
    node: LayoutNode,
    target: PaneId,
    direction: Direction,
    new_id: PaneId,
) -> SplitResult {
    match node {
        LayoutNode::Leaf { pane_id } if pane_id == target => {
            let new_node = LayoutNode::Split {
                direction,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf { pane_id }),
                second: Box::new(LayoutNode::Leaf { pane_id: new_id }),
            };
            SplitResult::Replaced(new_node)
        },
        LayoutNode::Leaf { .. } => SplitResult::NotFound(node),
        LayoutNode::Split { direction: d, ratio, first, second } => {
            match split_inner(*first, target, direction, new_id) {
                SplitResult::Replaced(new_first) => SplitResult::Replaced(LayoutNode::Split {
                    direction: d,
                    ratio,
                    first: Box::new(new_first),
                    second,
                }),
                SplitResult::NotFound(orig_first) => {
                    match split_inner(*second, target, direction, new_id) {
                        SplitResult::Replaced(new_second) => {
                            SplitResult::Replaced(LayoutNode::Split {
                                direction: d,
                                ratio,
                                first: Box::new(orig_first),
                                second: Box::new(new_second),
                            })
                        },
                        SplitResult::NotFound(orig_second) => {
                            SplitResult::NotFound(LayoutNode::Split {
                                direction: d,
                                ratio,
                                first: Box::new(orig_first),
                                second: Box::new(orig_second),
                            })
                        },
                    }
                },
            }
        },
    }
}

/// Close the pane identified by `target`.
///
/// Returns `None` if the last pane was closed (tree is now empty).
pub fn close_pane(tree: LayoutNode, target: PaneId) -> MuxResult<Option<LayoutNode>> {
    match close_inner(tree, target) {
        CloseResult::Removed(remaining) => Ok(remaining),
        CloseResult::NotFound(_) => Err(MuxError::PaneNotFound(target.0)),
    }
}

enum CloseResult {
    Removed(Option<LayoutNode>),
    NotFound(LayoutNode),
}

fn close_inner(node: LayoutNode, target: PaneId) -> CloseResult {
    match node {
        LayoutNode::Leaf { pane_id } if pane_id == target => CloseResult::Removed(None),
        LayoutNode::Leaf { .. } => CloseResult::NotFound(node),
        LayoutNode::Split { direction, ratio, first, second } => {
            match close_inner(*first, target) {
                CloseResult::Removed(None) => CloseResult::Removed(Some(*second)),
                CloseResult::Removed(Some(new_first)) => {
                    CloseResult::Removed(Some(LayoutNode::Split {
                        direction,
                        ratio,
                        first: Box::new(new_first),
                        second,
                    }))
                },
                CloseResult::NotFound(orig_first) => match close_inner(*second, target) {
                    CloseResult::Removed(None) => CloseResult::Removed(Some(orig_first)),
                    CloseResult::Removed(Some(new_second)) => {
                        CloseResult::Removed(Some(LayoutNode::Split {
                            direction,
                            ratio,
                            first: Box::new(orig_first),
                            second: Box::new(new_second),
                        }))
                    },
                    CloseResult::NotFound(orig_second) => {
                        CloseResult::NotFound(LayoutNode::Split {
                            direction,
                            ratio,
                            first: Box::new(orig_first),
                            second: Box::new(orig_second),
                        })
                    },
                },
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u32) -> LayoutNode {
        LayoutNode::Leaf { pane_id: PaneId(id) }
    }

    #[test]
    fn split_increases_count() {
        let tree = leaf(1);
        let (tree, new_id) = split_pane(tree, PaneId(1), Direction::Vertical, PaneId(2)).unwrap();
        assert_eq!(tree.pane_count(), 2);
        assert_eq!(new_id, PaneId(2));
    }

    #[test]
    fn split_not_found() {
        let tree = leaf(1);
        let result = split_pane(tree, PaneId(99), Direction::Vertical, PaneId(2));
        assert!(result.is_err());
    }

    #[test]
    fn close_removes_pane() {
        let tree = LayoutNode::Split {
            direction: Direction::Vertical,
            ratio: 0.5,
            first: Box::new(leaf(1)),
            second: Box::new(leaf(2)),
        };
        let remaining = close_pane(tree, PaneId(1)).unwrap().unwrap();
        assert_eq!(remaining.pane_count(), 1);
        assert!(remaining.find_pane(PaneId(2)));
    }

    #[test]
    fn close_last_pane() {
        let tree = leaf(1);
        let remaining = close_pane(tree, PaneId(1)).unwrap();
        assert!(remaining.is_none());
    }

    #[test]
    fn close_not_found() {
        let tree = leaf(1);
        let result = close_pane(tree, PaneId(99));
        assert!(result.is_err());
    }

    #[test]
    fn split_then_close_roundtrip() {
        let tree = leaf(1);
        let (tree, _) = split_pane(tree, PaneId(1), Direction::Horizontal, PaneId(2)).unwrap();
        assert_eq!(tree.pane_count(), 2);

        let tree = close_pane(tree, PaneId(2)).unwrap().unwrap();
        assert_eq!(tree.pane_count(), 1);
        assert!(tree.find_pane(PaneId(1)));
    }
}
