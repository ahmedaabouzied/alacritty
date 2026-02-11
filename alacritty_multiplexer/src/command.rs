//! Multiplexer command definitions.

use serde::{Deserialize, Serialize};

use crate::layout::Direction;

/// A command dispatched by the multiplexer input layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MuxCommand {
    /// Split the active pane horizontally (top/bottom).
    SplitHorizontal,
    /// Split the active pane vertically (left/right).
    SplitVertical,
    /// Close the active pane.
    ClosePane,
    /// Focus the next pane.
    NextPane,
    /// Focus the previous pane.
    PrevPane,
    /// Navigate to an adjacent pane in the given direction.
    NavigatePane(Direction),
    /// Create a new window (tab).
    NewWindow,
    /// Close the active window.
    CloseWindow,
    /// Switch to the next window.
    NextWindow,
    /// Switch to the previous window.
    PrevWindow,
    /// Switch to window by number (0–9).
    SwitchToWindow(u8),
    /// Rename the active window.
    RenameWindow(String),
    /// Detach from the current session.
    DetachSession,
    /// Toggle pane zoom (full-screen).
    ToggleZoom,
    /// Resize the active pane in a direction.
    ResizePane(Direction, i16),
    /// Enter scrollback / vi mode.
    ScrollbackMode,
}

/// Configuration for the leader (prefix) key(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderKeyConfig {
    /// Key combinations that activate command mode (e.g. "Control-Space").
    pub keys: Vec<String>,
    /// Timeout in milliseconds before leader mode expires.
    pub timeout_ms: u64,
}

impl Default for LeaderKeyConfig {
    fn default() -> Self {
        Self {
            keys: vec!["Control-Space".into(), "Control-b".into()],
            timeout_ms: 1000,
        }
    }
}
