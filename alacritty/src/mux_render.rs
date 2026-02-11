//! Multi-pane rendering support for the multiplexer.
//!
//! This module provides functions to render multiple terminal panes within a
//! single Alacritty window, drawing borders between them and a status bar.

use std::collections::HashMap;

use alacritty_multiplexer::layout::PaneId;
use alacritty_multiplexer::rect::Rect as MuxRect;
use alacritty_multiplexer::session::Session;
use alacritty_multiplexer::statusbar;

use crate::display::SizeInfo;

/// Information about a pane's screen region in pixels.
#[derive(Debug, Clone, Copy)]
pub struct PaneRegion {
    /// X offset in pixels from the left of the window.
    pub x: f32,
    /// Y offset in pixels from the top of the window.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Number of columns that fit.
    pub cols: usize,
    /// Number of rows that fit.
    pub rows: usize,
}

/// Compute pixel regions for each pane, reserving one row for the status bar.
pub fn compute_pane_regions(
    session: &Session,
    size_info: &SizeInfo,
) -> HashMap<PaneId, PaneRegion> {
    let cell_width = size_info.cell_width();
    let cell_height = size_info.cell_height();

    // Reserve one row at the bottom for the status bar.
    let usable_cols = (size_info.width() / cell_width) as u16;
    let total_rows = (size_info.height() / cell_height) as u16;
    let usable_rows = total_rows.saturating_sub(1);

    let total_area = MuxRect::new(0, 0, usable_cols, usable_rows);

    let win = match session.active_win() {
        Some(w) => w,
        None => return HashMap::new(),
    };

    // If zoomed, the active pane fills the entire usable area.
    if win.zoomed {
        let mut result = HashMap::new();
        result.insert(win.active_pane, PaneRegion {
            x: 0.0,
            y: 0.0,
            width: usable_cols as f32 * cell_width,
            height: usable_rows as f32 * cell_height,
            cols: usable_cols as usize,
            rows: usable_rows as usize,
        });
        return result;
    }

    let mux_rects = win.pane_rects(total_area);

    mux_rects
        .into_iter()
        .map(|(id, rect)| {
            let region = PaneRegion {
                x: rect.x as f32 * cell_width,
                y: rect.y as f32 * cell_height,
                width: rect.width as f32 * cell_width,
                height: rect.height as f32 * cell_height,
                cols: rect.width as usize,
                rows: rect.height as usize,
            };
            (id, region)
        })
        .collect()
}

/// Border line between two panes.
#[derive(Debug, Clone, Copy)]
pub struct PaneBorder {
    /// X start in pixels.
    pub x: f32,
    /// Y start in pixels.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
    /// Whether this border is adjacent to the active pane.
    pub is_active: bool,
}

/// Compute the border rectangles for pane separators.
pub fn compute_borders(
    session: &Session,
    size_info: &SizeInfo,
) -> Vec<PaneBorder> {
    let cell_width = size_info.cell_width();
    let cell_height = size_info.cell_height();

    let win = match session.active_win() {
        Some(w) if !w.zoomed => w,
        _ => return Vec::new(),
    };

    let usable_cols = (size_info.width() / cell_width) as u16;
    let usable_rows = ((size_info.height() / cell_height) as u16).saturating_sub(1);
    let total_area = MuxRect::new(0, 0, usable_cols, usable_rows);
    let rects = win.pane_rects(total_area);

    let active_rect = rects.get(&win.active_pane);
    let mut borders = Vec::new();

    // For each pair of panes, detect shared edges.
    let ids: Vec<PaneId> = rects.keys().copied().collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let r1 = &rects[&ids[i]];
            let r2 = &rects[&ids[j]];

            // Vertical border (r1 left of r2 or vice versa).
            if r1.x + r1.width == r2.x && ranges_overlap(r1.y, r1.height, r2.y, r2.height) {
                let is_active = active_rect.map_or(false, |ar| ar == r1 || ar == r2);
                borders.push(PaneBorder {
                    x: r2.x as f32 * cell_width - 1.0,
                    y: r1.y.max(r2.y) as f32 * cell_height,
                    width: 1.0,
                    height: overlap_len(r1.y, r1.height, r2.y, r2.height) as f32 * cell_height,
                    is_active,
                });
            } else if r2.x + r2.width == r1.x
                && ranges_overlap(r1.y, r1.height, r2.y, r2.height)
            {
                let is_active = active_rect.map_or(false, |ar| ar == r1 || ar == r2);
                borders.push(PaneBorder {
                    x: r1.x as f32 * cell_width - 1.0,
                    y: r1.y.max(r2.y) as f32 * cell_height,
                    width: 1.0,
                    height: overlap_len(r1.y, r1.height, r2.y, r2.height) as f32 * cell_height,
                    is_active,
                });
            }

            // Horizontal border (r1 above r2 or vice versa).
            if r1.y + r1.height == r2.y && ranges_overlap(r1.x, r1.width, r2.x, r2.width) {
                let is_active = active_rect.map_or(false, |ar| ar == r1 || ar == r2);
                borders.push(PaneBorder {
                    x: r1.x.max(r2.x) as f32 * cell_width,
                    y: r2.y as f32 * cell_height - 1.0,
                    width: overlap_len(r1.x, r1.width, r2.x, r2.width) as f32 * cell_width,
                    height: 1.0,
                    is_active,
                });
            } else if r2.y + r2.height == r1.y
                && ranges_overlap(r1.x, r1.width, r2.x, r2.width)
            {
                let is_active = active_rect.map_or(false, |ar| ar == r1 || ar == r2);
                borders.push(PaneBorder {
                    x: r1.x.max(r2.x) as f32 * cell_width,
                    y: r1.y as f32 * cell_height - 1.0,
                    width: overlap_len(r1.x, r1.width, r2.x, r2.width) as f32 * cell_width,
                    height: 1.0,
                    is_active,
                });
            }
        }
    }

    borders
}

fn ranges_overlap(start1: u16, len1: u16, start2: u16, len2: u16) -> bool {
    let end1 = start1 + len1;
    let end2 = start2 + len2;
    start1 < end2 && start2 < end1
}

fn overlap_len(start1: u16, len1: u16, start2: u16, len2: u16) -> u16 {
    let end1 = start1 + len1;
    let end2 = start2 + len2;
    let start = start1.max(start2);
    let end = end1.min(end2);
    end.saturating_sub(start)
}

/// Build the status bar text line for the current session.
pub fn build_status_line(session: &Session, width_cols: usize) -> String {
    let content = statusbar::build_status(session);
    statusbar::render_status_line(&content, width_cols)
}
