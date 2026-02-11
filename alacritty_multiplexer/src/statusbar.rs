//! Status bar content generation.

use crate::session::Session;

/// Describes a window entry for the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowEntry {
    /// Index in the window list.
    pub index: usize,
    /// Window name.
    pub name: String,
    /// Whether this window is currently active.
    pub is_active: bool,
}

/// Content to be rendered in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarContent {
    /// Name of the current session.
    pub session_name: String,
    /// Window list with active indicator.
    pub windows: Vec<WindowEntry>,
    /// Information about the active pane.
    pub pane_info: String,
}

/// Build the status bar content from the current session state.
pub fn build_status(session: &Session) -> StatusBarContent {
    let windows = session
        .windows
        .iter()
        .enumerate()
        .map(|(i, w)| WindowEntry {
            index: i,
            name: w.name.clone(),
            is_active: i == session.active_window,
        })
        .collect();

    let pane_info = session
        .active_win()
        .map(|w| format!("pane {}/{}", pane_position(w), w.layout.pane_count()))
        .unwrap_or_default();

    StatusBarContent { session_name: session.name.clone(), windows, pane_info }
}

fn pane_position(w: &crate::window::MuxWindow) -> usize {
    let order = w.pane_order();
    order.iter().position(|&id| id == w.active_pane).map(|p| p + 1).unwrap_or(1)
}

/// Format a window entry for the status bar.
fn format_window_entry(w: &WindowEntry) -> String {
    let marker = if w.is_active { "*" } else { "" };
    format!(" {}:{}{}", w.index, w.name, marker)
}

/// Render the status bar content as a single line string.
pub fn render_status_line(content: &StatusBarContent, width: usize) -> String {
    let left = format!("[{}]", content.session_name);
    let center: String = content.windows.iter().map(format_window_entry).collect();
    let right = &content.pane_info;

    let used = left.len() + center.len() + right.len();
    let padding = width.saturating_sub(used);

    format!("{left}{center}{:>pad$}{right}", "", pad = padding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, SessionId};

    fn session() -> Session {
        Session::new(SessionId(0), "work")
    }

    #[test]
    fn build_status_single_window() {
        let s = session();
        let status = build_status(&s);
        assert_eq!(status.session_name, "work");
        assert_eq!(status.windows.len(), 1);
        assert!(status.windows[0].is_active);
    }

    #[test]
    fn build_status_multiple_windows() {
        let mut s = session();
        s.add_window("vim");
        s.add_window("logs");

        let status = build_status(&s);
        assert_eq!(status.windows.len(), 3);

        let active_count = status.windows.iter().filter(|w| w.is_active).count();
        assert_eq!(active_count, 1);
    }

    #[test]
    fn pane_info_format() {
        let s = session();
        let status = build_status(&s);
        assert_eq!(status.pane_info, "pane 1/1");
    }

    #[test]
    fn render_status_line_basic() {
        let content = StatusBarContent {
            session_name: "s".into(),
            windows: vec![WindowEntry { index: 0, name: "w".into(), is_active: true }],
            pane_info: "pane 1/1".into(),
        };
        let line = render_status_line(&content, 40);
        assert!(line.contains("[s]"));
        assert!(line.contains("0:w*"));
        assert!(line.contains("pane 1/1"));
    }
}
