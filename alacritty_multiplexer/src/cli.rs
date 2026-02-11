//! CLI subcommand definitions for session management.
//!
//! These types define the `alacritty mux` subcommands used to create,
//! attach to, list, and kill multiplexer sessions.

use serde::{Deserialize, Serialize};

/// Top-level mux subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MuxSubcommand {
    /// Start a new session (server + client).
    New(NewOptions),
    /// Attach to an existing session.
    Attach(AttachOptions),
    /// List active sessions.
    List,
    /// Kill a session.
    Kill(KillOptions),
}

/// Options for `mux new`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOptions {
    /// Session name (auto-generated if not provided).
    pub session_name: Option<String>,
}

/// Options for `mux attach`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachOptions {
    /// Target session name.
    pub target: String,
}

/// Options for `mux kill`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillOptions {
    /// Target session name.
    pub target: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_options_serializable() {
        let opts = NewOptions { session_name: Some("work".into()) };
        let json = serde_json::to_string(&opts).unwrap();
        let restored: NewOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.session_name.as_deref(), Some("work"));
    }

    #[test]
    fn subcommand_variants() {
        let cmds = vec![
            MuxSubcommand::New(NewOptions { session_name: None }),
            MuxSubcommand::Attach(AttachOptions { target: "s".into() }),
            MuxSubcommand::List,
            MuxSubcommand::Kill(KillOptions { target: "s".into() }),
        ];
        for cmd in &cmds {
            let json = serde_json::to_string(cmd).unwrap();
            let _: MuxSubcommand = serde_json::from_str(&json).unwrap();
        }
    }
}
