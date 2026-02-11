//! Server-side session management for detach/reattach.
//!
//! The server runs in headless mode, owns the PTYs and session state,
//! and communicates with clients over a Unix domain socket.

use std::path::PathBuf;

use crate::error::MuxResult;
use crate::persistence;
use crate::protocol::{ClientMessage, ServerMessage};
use crate::session::Session;

/// State of a running multiplexer server.
#[derive(Debug)]
pub struct ServerState {
    /// The session managed by this server.
    pub session: Session,
    /// Path to the Unix domain socket.
    pub socket_path: PathBuf,
    /// Whether the server should keep running.
    pub running: bool,
}

impl ServerState {
    /// Create a new server state for the given session.
    pub fn new(session: Session) -> MuxResult<Self> {
        let socket_path = socket_path_for(&session.name);
        Ok(Self { session, socket_path, running: true })
    }

    /// Process a client message and return the response(s).
    pub fn handle_message(&mut self, msg: ClientMessage) -> Vec<ServerMessage> {
        match msg {
            ClientMessage::Attach => vec![ServerMessage::StateSync(self.session.clone())],
            ClientMessage::Detach => Vec::new(),
            ClientMessage::Resize { rows, cols } => {
                self.handle_resize(rows, cols);
                Vec::new()
            },
            ClientMessage::Command(cmd) => {
                self.handle_command(cmd);
                vec![ServerMessage::StateSync(self.session.clone())]
            },
            ClientMessage::Input(_data) => {
                // Input forwarding to PTY is handled by the alacritty binary
                // crate (which owns the actual PTY handles), not here.
                Vec::new()
            },
            ClientMessage::RequestPaneContent(pane_id) => {
                // Terminal content is owned by the binary crate (Term<T>).
                // This message is forwarded to the MuxState layer which has
                // access to the actual terminal grids. We return an empty
                // PaneContent as a placeholder — the binary crate overrides
                // this with real grid data.
                vec![ServerMessage::PaneContent { pane_id, content: Vec::new(), cols: 0, rows: 0 }]
            },
        }
    }

    /// Request server shutdown.
    pub fn shutdown(&mut self) {
        self.running = false;
    }

    /// Save the session layout to disk for crash recovery.
    pub fn save_session(&self) -> MuxResult<()> {
        persistence::save_session(&self.session)
    }

    fn handle_resize(&mut self, _rows: u16, _cols: u16) {
        // Resize propagation to individual PTYs is handled by the binary
        // crate. Here we could update the session's notion of terminal size.
    }

    fn handle_command(&mut self, cmd: crate::command::MuxCommand) {
        use crate::command::MuxCommand;
        use crate::layout::Direction;

        match cmd {
            MuxCommand::SplitHorizontal => {
                let _ = self.session.split_active(Direction::Horizontal);
            },
            MuxCommand::SplitVertical => {
                let _ = self.session.split_active(Direction::Vertical);
            },
            MuxCommand::ClosePane => {
                if let Some(win) = self.session.active_win_mut() {
                    let pane = win.active_pane;
                    let _ = win.close_pane(pane);
                }
            },
            MuxCommand::NextPane => {
                if let Some(win) = self.session.active_win_mut() {
                    win.next_pane();
                }
            },
            MuxCommand::PrevPane => {
                if let Some(win) = self.session.active_win_mut() {
                    win.prev_pane();
                }
            },
            MuxCommand::NewWindow => {
                self.session.add_window("new");
            },
            MuxCommand::CloseWindow => {
                let idx = self.session.active_window;
                let _ = self.session.close_window(idx);
            },
            MuxCommand::NextWindow => self.session.next_window(),
            MuxCommand::PrevWindow => self.session.prev_window(),
            MuxCommand::SwitchToWindow(n) => {
                let idx = n as usize;
                if idx < self.session.windows.len() {
                    self.session.active_window = idx;
                }
            },
            MuxCommand::ToggleZoom => {
                if let Some(win) = self.session.active_win_mut() {
                    win.zoomed = !win.zoomed;
                }
            },
            MuxCommand::RenameWindow(name) => {
                if let Some(win) = self.session.active_win_mut() {
                    win.name = name;
                }
            },
            MuxCommand::DetachSession => {},
            MuxCommand::NavigatePane(_) | MuxCommand::ResizePane(..) => {
                // Direction-based navigation and resize require layout geometry
                // which is handled at the rendering layer.
            },
            MuxCommand::ScrollbackMode => {},
        }
    }
}

