//! Pane metadata.
//!
//! The actual `Term` and PTY live in the main `alacritty` crate since they
//! depend on windowing context. This crate tracks ids and metadata only.

use serde::{Deserialize, Serialize};

use crate::layout::PaneId;

/// Metadata for a single pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pane {
    /// Unique pane identifier.
    pub id: PaneId,
    /// Display title (e.g. shell command or working directory).
    pub title: String,
}

impl Pane {
    /// Create a new pane with a default title.
    pub fn new(id: PaneId) -> Self {
        Self { id, title: String::new() }
    }
}
