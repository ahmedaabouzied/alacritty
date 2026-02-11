//! Unix domain socket communication helpers.
//!
//! Provides stream-based message reading/writing over Unix sockets
//! using the length-prefixed JSON protocol from [`crate::protocol`].

use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::error::MuxResult;
use crate::protocol::{ClientMessage, ServerMessage, decode_message, encode_message};

/// Buffer for accumulating data from a socket stream.
#[derive(Debug)]
pub struct MessageReader {
    /// Internal buffer for partially received messages.
    buf: Vec<u8>,
}

impl Default for MessageReader {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageReader {
    /// Create a new empty message reader.
    pub fn new() -> Self {
        Self { buf: Vec::with_capacity(4096) }
    }

    /// Read from a stream and try to decode one complete message.
    ///
    /// Returns `Ok(Some(msg))` if a complete message was decoded,
    /// `Ok(None)` if more data is needed, or `Err` on I/O error.
    pub fn read_message<T, R>(&mut self, reader: &mut R) -> io::Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
        R: Read,
    {
        // First try to decode from existing buffer (may have leftover data).
        if let Some((msg, consumed)) = decode_message::<T>(&self.buf) {
            self.buf.drain(..consumed);
            return Ok(Some(msg));
        }

        // Read more data from the stream.
        let mut tmp = [0u8; 4096];
        match reader.read(&mut tmp) {
            Ok(0) => {
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, "connection closed"));
            },
            Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
            Err(e) => return Err(e),
        }

        if let Some((msg, consumed)) = decode_message::<T>(&self.buf) {
            self.buf.drain(..consumed);
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }
}

/// Write a single message to a stream.
pub fn write_message<T, W>(writer: &mut W, msg: &T) -> io::Result<()>
where
    T: serde::Serialize,
    W: Write,
{
    let data = encode_message(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    writer.write_all(&data)?;
    writer.flush()
}

/// Create a Unix listener at the given socket path.
///
/// Removes any stale socket file before binding.
#[cfg(unix)]
pub fn create_listener(path: &Path) -> MuxResult<UnixListener> {
    // Remove stale socket if it exists.
    if path.exists() {
        std::fs::remove_file(path).map_err(crate::error::MuxError::IoError)?;
    }

    // Ensure the parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(crate::error::MuxError::IoError)?;
    }

    let listener = UnixListener::bind(path).map_err(crate::error::MuxError::IoError)?;
    Ok(listener)
}

/// Connect to an existing Unix socket.
#[cfg(unix)]
pub fn connect(path: &Path) -> MuxResult<UnixStream> {
    let stream = UnixStream::connect(path).map_err(crate::error::MuxError::IoError)?;
    Ok(stream)
}

/// Send a client message over a stream.
#[cfg(unix)]
pub fn send_client_message(stream: &mut UnixStream, msg: &ClientMessage) -> io::Result<()> {
    write_message(stream, msg)
}

/// Send a server message over a stream.
#[cfg(unix)]
pub fn send_server_message(stream: &mut UnixStream, msg: &ServerMessage) -> io::Result<()> {
    write_message(stream, msg)
}

/// Clean up a socket file on drop.
#[cfg(unix)]
pub struct SocketGuard {
    path: std::path::PathBuf,
}

#[cfg(unix)]
impl SocketGuard {
    /// Create a guard that removes the socket file when dropped.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[cfg(unix)]
impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_reader_default() {
        let reader = MessageReader::default();
        assert!(reader.buf.is_empty());
    }

    #[test]
    fn write_and_read_message() {
        use crate::protocol::ClientMessage;

        let msg = ClientMessage::Attach;
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = MessageReader::new();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: Option<ClientMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(matches!(decoded, Some(ClientMessage::Attach)));
    }

    #[test]
    fn write_and_read_server_message() {
        use crate::protocol::ServerMessage;

        let msg = ServerMessage::ServerShutdown;
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = MessageReader::new();
        let mut cursor = std::io::Cursor::new(buf);
        let decoded: Option<ServerMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(matches!(decoded, Some(ServerMessage::ServerShutdown)));
    }

    #[test]
    fn partial_read_returns_none() {
        use crate::protocol::ClientMessage;

        let msg = ClientMessage::Detach;
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        // Only give the reader half the data.
        let half = buf.len() / 2;
        let mut reader = MessageReader::new();
        let mut cursor = std::io::Cursor::new(&buf[..half]);
        let decoded: Option<ClientMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(decoded.is_none());

        // Now give the rest.
        let mut cursor = std::io::Cursor::new(&buf[half..]);
        let decoded: Option<ClientMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(matches!(decoded, Some(ClientMessage::Detach)));
    }

    #[test]
    fn multiple_messages_in_sequence() {
        use crate::protocol::ClientMessage;

        let mut buf = Vec::new();
        write_message(&mut buf, &ClientMessage::Attach).unwrap();
        write_message(&mut buf, &ClientMessage::Detach).unwrap();

        let mut reader = MessageReader::new();
        let mut cursor = std::io::Cursor::new(buf);

        let msg1: Option<ClientMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(matches!(msg1, Some(ClientMessage::Attach)));

        let msg2: Option<ClientMessage> = reader.read_message(&mut cursor).unwrap();
        assert!(matches!(msg2, Some(ClientMessage::Detach)));
    }

    #[cfg(unix)]
    #[test]
    fn socket_guard_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        std::fs::write(&sock_path, "placeholder").unwrap();
        assert!(sock_path.exists());

        {
            let _guard = SocketGuard::new(&sock_path);
        }
        assert!(!sock_path.exists());
    }
}
