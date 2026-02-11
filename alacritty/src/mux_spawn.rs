//! Reusable PTY+Term spawning for multiplexer panes.
//!
//! Extracts the PTY and terminal creation logic from `WindowContext::new` into
//! a standalone function that can be called for each new pane.

use std::error::Error;
#[cfg(not(windows))]
use std::os::unix::io::AsRawFd;
use std::sync::Arc;

use log::info;

use alacritty_multiplexer::layout::PaneId;
use alacritty_terminal::event::Event as TerminalEvent;
use alacritty_terminal::event_loop::{EventLoop as PtyEventLoop, Notifier};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::tty;

use crate::config::UiConfig;
use crate::display::SizeInfo;
use crate::event::EventProxy;
use crate::mux_state::PaneState;

/// Spawn a new PTY + Term pair for a pane.
///
/// This mirrors the creation logic in `WindowContext::new` but returns a
/// self-contained `PaneState` that can be stored in `MuxState`.
pub fn spawn_pane(
    config: &UiConfig,
    size_info: &SizeInfo,
    event_proxy: EventProxy,
    pane_id: PaneId,
) -> Result<PaneState, Box<dyn Error>> {
    let pty_config = config.pty_config();

    info!(
        "Spawning pane {:?}: {:?} x {:?}",
        pane_id.0,
        size_info.screen_lines(),
        size_info.columns(),
    );

    // Create the terminal emulator.
    let terminal = Term::new(config.term_options(), size_info, event_proxy.clone());
    let terminal = Arc::new(FairMutex::new(terminal));

    // Create the PTY (forks a shell process).
    //
    // We use 0 as the window_id since panes don't correspond to OS windows.
    let pty = tty::new(&pty_config, (*size_info).into(), 0)?;

    #[cfg(not(windows))]
    let master_fd = pty.file().as_raw_fd();
    #[cfg(not(windows))]
    let shell_pid = pty.child().id();

    // Create the PTY I/O event loop on a background thread.
    let event_loop = PtyEventLoop::new(
        Arc::clone(&terminal),
        event_proxy,
        pty,
        pty_config.drain_on_exit,
        config.debug.ref_test,
    )?;

    let loop_tx = event_loop.channel();
    let io_thread = event_loop.spawn();

    Ok(PaneState {
        terminal,
        notifier: Notifier(loop_tx),
        io_thread: Some(io_thread),
        #[cfg(not(windows))]
        master_fd,
        #[cfg(not(windows))]
        shell_pid,
    })
}
