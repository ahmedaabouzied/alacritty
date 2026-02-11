//! Multiplexer state holding multiple terminals and PTYs.
//!
//! This module is gated behind `#[cfg(feature = "multiplexer")]`.

use std::collections::HashMap;
#[cfg(not(windows))]
use std::os::unix::io::RawFd;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_multiplexer::layout::PaneId;
use alacritty_multiplexer::session::{Session, SessionId};
use alacritty_terminal::event_loop::{EventLoop as PtyEventLoop, Notifier};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::tty;

use crate::event::EventProxy;

/// Per-pane terminal state.
pub struct PaneState {
    /// The terminal emulator for this pane.
    pub terminal: Arc<FairMutex<Term<EventProxy>>>,
    /// Notifier to write to this pane's PTY.
    pub notifier: Notifier,
    /// I/O thread handle.
    pub io_thread: Option<JoinHandle<(PtyEventLoop<tty::Pty, EventProxy>, alacritty_terminal::event_loop::State)>>,
    /// Master file descriptor for this PTY (Unix only).
    #[cfg(not(windows))]
    pub master_fd: RawFd,
    /// Shell PID.
    #[cfg(not(windows))]
    pub shell_pid: u32,
}

/// Holds the multiplexer session and all per-pane terminal state.
pub struct MuxState {
    /// The logical session (layout, windows, pane metadata).
    pub session: Session,
    /// Per-pane terminal + PTY state, keyed by PaneId.
    pub panes: HashMap<PaneId, PaneState>,
}

impl MuxState {
    /// Create a new multiplexer state with a default session.
    pub fn new(session: Session) -> Self {
        Self { session, panes: HashMap::new() }
    }

    /// Register a pane's terminal state.
    pub fn register_pane(&mut self, id: PaneId, state: PaneState) {
        self.panes.insert(id, state);
    }

    /// Remove a pane's terminal state and return it.
    pub fn remove_pane(&mut self, id: PaneId) -> Option<PaneState> {
        self.panes.remove(&id)
    }

    /// Get the active pane's terminal.
    pub fn active_terminal(&self) -> Option<&Arc<FairMutex<Term<EventProxy>>>> {
        let pane_id = self.session.active_pane_id()?;
        self.panes.get(&pane_id).map(|p| &p.terminal)
    }

    /// Get the active pane's notifier.
    pub fn active_notifier(&self) -> Option<&Notifier> {
        let pane_id = self.session.active_pane_id()?;
        self.panes.get(&pane_id).map(|p| &p.notifier)
    }

    /// Get a pane's terminal by ID.
    pub fn terminal_for(&self, id: PaneId) -> Option<&Arc<FairMutex<Term<EventProxy>>>> {
        self.panes.get(&id).map(|p| &p.terminal)
    }

    /// Get a pane's notifier by ID.
    pub fn notifier_for(&self, id: PaneId) -> Option<&Notifier> {
        self.panes.get(&id).map(|p| &p.notifier)
    }

    /// Get all active pane IDs.
    pub fn active_pane_ids(&self) -> Vec<PaneId> {
        self.panes.keys().copied().collect()
    }

    /// Get all master file descriptors for PTY polling (Unix only).
    #[cfg(not(windows))]
    pub fn pty_fds(&self) -> Vec<(PaneId, RawFd)> {
        self.panes.iter().map(|(&id, ps)| (id, ps.master_fd)).collect()
    }

    /// Check if any pane has the given file descriptor (Unix only).
    #[cfg(not(windows))]
    pub fn pane_for_fd(&self, fd: RawFd) -> Option<PaneId> {
        self.panes.iter().find(|(_, ps)| ps.master_fd == fd).map(|(&id, _)| id)
    }
}
