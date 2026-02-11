//! Client-side connection for attaching to a multiplexer session.
//!
//! When the user runs `alacritty mux attach -t <name>`, this module
//! handles connecting to the server socket, receiving state sync,
//! and forwarding input/output.

use std::path::Path;

use log::{error, info};

use alacritty_multiplexer::command::MuxCommand;
use alacritty_multiplexer::protocol::{ClientMessage, ServerMessage};
use alacritty_multiplexer::server::socket_path_for;
use alacritty_multiplexer::session::Session;
use alacritty_multiplexer::socket::{self, MessageReader};

/// State of a client connected to a multiplexer server.
#[cfg(unix)]
pub struct MuxClient {
    /// The Unix stream to the server.
    stream: std::os::unix::net::UnixStream,
    /// Message reader with internal buffer.
    reader: MessageReader,
}

#[cfg(unix)]
impl MuxClient {
    /// Connect to a named session.
    pub fn connect(session_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = socket_path_for(session_name);
        info!("Connecting to session '{}' at {}", session_name, path.display());
        let stream = socket::connect(&path)?;
        Ok(Self { stream, reader: MessageReader::new() })
    }

    /// Send an attach request to the server.
    pub fn attach(&mut self) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, &ClientMessage::Attach)
    }

    /// Send a detach request to the server.
    pub fn detach(&mut self) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, &ClientMessage::Detach)
    }

    /// Send raw terminal input to the server.
    pub fn send_input(&mut self, data: Vec<u8>) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, &ClientMessage::Input(data))
    }

    /// Send a resize notification to the server.
    pub fn send_resize(&mut self, rows: u16, cols: u16) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, &ClientMessage::Resize { rows, cols })
    }

    /// Send a multiplexer command to the server.
    pub fn send_command(&mut self, cmd: MuxCommand) -> std::io::Result<()> {
        socket::write_message(&mut self.stream, &ClientMessage::Command(cmd))
    }

    /// Try to read one server message.
    pub fn read_message(&mut self) -> std::io::Result<Option<ServerMessage>> {
        self.reader.read_message(&mut self.stream)
    }

    /// Block until a server message is received.
    pub fn recv_message(&mut self) -> std::io::Result<ServerMessage> {
        loop {
            if let Some(msg) = self.read_message()? {
                return Ok(msg);
            }
        }
    }

    /// Attach and wait for the initial state sync.
    pub fn attach_and_sync(&mut self) -> Result<Session, Box<dyn std::error::Error>> {
        self.attach()?;
        match self.recv_message()? {
            ServerMessage::StateSync(session) => Ok(session),
            other => Err(format!("Expected StateSync, got: {other:?}").into()),
        }
    }
}
