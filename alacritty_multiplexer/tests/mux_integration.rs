//! Integration tests for the multiplexer lifecycle.

use alacritty_multiplexer::layout::{Direction, PaneId};
use alacritty_multiplexer::persistence;
use alacritty_multiplexer::rect::Rect;
use alacritty_multiplexer::session::{Session, SessionId};

/// Create a session, split panes, verify layout, navigate, verify active pane.
#[test]
fn session_split_navigate_lifecycle() {
    let mut session = Session::new(SessionId(0), "test");

    // Initial: one window, one pane.
    assert_eq!(session.windows.len(), 1);
    assert_eq!(session.active_pane_id(), Some(PaneId(0)));

    // Split vertically.
    let p1 = session.split_active(Direction::Vertical).unwrap();
    let win = session.active_win().unwrap();
    assert_eq!(win.layout.pane_count(), 2);
    assert!(win.layout.find_pane(PaneId(0)));
    assert!(win.layout.find_pane(p1));

    // Split again horizontally.
    let p2 = session.split_active(Direction::Horizontal).unwrap();
    let win = session.active_win().unwrap();
    assert_eq!(win.layout.pane_count(), 3);
    assert!(win.layout.find_pane(p2));

    // Verify pane rects tile the area.
    let area = Rect::new(0, 0, 80, 24);
    let rects = win.pane_rects(area);
    assert_eq!(rects.len(), 3);
    let total_area: u32 = rects.values().map(|r| r.width as u32 * r.height as u32).sum();
    assert_eq!(total_area, 80 * 24);

    // Navigate panes.
    let start = session.active_pane_id().unwrap();
    session.active_win_mut().unwrap().next_pane();
    let after_next = session.active_pane_id().unwrap();
    assert_ne!(start, after_next);

    // Cycle back.
    for _ in 0..2 {
        session.active_win_mut().unwrap().next_pane();
    }
    assert_eq!(session.active_pane_id().unwrap(), start);
}

/// Window creation and switching.
#[test]
fn window_creation_and_switching() {
    let mut session = Session::new(SessionId(0), "multi");

    session.add_window("editor");
    session.add_window("logs");
    assert_eq!(session.windows.len(), 3);
    assert_eq!(session.active_window, 2); // Last added is active.

    // Switch to first window.
    session.active_window = 0;
    assert_eq!(session.active_win().unwrap().name, "0");

    // Next window wraps.
    session.active_window = 2;
    session.next_window();
    assert_eq!(session.active_window, 0);

    // Prev window wraps.
    session.prev_window();
    assert_eq!(session.active_window, 2);

    // Close middle window.
    session.active_window = 1;
    session.close_window(1).unwrap();
    assert_eq!(session.windows.len(), 2);
}

/// Persistence: save session, load, verify layout matches.
#[test]
fn persistence_roundtrip() {
    let mut session = Session::new(SessionId(0), "persist_test");
    session.split_active(Direction::Vertical).unwrap();
    session.add_window("second");

    let json = persistence::serialize_session(&session).unwrap();
    let restored = persistence::deserialize_session(&json).unwrap();

    assert_eq!(restored.name, "persist_test");
    assert_eq!(restored.windows.len(), 2);
    assert_eq!(restored.windows[0].layout.pane_count(), 2);
    assert_eq!(restored.windows[1].layout.pane_count(), 1);
}

/// Multiple splits create correct tree structure.
#[test]
fn complex_split_layout() {
    let mut session = Session::new(SessionId(0), "complex");

    // Split pane 0 vertically → pane 0 | pane 1
    session.split_active(Direction::Vertical).unwrap();

    // Focus pane 0, split horizontally → pane 0 / pane 2
    session.active_win_mut().unwrap().active_pane = PaneId(0);
    session.split_active(Direction::Horizontal).unwrap();

    let win = session.active_win().unwrap();
    assert_eq!(win.layout.pane_count(), 3);

    // Verify rects don't overlap.
    let area = Rect::new(0, 0, 100, 50);
    let rects = win.pane_rects(area);
    let ids: Vec<PaneId> = rects.keys().copied().collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let r1 = &rects[&ids[i]];
            let r2 = &rects[&ids[j]];
            // No overlap: one must be entirely left/right/above/below the other.
            let no_h_overlap = r1.x + r1.width <= r2.x || r2.x + r2.width <= r1.x;
            let no_v_overlap = r1.y + r1.height <= r2.y || r2.y + r2.height <= r1.y;
            assert!(
                no_h_overlap || no_v_overlap,
                "Panes {:?} and {:?} overlap: {:?} vs {:?}",
                ids[i],
                ids[j],
                r1,
                r2
            );
        }
    }
}
