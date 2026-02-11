//! Additional tests to improve line coverage across all modules.

use alacritty_multiplexer::command::{LeaderKeyConfig, MuxCommand};
use alacritty_multiplexer::config::{KeybindingsConfig, MultiplexerConfig, StatusBarConfig};
use alacritty_multiplexer::error::MuxError;
use alacritty_multiplexer::layout::{Direction, LayoutNode, PaneId};
use alacritty_multiplexer::pane::Pane;
use alacritty_multiplexer::protocol::{
    ClientMessage, ServerMessage, decode_message, encode_message,
};
use alacritty_multiplexer::rect::Rect;
use alacritty_multiplexer::resize::resize_pane;
use alacritty_multiplexer::session::{Session, SessionId};
use alacritty_multiplexer::statusbar::{
    StatusBarContent, WindowEntry, build_status, render_status_line,
};
use alacritty_multiplexer::window::{MuxWindow, WindowId};

// ---- error.rs coverage ----

#[test]
fn error_display_messages() {
    let e1 = MuxError::LayoutError("too small".into());
    assert!(e1.to_string().contains("too small"));

    let e2 = MuxError::PaneNotFound(42);
    assert!(e2.to_string().contains("42"));

    let e3 = MuxError::WindowNotFound(5);
    assert!(e3.to_string().contains("5"));

    let e4 = MuxError::SessionError("gone".into());
    assert!(e4.to_string().contains("gone"));

    let e5 = MuxError::PersistenceError("corrupt".into());
    assert!(e5.to_string().contains("corrupt"));

    let e6: MuxError = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
    assert!(e6.to_string().contains("missing"));
}

// ---- command.rs coverage ----

#[test]
fn mux_command_serialization_roundtrip() {
    let commands = vec![
        MuxCommand::SplitHorizontal,
        MuxCommand::SplitVertical,
        MuxCommand::ClosePane,
        MuxCommand::NextPane,
        MuxCommand::PrevPane,
        MuxCommand::NavigatePane(Direction::Horizontal),
        MuxCommand::NavigatePane(Direction::Vertical),
        MuxCommand::NewWindow,
        MuxCommand::CloseWindow,
        MuxCommand::NextWindow,
        MuxCommand::PrevWindow,
        MuxCommand::SwitchToWindow(0),
        MuxCommand::SwitchToWindow(9),
        MuxCommand::RenameWindow("test".into()),
        MuxCommand::DetachSession,
        MuxCommand::ToggleZoom,
        MuxCommand::ResizePane(Direction::Horizontal, 5),
        MuxCommand::ResizePane(Direction::Vertical, -3),
        MuxCommand::ScrollbackMode,
    ];
    for cmd in &commands {
        let json = serde_json::to_string(cmd).unwrap();
        let restored: MuxCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, *cmd);
    }
}

#[test]
fn leader_key_config_default() {
    let cfg = LeaderKeyConfig::default();
    assert_eq!(cfg.keys.len(), 2);
    assert!(cfg.keys.contains(&"Control-Space".to_string()));
    assert!(cfg.keys.contains(&"Control-b".to_string()));
    assert_eq!(cfg.timeout_ms, 1000);
}

// ---- pane.rs coverage ----

#[test]
fn pane_serialization_roundtrip() {
    let pane = Pane::new(PaneId(7));
    let json = serde_json::to_string(&pane).unwrap();
    let restored: Pane = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, PaneId(7));
    assert!(restored.title.is_empty());
}

// ---- session.rs empty session edge cases ----

#[test]
fn empty_session_returns_none() {
    let mut session = Session::new(SessionId(0), "s");
    session.close_window(0).unwrap();
    assert!(session.is_empty());
    assert!(session.active_win().is_none());
    assert!(session.active_win_mut().is_none());
    assert!(session.active_pane_id().is_none());
    assert!(session.active_layout().is_none());
}

#[test]
fn empty_session_navigate_noop() {
    let mut session = Session::new(SessionId(0), "s");
    session.close_window(0).unwrap();
    // Should not panic on empty session.
    session.next_window();
    session.prev_window();
    assert!(session.is_empty());
}

#[test]
fn split_active_on_empty_session_errors() {
    let mut session = Session::new(SessionId(0), "s");
    session.close_window(0).unwrap();
    assert!(session.split_active(Direction::Vertical).is_err());
}

// ---- protocol.rs missing variants ----

