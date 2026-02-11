//! Rectangle math for pane regions.

use serde::{Deserialize, Serialize};

/// A rectangle defined by its top-left corner and dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Column of the left edge.
    pub x: u16,
    /// Row of the top edge.
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self { x, y, width, height }
    }

    /// Whether this rectangle contains the given point.
    pub fn contains(&self, col: u16, row: u16) -> bool {
        col >= self.x
            && col < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }

    /// Split horizontally (top/bottom) at `ratio` (0.0–1.0).
    ///
    /// Returns `(top, bottom)`.
    pub fn split_horizontal(&self, ratio: f32) -> (Rect, Rect) {
        let top_h = (self.height as f32 * ratio.clamp(0.0, 1.0)) as u16;
        let top_h = top_h.max(1).min(self.height.saturating_sub(1));
        let bottom_h = self.height.saturating_sub(top_h);

        let top = Rect::new(self.x, self.y, self.width, top_h);
        let bottom = Rect::new(self.x, self.y.saturating_add(top_h), self.width, bottom_h);
        (top, bottom)
    }

    /// Split vertically (left/right) at `ratio` (0.0–1.0).
    ///
    /// Returns `(left, right)`.
    pub fn split_vertical(&self, ratio: f32) -> (Rect, Rect) {
        let left_w = (self.width as f32 * ratio.clamp(0.0, 1.0)) as u16;
        let left_w = left_w.max(1).min(self.width.saturating_sub(1));
        let right_w = self.width.saturating_sub(left_w);

        let left = Rect::new(self.x, self.y, left_w, self.height);
        let right = Rect::new(self.x.saturating_add(left_w), self.y, right_w, self.height);
        (left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_inside() {
        let r = Rect::new(10, 20, 30, 40);
        assert!(r.contains(10, 20));
        assert!(r.contains(25, 35));
        assert!(r.contains(39, 59));
    }

    #[test]
    fn contains_outside() {
        let r = Rect::new(10, 20, 30, 40);
        assert!(!r.contains(9, 20));
        assert!(!r.contains(10, 19));
        assert!(!r.contains(40, 20));
        assert!(!r.contains(10, 60));
    }

    #[test]
    fn split_horizontal_halves() {
        let r = Rect::new(0, 0, 80, 24);
        let (top, bottom) = r.split_horizontal(0.5);
        assert_eq!(top.height + bottom.height, r.height);
        assert_eq!(top.width, r.width);
        assert_eq!(bottom.width, r.width);
        assert_eq!(top.y, 0);
        assert_eq!(bottom.y, top.height);
    }

    #[test]
    fn split_vertical_halves() {
        let r = Rect::new(0, 0, 80, 24);
        let (left, right) = r.split_vertical(0.5);
        assert_eq!(left.width + right.width, r.width);
        assert_eq!(left.height, r.height);
        assert_eq!(right.height, r.height);
        assert_eq!(left.x, 0);
        assert_eq!(right.x, left.width);
    }

    #[test]
    fn split_preserves_total_area() {
        let r = Rect::new(5, 10, 100, 50);
        let (top, bottom) = r.split_horizontal(0.3);
        assert_eq!(top.height + bottom.height, r.height);

        let (left, right) = r.split_vertical(0.7);
        assert_eq!(left.width + right.width, r.width);
    }

    #[test]
    fn split_clamped_ratio() {
        let r = Rect::new(0, 0, 80, 24);
        let (top, bottom) = r.split_horizontal(0.0);
        assert!(top.height >= 1);
        assert!(bottom.height >= 1);

        let (top, bottom) = r.split_horizontal(1.0);
        assert!(top.height >= 1);
        assert!(bottom.height >= 1);
    }
}
