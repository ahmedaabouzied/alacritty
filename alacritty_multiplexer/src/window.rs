//! Window (tab) management.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::MuxResult;
use crate::layout::{Direction, LayoutNode, PaneId};
use crate::pane::Pane;
use crate::rect::Rect;
use crate::split;

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u32);

/// A multiplexer window (tab) owning a layout and panes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuxWindow {
    /// Unique window identifier.
    pub id: WindowId,
    /// User-visible name.
    pub name: String,
    /// Binary layout tree.
    pub layout: LayoutNode,
    /// Currently focused pane.
    pub active_pane: PaneId,
    /// Pane metadata keyed by id.
    pub panes: HashMap<PaneId, Pane>,
    /// Next pane id counter.
    next_pane_id: u32,
    /// Whether the active pane is zoomed (full-screen).
    pub zoomed: bool,
}

impl MuxWindow {
    /// Create a window with a single initial pane.
    pub fn new(id: WindowId, name: impl Into<String>) -> Self {
        let pane_id = PaneId(0);
        let pane = Pane::new(pane_id);
        let mut panes = HashMap::new();
        panes.insert(pane_id, pane);

        Self {
            id,
            name: name.into(),
            layout: LayoutNode::Leaf { pane_id },
            active_pane: pane_id,
            panes,
            next_pane_id: 1,
            zoomed: false,
        }
    }

    /// Split the given pane, returning the new pane's id.
    pub fn split(&mut self, pane_id: PaneId, dir: Direction) -> MuxResult<PaneId> {
        let new_id = PaneId(self.next_pane_id);
        self.next_pane_id += 1;

        let layout = std::mem::replace(&mut self.layout, LayoutNode::Leaf { pane_id: PaneId(0) });
        let (new_layout, new_pane_id) = split::split_pane(layout, pane_id, dir, new_id)?;
        self.layout = new_layout;
        self.panes.insert(new_pane_id, Pane::new(new_pane_id));
        self.zoomed = false;
        Ok(new_pane_id)
    }

    /// Close the given pane. Returns `true` if the window is now empty.
    pub fn close_pane(&mut self, pane_id: PaneId) -> MuxResult<bool> {
        let layout = std::mem::replace(&mut self.layout, LayoutNode::Leaf { pane_id: PaneId(0) });
        let remaining = split::close_pane(layout, pane_id)?;

        self.panes.remove(&pane_id);
        self.zoomed = false;

        match remaining {
            Some(new_layout) => {
                self.layout = new_layout;
                if self.active_pane == pane_id {
                    self.active_pane = self.pane_order()[0];
                }
                Ok(false)
            },
            None => Ok(true),
        }
    }

    /// Ordered list of pane ids (depth-first).
    pub fn pane_order(&self) -> Vec<PaneId> {
        self.layout.pane_ids()
    }

    /// Focus the next pane in order (wraps around).
    pub fn next_pane(&mut self) {
        let order = self.pane_order();
        self.active_pane = cycle_next(&order, self.active_pane);
    }

    /// Focus the previous pane in order (wraps around).
    pub fn prev_pane(&mut self) {
        let order = self.pane_order();
        self.active_pane = cycle_prev(&order, self.active_pane);
    }

    /// Compute screen rectangles for all panes.
    pub fn pane_rects(&self, total_area: Rect) -> HashMap<PaneId, Rect> {
        self.layout.calculate_rects(total_area)
    }
}

fn cycle_next(order: &[PaneId], current: PaneId) -> PaneId {
    let pos = order.iter().position(|&id| id == current).unwrap_or(0);
    order[(pos + 1) % order.len()]
}

fn cycle_prev(order: &[PaneId], current: PaneId) -> PaneId {
    let pos = order.iter().position(|&id| id == current).unwrap_or(0);
    let prev = if pos == 0 { order.len() - 1 } else { pos - 1 };
    order[prev]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_window_has_one_pane() {
        let w = MuxWindow::new(WindowId(0), "test");
        assert_eq!(w.layout.pane_count(), 1);
        assert_eq!(w.panes.len(), 1);
    }

    #[test]
    fn split_adds_pane() {
        let mut w = MuxWindow::new(WindowId(0), "test");
        let initial = w.active_pane;
        let new_id = w.split(initial, Direction::Vertical).unwrap();
        assert_eq!(w.layout.pane_count(), 2);
        assert_eq!(w.panes.len(), 2);
        assert!(w.panes.contains_key(&new_id));
    }

    #[test]
    fn close_pane_removes() {
        let mut w = MuxWindow::new(WindowId(0), "test");
        let p0 = w.active_pane;
        let p1 = w.split(p0, Direction::Horizontal).unwrap();
        let empty = w.close_pane(p1).unwrap();
        assert!(!empty);
        assert_eq!(w.layout.pane_count(), 1);
    }

    #[test]
    fn close_last_pane_returns_empty() {
        let mut w = MuxWindow::new(WindowId(0), "test");
        let empty = w.close_pane(w.active_pane).unwrap();
        assert!(empty);
    }

    #[test]
    fn next_prev_pane_cycles() {
        let mut w = MuxWindow::new(WindowId(0), "test");
        let p0 = w.active_pane;
        let p1 = w.split(p0, Direction::Vertical).unwrap();
        let _p2 = w.split(p1, Direction::Vertical).unwrap();

        // Start at p0.
        w.active_pane = p0;
        w.next_pane();
        assert_ne!(w.active_pane, p0);

        // Cycle all the way around.
        let start = w.active_pane;
        for _ in 0..w.layout.pane_count() {
            w.next_pane();
        }
        assert_eq!(w.active_pane, start);
    }

    #[test]
    fn prev_pane_wraps() {
        let mut w = MuxWindow::new(WindowId(0), "test");
        let p0 = w.active_pane;
        let _p1 = w.split(p0, Direction::Vertical).unwrap();

        w.active_pane = p0;
        w.prev_pane();
        // Should wrap to last pane.
        assert_ne!(w.active_pane, p0);
    }
}