#[test]
fn protocol_state_sync_roundtrip() {
    let session = Session::new(SessionId(0), "sync_test");
    let msg = ServerMessage::StateSync(session);
    let encoded = encode_message(&msg).unwrap();
    let (decoded, consumed): (ServerMessage, _) = decode_message(&encoded).unwrap();
    assert_eq!(consumed, encoded.len());
    match decoded {
        ServerMessage::StateSync(s) => assert_eq!(s.name, "sync_test"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn protocol_output_roundtrip() {
    let msg = ServerMessage::Output { pane_id: PaneId(3), data: vec![65, 66, 67] };
    let encoded = encode_message(&msg).unwrap();
    let (decoded, _): (ServerMessage, _) = decode_message(&encoded).unwrap();
    match decoded {
        ServerMessage::Output { pane_id, data } => {
            assert_eq!(pane_id, PaneId(3));
            assert_eq!(data, vec![65, 66, 67]);
        },
        _ => panic!("wrong variant"),
    }
}

#[test]
fn protocol_server_shutdown_roundtrip() {
    let msg = ServerMessage::ServerShutdown;
    let encoded = encode_message(&msg).unwrap();
    let (decoded, _): (ServerMessage, _) = decode_message(&encoded).unwrap();
    assert!(matches!(decoded, ServerMessage::ServerShutdown));
}

#[test]
fn protocol_detach_roundtrip() {
    let msg = ClientMessage::Detach;
    let encoded = encode_message(&msg).unwrap();
    let (decoded, _): (ClientMessage, _) = decode_message(&encoded).unwrap();
    assert!(matches!(decoded, ClientMessage::Detach));
}

#[test]
fn protocol_invalid_json_returns_none() {
    // Valid length header but garbage JSON.
    let mut buf = vec![0, 0, 0, 5]; // length = 5
    buf.extend_from_slice(b"xxxxx");
    assert!(decode_message::<ClientMessage>(&buf).is_none());
}

// ---- config.rs edge cases ----

#[test]
fn keybindings_map_window_numbers() {
    let cfg = KeybindingsConfig::default();
    let map = cfg.to_bindings_map();
    for i in 0..=9u8 {
        let key = i.to_string();
        assert!(map.contains_key(&key));
        assert_eq!(map[&key], MuxCommand::SwitchToWindow(i));
    }
}

#[test]
fn config_partial_deserialization() {
    let json = r#"{"enabled": false}"#;
    let cfg: MultiplexerConfig = serde_json::from_str(json).unwrap();
    assert!(!cfg.enabled);
    // All other fields should have defaults.
    assert!(cfg.status_bar);
    assert_eq!(cfg.leader_timeout_ms, 1000);
}

#[test]
fn status_bar_config_roundtrip() {
    let cfg = StatusBarConfig::default();
    let json = serde_json::to_string(&cfg).unwrap();
    let restored: StatusBarConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.fg, cfg.fg);
    assert_eq!(restored.bg, cfg.bg);
}

// ---- statusbar.rs edge cases ----

#[test]
fn status_bar_zero_width() {
    let content = StatusBarContent {
        session_name: "s".into(),
        windows: vec![WindowEntry { index: 0, name: "w".into(), is_active: true }],
        pane_info: "p".into(),
    };
    let line = render_status_line(&content, 0);
    // Should still render without panicking.
    assert!(line.contains("[s]"));
}

#[test]
fn status_bar_narrow_width() {
    let content = StatusBarContent {
        session_name: "session".into(),
        windows: vec![
            WindowEntry { index: 0, name: "editor".into(), is_active: true },
            WindowEntry { index: 1, name: "shell".into(), is_active: false },
        ],
        pane_info: "pane 1/2".into(),
    };
    let line = render_status_line(&content, 5);
    // Width smaller than content — should not panic.
    assert!(!line.is_empty());
}

#[test]
fn build_status_empty_session() {
    let mut session = Session::new(SessionId(0), "empty");
    session.close_window(0).unwrap();
    let status = build_status(&session);
    assert!(status.windows.is_empty());
    assert!(status.pane_info.is_empty());
}

// ---- rect.rs boundary edge cases ----

#[test]
fn rect_contains_exact_boundary() {
    let r = Rect::new(10, 20, 30, 40);
    // Last valid pixel.
    assert!(r.contains(39, 59));
    // Just outside.
    assert!(!r.contains(40, 59));
    assert!(!r.contains(39, 60));
}

#[test]
fn rect_split_tiny() {
    let r = Rect::new(0, 0, 2, 2);
    let (top, bottom) = r.split_horizontal(0.5);
    assert!(top.height >= 1);
    assert!(bottom.height >= 1);
    assert_eq!(top.height + bottom.height, 2);

    let (left, right) = r.split_vertical(0.5);
    assert!(left.width >= 1);
    assert!(right.width >= 1);
    assert_eq!(left.width + right.width, 2);
}

#[test]
fn rect_split_extreme_ratios_tiny() {
    let r = Rect::new(0, 0, 3, 3);
    let (top, bottom) = r.split_horizontal(0.01);
    assert!(top.height >= 1);
    assert!(bottom.height >= 1);

    let (left, right) = r.split_vertical(0.99);
    assert!(left.width >= 1);
    assert!(right.width >= 1);
}

// ---- resize.rs deep nesting ----

#[test]
fn resize_deeply_nested() {
    use alacritty_multiplexer::split::split_pane;

    let mut tree = LayoutNode::Leaf { pane_id: PaneId(0) };
    let (t, _) = split_pane(tree, PaneId(0), Direction::Vertical, PaneId(1)).unwrap();
    tree = t;
    let (t, _) = split_pane(tree, PaneId(0), Direction::Horizontal, PaneId(2)).unwrap();
    tree = t;
    let (t, _) = split_pane(tree, PaneId(2), Direction::Vertical, PaneId(3)).unwrap();
    tree = t;

    // Resize a deeply nested pane.
    resize_pane(&mut tree, PaneId(3), 0.15).unwrap();

    // All ratios should still be in bounds.
    check_all_ratios(&tree);
}

// ---- layout.rs edge cases ----

#[test]
fn calculate_rects_width_1() {
    let tree = LayoutNode::Leaf { pane_id: PaneId(0) };
    let area = Rect::new(0, 0, 1, 1);
    let rects = tree.calculate_rects(area);
    assert_eq!(rects[&PaneId(0)], area);
}

#[test]
fn layout_rects_varied_ratio() {
    let tree = LayoutNode::Split {
        direction: Direction::Vertical,
        ratio: 0.3,
        first: Box::new(LayoutNode::Leaf { pane_id: PaneId(0) }),
        second: Box::new(LayoutNode::Leaf { pane_id: PaneId(1) }),
    };
    let area = Rect::new(0, 0, 100, 50);
    let rects = tree.calculate_rects(area);
    let total: u32 = rects.values().map(|r| r.width as u32 * r.height as u32).sum();
    assert_eq!(total, 100 * 50);
}

// ---- window.rs edge cases ----

#[test]
fn window_pane_rects_zoomed() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;
    win.split(p0, Direction::Vertical).unwrap();
    win.zoomed = true;

    let area = Rect::new(0, 0, 80, 24);
    let rects = win.pane_rects(area);
    // Zoomed mode is not handled by MuxWindow.pane_rects — it returns all panes.
    // Zoom behavior is handled at the rendering layer (mux_render.rs).
    assert_eq!(rects.len(), 2);
}

#[test]
fn split_multiple_and_verify_order() {
    let mut win = MuxWindow::new(WindowId(0), "test");
    let p0 = win.active_pane;
    let p1 = win.split(p0, Direction::Vertical).unwrap();
    let _p2 = win.split(p1, Direction::Horizontal).unwrap();

    let order = win.pane_order();
    assert_eq!(order.len(), 3);
    // Depth-first: p0 should come first.
    assert_eq!(order[0], p0);
}

// ---- persistence.rs edge cases ----

#[test]
fn delete_nonexistent_session_is_ok() {
    use alacritty_multiplexer::persistence;
    // Deleting a session that doesn't exist should not error.
    let result = persistence::delete_session("surely_does_not_exist_123456");
    assert!(result.is_ok());
}

#[test]
fn deserialize_invalid_json_errors() {
    use alacritty_multiplexer::persistence;
    let result = persistence::deserialize_session("not valid json");
    assert!(result.is_err());
}

/// Helper to check all ratios are in [0.1, 0.9].
fn check_all_ratios(node: &LayoutNode) {
    match node {
        LayoutNode::Leaf { .. } => {},
        LayoutNode::Split { ratio, first, second, .. } => {
            assert!(*ratio >= 0.1 && *ratio <= 0.9, "ratio out of bounds: {ratio}");
            check_all_ratios(first);
            check_all_ratios(second);
        },
    }
}
