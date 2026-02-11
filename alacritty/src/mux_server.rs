//! Server-side socket listener for multiplexer sessions.
//!
//! When Alacritty runs in server mode (`--server`), this module manages
//! the Unix domain socket listener, accepts client connections, and
//! dispatches messages between clients and the session.

#[cfg(unix)]
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;

use log::{error, info};

use alacritty_multiplexer::protocol::{ClientMessage, ServerMessage};
use alacritty_multiplexer::server::ServerState;
use alacritty_multiplexer::session::{Session, SessionId};
use alacritty_multiplexer::socket::{self, MessageReader, SocketGuard};

use crate::mux_state::MuxState;

/// State for a running multiplexer server.
pub struct MuxServer {
    /// The server-side session state.
    pub server_state: ServerState,
    /// Socket listener.
    #[cfg(unix)]
    pub listener: UnixListener,
    /// Guard that cleans up the socket file on drop.
    #[cfg(unix)]
    pub _socket_guard: SocketGuard,
}

#[cfg(unix)]
impl MuxServer {
    /// Start a new server for the given session name.
    pub fn start(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let session = Session::new(SessionId(0), name);
        let server_state = ServerState::new(session)?;
        let socket_path = server_state.socket_path.clone();

        let listener = socket::create_listener(&socket_path)?;
        listener.set_nonblocking(true)?;

        info!("Server listening on {}", socket_path.display());

        let guard = SocketGuard::new(&socket_path);

        Ok(Self { server_state, listener, _socket_guard: guard })
    }

    /// Accept a pending connection, if any.
    ///
    /// Returns a new `ClientConnection` or `None` if no client is waiting.
    pub fn accept(&self) -> Option<ClientConnection> {
        match self.listener.accept() {
            Ok((stream, _)) => {
                info!("Client connected");
                Some(ClientConnection { stream, reader: MessageReader::new() })
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(e) => {
                error!("Accept error: {e}");
                None
            },
        }
    }

    /// Process messages from a client connection.
    ///
    /// Returns `false` if the client disconnected or sent a Detach.
    pub fn process_client(&mut self, client: &mut ClientConnection) -> bool {
        let msg = match client.read_message() {
            Ok(Some(msg)) => msg,
            Ok(None) => return true,
            Err(_) => return false,
        };

        // Handle detach specially.
        if matches!(&msg, ClientMessage::Detach) {
            info!("Client detached");
            return false;
        }

        let responses = self.server_state.handle_message(msg);
        for response in &responses {
            if client.write_message(response).is_err() {
                return false;
            }
        }

        true
    }
}

/// A connected client session.
pub struct ClientConnection {
    /// The Unix stream.
    #[cfg(unix)]
    stream: std::os::unix::net::UnixStream,
    /// Message reader with internal buffer.
    reader: MessageReader,
}

impl ClientConnection {
    /// Try to read one client message.
    fn read_message(&mut self) -> std::io::Result<Option<ClientMessage>> {
        self.reader.read_message(&mut self.stream)
    }

    /// Write a server message to the client.
    fn write_message(&mut self, msg: &ServerMessage) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, msg)
    }

    /// Send a full state sync to the client.
    pub fn send_state_sync(&mut self, session: &Session) -> std::io::Result<()> {
        let msg = ServerMessage::StateSync(session.clone());
        self.write_message(&msg)
    }

    /// Send terminal output for a pane.
    pub fn send_output(
        &mut self,
        pane_id: alacritty_multiplexer::layout::PaneId,
        data: Vec<u8>,
    ) -> std::io::Result<()> {
        let msg = ServerMessage::Output { pane_id, data };
        self.write_message(&msg)
    }

    /// Notify client that a pane has exited.
    pub fn send_pane_exited(
        &mut self,
        pane_id: alacritty_multiplexer::layout::PaneId,
    ) -> std::io::Result<()> {
        let msg = ServerMessage::PaneExited(pane_id);
        self.write_message(&msg)
    }

    /// Notify client that server is shutting down.
    pub fn send_shutdown(&mut self) -> std::io::Result<()> {
        self.write_message(&ServerMessage::ServerShutdown)
    }
}
