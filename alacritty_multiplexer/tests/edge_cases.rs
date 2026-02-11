//! Edge case tests for pane and window lifecycle.

use alacritty_multiplexer::layout::{Direction, PaneId};
use alacritty_multiplexer::rect::Rect;
use alacritty_multiplexer::session::{Session, SessionId};
use alacritty_multiplexer::window::{MuxWindow, WindowId};

/// Closing the last pane in a window signals the window is empty.
#[test]
fn close_last_pane_in_window() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let pane = win.active_pane;
    let is_empty = win.close_pane(pane).unwrap();
    assert!(is_empty);
}

/// Closing the last window in a session leaves it empty.
#[test]
fn close_last_window_in_session() {
    let mut session = Session::new(SessionId(0), "s");
    session.close_window(0).unwrap();
    assert!(session.is_empty());
}

/// Closing a pane makes active_pane point to a valid remaining pane.
#[test]
fn close_active_pane_resets_focus() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;
    let p1 = win.split(p0, Direction::Vertical).unwrap();
    win.active_pane = p1;

    win.close_pane(p1).unwrap();
    assert_eq!(win.active_pane, p0);
}

/// Closing a non-active pane leaves active_pane unchanged.
#[test]
fn close_non_active_pane_preserves_focus() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;
    let p1 = win.split(p0, Direction::Vertical).unwrap();
    win.active_pane = p0;

    win.close_pane(p1).unwrap();
    assert_eq!(win.active_pane, p0);
}

/// Closing a nonexistent pane returns an error.
#[test]
fn close_nonexistent_pane_errors() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    assert!(win.close_pane(PaneId(999)).is_err());
}

/// Splitting a nonexistent pane returns an error.
#[test]
fn split_nonexistent_pane_errors() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    assert!(win.split(PaneId(999), Direction::Horizontal).is_err());
}

/// Rapid split/close cycles don't corrupt state.
#[test]
fn rapid_split_close_cycles() {
    let mut session = Session::new(SessionId(0), "stress");

    for _ in 0..20 {
        let new = session.split_active(Direction::Vertical).unwrap();
        let win = session.active_win().unwrap();
        assert!(win.layout.find_pane(new));

        let win = session.active_win_mut().unwrap();
        let is_empty = win.close_pane(new).unwrap();
        assert!(!is_empty);
    }

    // Should be back to a single pane.
    let win = session.active_win().unwrap();
    assert_eq!(win.layout.pane_count(), 1);
}

/// Alternating horizontal and vertical splits produce valid layouts.
#[test]
fn alternating_split_directions() {
    let mut session = Session::new(SessionId(0), "alt");
    let area = Rect::new(0, 0, 120, 40);

    for i in 0..6 {
        let dir = if i % 2 == 0 { Direction::Horizontal } else { Direction::Vertical };
        session.split_active(dir).unwrap();
    }

    let win = session.active_win().unwrap();
    assert_eq!(win.layout.pane_count(), 7);

    let rects = win.pane_rects(area);
    assert_eq!(rects.len(), 7);
    verify_no_overlap(&rects);
}

/// Navigating panes with only one pane stays on the same pane.
#[test]
fn navigate_single_pane_noop() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;

    win.next_pane();
    assert_eq!(win.active_pane, p0);

    win.prev_pane();
    assert_eq!(win.active_pane, p0);
}

/// Window navigation with a single window wraps to itself.
#[test]
fn navigate_single_window_noop() {
    let mut session = Session::new(SessionId(0), "s");
    session.next_window();
    assert_eq!(session.active_window, 0);
    session.prev_window();
    assert_eq!(session.active_window, 0);
}

/// Zoomed state is cleared when splitting.
#[test]
fn zoom_cleared_on_split() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    win.zoomed = true;
    let p0 = win.active_pane;
    win.split(p0, Direction::Vertical).unwrap();
    assert!(!win.zoomed);
}

/// Zoomed state is cleared when closing a pane.
#[test]
fn zoom_cleared_on_close() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;
    let p1 = win.split(p0, Direction::Vertical).unwrap();
    win.zoomed = true;
    win.close_pane(p1).unwrap();
    assert!(!win.zoomed);
}

/// Closing the active window when it's the last one adjusts active_window.
#[test]
fn close_active_last_window_adjusts() {
    let mut session = Session::new(SessionId(0), "s");
    session.add_window("w1");
    session.add_window("w2");
    // active_window = 2 (the last added)
    session.close_window(2).unwrap();
    assert!(session.active_window < session.windows.len());
}

/// Closing a window before the active one adjusts the active index.
#[test]
fn close_earlier_window_adjusts_active() {
    let mut session = Session::new(SessionId(0), "s");
    session.add_window("w1");
    session.add_window("w2");
    session.active_window = 2;
    session.close_window(0).unwrap();
    // active_window should now point to the same window (shifted left).
    assert_eq!(session.active_window, session.windows.len() - 1);
}

/// Persistence roundtrip preserves zoomed state.
#[test]
fn persistence_preserves_zoom() {
    use alacritty_multiplexer::persistence;

    let mut session = Session::new(SessionId(0), "zoom_test");
    session.split_active(Direction::Vertical).unwrap();
    session.active_win_mut().unwrap().zoomed = true;

    let json = persistence::serialize_session(&session).unwrap();
    let restored = persistence::deserialize_session(&json).unwrap();
    assert!(restored.active_win().unwrap().zoomed);
}

/// Multiple windows each with splits, all rects valid.
#[test]
fn multiple_windows_all_valid_rects() {
    let mut session = Session::new(SessionId(0), "multi");
    session.split_active(Direction::Horizontal).unwrap();

    session.add_window("second");
    session.split_active(Direction::Vertical).unwrap();
    session.split_active(Direction::Horizontal).unwrap();

    let area = Rect::new(0, 0, 80, 24);
    for win in &session.windows {
        let rects = win.pane_rects(area);
        assert_eq!(rects.len(), win.layout.pane_count());
        verify_no_overlap(&rects);
    }
}

/// Helper: verify no rectangles overlap.
fn verify_no_overlap(rects: &std::collections::HashMap<PaneId, Rect>) {
    let ids: Vec<PaneId> = rects.keys().copied().collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let r1 = &rects[&ids[i]];
            let r2 = &rects[&ids[j]];
            let no_h = r1.x + r1.width <= r2.x || r2.x + r2.width <= r1.x;
            let no_v = r1.y + r1.height <= r2.y || r2.y + r2.height <= r1.y;
            assert!(
                no_h || no_v,
                "Panes {:?} and {:?} overlap: {:?} vs {:?}",
                ids[i], ids[j], r1, r2
            );
        }
    }
}
