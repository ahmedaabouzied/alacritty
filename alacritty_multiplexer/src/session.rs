//! Session management.

use serde::{Deserialize, Serialize};

use crate::error::{MuxError, MuxResult};
use crate::layout::{Direction, LayoutNode, PaneId};
use crate::window::{MuxWindow, WindowId};

/// Unique identifier for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u32);

/// A multiplexer session owning one or more windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// User-visible session name.
    pub name: String,
    /// All windows in this session.
    pub windows: Vec<MuxWindow>,
    /// Index of the active window.
    pub active_window: usize,
    /// Counter for generating unique window ids.
    next_window_id: u32,
}

impl Session {
    /// Create a new session with one default window.
    pub fn new(id: SessionId, name: impl Into<String>) -> Self {
        let win = MuxWindow::new(WindowId(0), "0");
        Self {
            id,
            name: name.into(),
            windows: vec![win],
            active_window: 0,
            next_window_id: 1,
        }
    }

    /// Add a new window and return its id.
    pub fn add_window(&mut self, name: impl Into<String>) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;
        self.windows.push(MuxWindow::new(id, name));
        self.active_window = self.windows.len() - 1;
        id
    }

    /// Close the window at the given index.
    pub fn close_window(&mut self, idx: usize) -> MuxResult<()> {
        if idx >= self.windows.len() {
            return Err(MuxError::WindowNotFound(idx));
        }
        self.windows.remove(idx);
        if self.windows.is_empty() {
            return Ok(());
        }
        if self.active_window >= self.windows.len() {
            self.active_window = self.windows.len() - 1;
        }
        Ok(())
    }

    /// Switch to the next window (wraps around).
    pub fn next_window(&mut self) {
        if !self.windows.is_empty() {
            self.active_window = (self.active_window + 1) % self.windows.len();
        }
    }

    /// Switch to the previous window (wraps around).
    pub fn prev_window(&mut self) {
        if !self.windows.is_empty() {
            self.active_window = if self.active_window == 0 {
                self.windows.len() - 1
            } else {
                self.active_window - 1
            };
        }
    }

    /// Get a reference to the active window.
    pub fn active_win(&self) -> Option<&MuxWindow> {
        self.windows.get(self.active_window)
    }

    /// Get a mutable reference to the active window.
    pub fn active_win_mut(&mut self) -> Option<&mut MuxWindow> {
        self.windows.get_mut(self.active_window)
    }

    /// Get the active pane id (from the active window).
    pub fn active_pane_id(&self) -> Option<PaneId> {
        self.active_win().map(|w| w.active_pane)
    }

    /// Get the layout of the active window.
    pub fn active_layout(&self) -> Option<&LayoutNode> {
        self.active_win().map(|w| &w.layout)
    }

    /// Whether the session has no windows left.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Split the active pane in the active window.
    pub fn split_active(&mut self, dir: Direction) -> MuxResult<PaneId> {
        let win = self.active_win_mut().ok_or(MuxError::SessionError(
            "no active window".into(),
        ))?;
        let pane_id = win.active_pane;
        win.split(pane_id, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new(SessionId(0), "test")
    }

    #[test]
    fn new_session_has_one_window() {
        let s = session();
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.active_window, 0);
    }

    #[test]
    fn add_window_switches_active() {
        let mut s = session();
        s.add_window("second");
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.active_window, 1);
    }

    #[test]
    fn close_window_adjusts_active() {
        let mut s = session();
        s.add_window("second");
        s.add_window("third");
        s.active_window = 2;
        s.close_window(2).unwrap();
        assert_eq!(s.active_window, 1);
    }

    #[test]
    fn close_invalid_window() {
        let mut s = session();
        assert!(s.close_window(99).is_err());
    }

    #[test]
    fn next_window_wraps() {
        let mut s = session();
        s.add_window("second");
        s.active_window = 1;
        s.next_window();
        assert_eq!(s.active_window, 0);
    }

    #[test]
    fn prev_window_wraps() {
        let mut s = session();
        s.add_window("second");
        s.active_window = 0;
        s.prev_window();
        assert_eq!(s.active_window, 1);
    }

    #[test]
    fn active_pane_id_returns_some() {
        let s = session();
        assert!(s.active_pane_id().is_some());
    }

    #[test]
    fn split_active_works() {
        let mut s = session();
        let new_id = s.split_active(Direction::Vertical).unwrap();
        let win = s.active_win().unwrap();
        assert!(win.layout.find_pane(new_id));
    }
}