/// Compute the socket path for a named session.
pub fn socket_path_for(name: &str) -> PathBuf {
    persistence::socket_dir().join(format!("{name}.sock"))
}

/// List active sessions by scanning the socket directory.
pub fn list_active_sessions() -> Vec<String> {
    let dir = persistence::socket_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "sock") {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().into_owned());
                }
            }
        }
    }
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::MuxCommand;
    use crate::layout::PaneId;
    use crate::protocol::ClientMessage;
    use crate::session::SessionId;

    fn server() -> ServerState {
        let session = Session::new(SessionId(0), "test");
        ServerState::new(session).unwrap()
    }

    #[test]
    fn attach_returns_state_sync() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::Attach);
        assert_eq!(responses.len(), 1);
        assert!(matches!(&responses[0], ServerMessage::StateSync(_)));
    }

    #[test]
    fn detach_returns_empty() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::Detach);
        assert!(responses.is_empty());
    }

    #[test]
    fn command_returns_state_sync() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::Command(MuxCommand::NewWindow));
        assert_eq!(responses.len(), 1);
        assert_eq!(srv.session.windows.len(), 2);
    }

    #[test]
    fn split_command() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::SplitVertical));
        let win = srv.session.active_win().unwrap();
        assert_eq!(win.layout.pane_count(), 2);
    }

    #[test]
    fn close_pane_command() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::SplitVertical));
        srv.handle_message(ClientMessage::Command(MuxCommand::ClosePane));
        let win = srv.session.active_win().unwrap();
        assert_eq!(win.layout.pane_count(), 1);
    }

    #[test]
    fn navigate_panes() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::SplitVertical));
        let before = srv.session.active_pane_id().unwrap();
        srv.handle_message(ClientMessage::Command(MuxCommand::NextPane));
        let after = srv.session.active_pane_id().unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn window_commands() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::NewWindow));
        assert_eq!(srv.session.windows.len(), 2);

        srv.handle_message(ClientMessage::Command(MuxCommand::PrevWindow));
        assert_eq!(srv.session.active_window, 0);

        srv.handle_message(ClientMessage::Command(MuxCommand::NextWindow));
        assert_eq!(srv.session.active_window, 1);

        srv.handle_message(ClientMessage::Command(MuxCommand::SwitchToWindow(0)));
        assert_eq!(srv.session.active_window, 0);
    }

    #[test]
    fn toggle_zoom() {
        let mut srv = server();
        assert!(!srv.session.active_win().unwrap().zoomed);
        srv.handle_message(ClientMessage::Command(MuxCommand::ToggleZoom));
        assert!(srv.session.active_win().unwrap().zoomed);
        srv.handle_message(ClientMessage::Command(MuxCommand::ToggleZoom));
        assert!(!srv.session.active_win().unwrap().zoomed);
    }

    #[test]
    fn rename_window() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::RenameWindow("editor".into())));
        assert_eq!(srv.session.active_win().unwrap().name, "editor");
    }

    #[test]
    fn close_window() {
        let mut srv = server();
        srv.handle_message(ClientMessage::Command(MuxCommand::NewWindow));
        srv.handle_message(ClientMessage::Command(MuxCommand::CloseWindow));
        assert_eq!(srv.session.windows.len(), 1);
    }

    #[test]
    fn shutdown() {
        let mut srv = server();
        assert!(srv.running);
        srv.shutdown();
        assert!(!srv.running);
    }

    #[test]
    fn socket_path_format() {
        let path = socket_path_for("work");
        assert!(path.to_string_lossy().ends_with("work.sock"));
    }

    #[test]
    fn input_message_returns_empty() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::Input(vec![65]));
        assert!(responses.is_empty());
    }

    #[test]
    fn resize_message_returns_empty() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::Resize { rows: 24, cols: 80 });
        assert!(responses.is_empty());
    }

    #[test]
    fn request_pane_content_returns_placeholder() {
        let mut srv = server();
        let responses = srv.handle_message(ClientMessage::RequestPaneContent(PaneId(0)));
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            ServerMessage::PaneContent { pane_id, cols, rows, .. } => {
                assert_eq!(*pane_id, PaneId(0));
                // Placeholder values from multiplexer crate layer.
                assert_eq!(*cols, 0);
                assert_eq!(*rows, 0);
            },
            _ => panic!("expected PaneContent"),
        }
    }
}
