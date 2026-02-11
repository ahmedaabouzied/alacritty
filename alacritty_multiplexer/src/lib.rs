//! Terminal multiplexer for Alacritty.
//!
//! This crate provides pane splitting, window/tab management, session
//! persistence, and status-bar content generation. It is intentionally
//! independent of the rendering and PTY layers so that it can be tested
//! in isolation.

pub mod cli;
pub mod command;
pub mod config;
pub mod error;
pub mod layout;
pub mod pane;
pub mod persistence;
pub mod protocol;
pub mod rect;
pub mod resize;
pub mod server;
pub mod session;
pub mod socket;
pub mod split;
pub mod statusbar;
pub mod window;
