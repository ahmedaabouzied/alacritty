//! Client-server protocol for session attach/detach.
//!
//! Messages are exchanged as length-prefixed JSON over a Unix domain socket.

use serde::{Deserialize, Serialize};

use crate::command::MuxCommand;
use crate::layout::PaneId;
use crate::session::Session;

/// Messages sent from the client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Raw terminal input to forward to the active PTY.
    Input(Vec<u8>),
    /// Terminal was resized to (rows, cols).
    Resize { rows: u16, cols: u16 },
    /// A multiplexer command (e.g. split, navigate).
    Command(MuxCommand),
    /// Request to attach to the session.
    Attach,
    /// Request to detach from the session.
    Detach,
}

/// Messages sent from the server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Terminal output from a specific pane.
    Output { pane_id: PaneId, data: Vec<u8> },
    /// Full session state for synchronization on attach.
    StateSync(Session),
    /// A pane has exited.
    PaneExited(PaneId),
    /// Server is shutting down.
    ServerShutdown,
}

/// Encode a message as length-prefixed JSON bytes.
pub fn encode_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, serde_json::Error> {
    let json = serde_json::to_vec(msg)?;
    let len = (json.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&json);
    Ok(buf)
}

/// Decode a length-prefixed JSON message from a byte buffer.
///
/// Returns the decoded message and the number of bytes consumed,
/// or `None` if the buffer doesn't contain a complete message yet.
pub fn decode_message<T: for<'de> Deserialize<'de>>(buf: &[u8]) -> Option<(T, usize)> {
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let total = 4 + len;
    if buf.len() < total {
        return None;
    }
    let msg = serde_json::from_slice(&buf[4..total]).ok()?;
    Some((msg, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_client_message() {
        let msg = ClientMessage::Resize { rows: 24, cols: 80 };
        let encoded = encode_message(&msg).unwrap();
        let (decoded, consumed): (ClientMessage, _) = decode_message(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        match decoded {
            ClientMessage::Resize { rows, cols } => {
                assert_eq!(rows, 24);
                assert_eq!(cols, 80);
            },
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn roundtrip_server_message() {
        let msg = ServerMessage::PaneExited(PaneId(42));
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _): (ServerMessage, _) = decode_message(&encoded).unwrap();
        match decoded {
            ServerMessage::PaneExited(id) => assert_eq!(id, PaneId(42)),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn decode_incomplete_returns_none() {
        let msg = ClientMessage::Attach;
        let encoded = encode_message(&msg).unwrap();
        // Partial buffer.
        let partial = &encoded[..encoded.len() - 1];
        assert!(decode_message::<ClientMessage>(partial).is_none());
    }

    #[test]
    fn decode_too_short_returns_none() {
        assert!(decode_message::<ClientMessage>(&[0, 0]).is_none());
    }

    #[test]
    fn encode_input_message() {
        let msg = ClientMessage::Input(vec![27, 91, 65]); // ESC [ A
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _): (ClientMessage, _) = decode_message(&encoded).unwrap();
        match decoded {
            ClientMessage::Input(data) => assert_eq!(data, vec![27, 91, 65]),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn encode_command_message() {
        let msg = ClientMessage::Command(MuxCommand::SplitVertical);
        let encoded = encode_message(&msg).unwrap();
        let (decoded, _): (ClientMessage, _) = decode_message(&encoded).unwrap();
        match decoded {
            ClientMessage::Command(cmd) => assert_eq!(cmd, MuxCommand::SplitVertical),
            _ => panic!("wrong variant"),
        }
    }
}
