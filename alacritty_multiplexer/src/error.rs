//! Error types for the multiplexer crate.

use std::io;

/// Errors that can occur in the multiplexer.
#[derive(Debug, thiserror::Error)]
pub enum MuxError {
    /// A layout operation failed (e.g. pane too small to split).
    #[error("layout error: {0}")]
    LayoutError(String),

    /// The requested pane was not found.
    #[error("pane not found: {0}")]
    PaneNotFound(u32),

    /// The requested window was not found.
    #[error("window not found: {0}")]
    WindowNotFound(usize),

    /// A session-level operation failed.
    #[error("session error: {0}")]
    SessionError(String),

    /// Persistence (save/load) failed.
    #[error("persistence error: {0}")]
    PersistenceError(String),

    /// An I/O error occurred.
    #[error("io error: {0}")]
    IoError(#[from] io::Error),
}

/// Convenience type alias for multiplexer results.
pub type MuxResult<T> = Result<T, MuxError>;
