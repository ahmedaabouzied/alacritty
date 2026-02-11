//! Execute multiplexer commands against session and terminal state.

use std::collections::HashMap;

use log::info;

use alacritty_multiplexer::command::{LeaderKeyConfig, MuxCommand};
use alacritty_multiplexer::layout::{Direction, PaneId};
use alacritty_multiplexer::resize::resize_pane;

use crate::config::UiConfig;
use crate::display::SizeInfo;
use crate::event::EventProxy;
use crate::mux_spawn;
use crate::mux_state::MuxState;

/// Execute a multiplexer command, updating the session and spawning/killing
/// PTYs as needed. Returns `true` if the command triggered a redraw.
pub fn execute_command(
    mux: &mut MuxState,
    cmd: MuxCommand,
    config: &UiConfig,
    size_info: &SizeInfo,
    event_proxy: &EventProxy,
) -> bool {
    match cmd {
        MuxCommand::SplitHorizontal => {
            split(mux, Direction::Horizontal, config, size_info, event_proxy)
        },
        MuxCommand::SplitVertical => {
            split(mux, Direction::Vertical, config, size_info, event_proxy)
        },
        MuxCommand::ClosePane => close_pane(mux),
        MuxCommand::NextPane => nav_next_pane(mux),
        MuxCommand::PrevPane => nav_prev_pane(mux),
        MuxCommand::NewWindow => new_window(mux, config, size_info, event_proxy),
        MuxCommand::CloseWindow => close_window(mux),
        MuxCommand::NextWindow => {
            mux.session.next_window();
            true
        },
        MuxCommand::PrevWindow => {
            mux.session.prev_window();
            true
        },
        MuxCommand::SwitchToWindow(n) => switch_to_window(mux, n),
        MuxCommand::ToggleZoom => toggle_zoom(mux),
        MuxCommand::ResizePane(dir, delta) => resize(mux, dir, delta),
        MuxCommand::DetachSession => {
            info!("Detach requested");
            false
        },
        MuxCommand::ScrollbackMode => false,
        MuxCommand::NavigatePane(_) => nav_next_pane(mux),
        MuxCommand::RenameWindow(name) => rename_window(mux, name),
    }
}

fn split(
    mux: &mut MuxState,
    dir: Direction,
    config: &UiConfig,
    size_info: &SizeInfo,
    event_proxy: &EventProxy,
) -> bool {
    let new_pane_id = match mux.session.split_active(dir) {
        Ok(id) => id,
        Err(e) => {
            info!("Split failed: {e}");
            return false;
        },
    };

    match mux_spawn::spawn_pane(config, size_info, event_proxy.clone(), new_pane_id) {
        Ok(state) => {
            mux.register_pane(new_pane_id, state);
            true
        },
        Err(e) => {
            info!("Failed to spawn pane: {e}");
            false
        },
    }
}

fn close_pane(mux: &mut MuxState) -> bool {
    let win = match mux.session.active_win_mut() {
        Some(w) => w,
        None => return false,
    };
    let pane_id = win.active_pane;
    match win.close_pane(pane_id) {
        Ok(empty) => {
            if let Some(mut pane_state) = mux.remove_pane(pane_id) {
                // Notify the I/O thread to shut down (it will detect the dropped
                // notifier or the PTY close).
                drop(pane_state.io_thread.take());
            }
            if empty {
                let idx = mux.session.active_window;
                let _ = mux.session.close_window(idx);
            }
            true
        },
        Err(e) => {
            info!("Close pane failed: {e}");
            false
        },
    }
}

fn nav_next_pane(mux: &mut MuxState) -> bool {
    if let Some(win) = mux.session.active_win_mut() {
        win.next_pane();
        true
    } else {
        false
    }
}

fn nav_prev_pane(mux: &mut MuxState) -> bool {
    if let Some(win) = mux.session.active_win_mut() {
        win.prev_pane();
        true
    } else {
        false
    }
}

fn new_window(
    mux: &mut MuxState,
    config: &UiConfig,
    size_info: &SizeInfo,
    event_proxy: &EventProxy,
) -> bool {
    let name = format!("{}", mux.session.windows.len());
    mux.session.add_window(&name);

    // The new window has a default pane — spawn a PTY for it.
    if let Some(pane_id) = mux.session.active_pane_id() {
        match mux_spawn::spawn_pane(config, size_info, event_proxy.clone(), pane_id) {
            Ok(state) => {
                mux.register_pane(pane_id, state);
            },
            Err(e) => info!("Failed to spawn pane for new window: {e}"),
        }
    }
    true
}

fn close_window(mux: &mut MuxState) -> bool {
    let idx = mux.session.active_window;
    // Remove all panes in this window.
    if let Some(win) = mux.session.windows.get(idx) {
        let pane_ids: Vec<PaneId> = win.layout.pane_ids();
        for id in pane_ids {
            if let Some(mut ps) = mux.remove_pane(id) {
                drop(ps.io_thread.take());
            }
        }
    }
    let _ = mux.session.close_window(idx);
    true
}

fn switch_to_window(mux: &mut MuxState, n: u8) -> bool {
    let idx = n as usize;
    if idx < mux.session.windows.len() {
        mux.session.active_window = idx;
        true
    } else {
        false
    }
}

fn toggle_zoom(mux: &mut MuxState) -> bool {
    if let Some(win) = mux.session.active_win_mut() {
        win.zoomed = !win.zoomed;
        true
    } else {
        false
    }
}

fn resize(mux: &mut MuxState, dir: Direction, delta: i16) -> bool {
    let win = match mux.session.active_win_mut() {
        Some(w) => w,
        None => return false,
    };
    let pane_id = win.active_pane;
    let d = delta as f32 * 0.05;
    resize_pane(&mut win.layout, pane_id, d).is_ok()
}

fn rename_window(mux: &mut MuxState, name: String) -> bool {
    if let Some(win) = mux.session.active_win_mut() {
        win.name = name;
        true
    } else {
        false
    }
}

/// Rebuild the leader key config and bindings map from the current UiConfig.
///
/// Called when config is hot-reloaded to pick up changes to
/// `[multiplexer.keybindings]`, `leader_keys`, etc.
pub fn rebuild_config(config: &UiConfig) -> (LeaderKeyConfig, HashMap<String, MuxCommand>) {
    let mux_config = &config.multiplexer;

    let leader_config = LeaderKeyConfig {
        keys: mux_config.leader_keys.clone(),
        timeout_ms: mux_config.leader_timeout_ms,
    };

    let bindings = mux_config.keybindings.to_bindings_map();

    (leader_config, bindings)
}

/// Propagate a window resize to all pane PTYs.
///
/// Recalculates pane rects from the session layout, then resizes each
/// pane's PTY+Term to its new cell dimensions.
pub fn propagate_resize(mux: &mut MuxState, size_info: &SizeInfo) {
    let cell_width = size_info.cell_width();
    let cell_height = size_info.cell_height();

    let usable_cols = (size_info.width() / cell_width) as u16;
    let total_rows = (size_info.height() / cell_height) as u16;
    let usable_rows = total_rows.saturating_sub(1);

    let total_area = alacritty_multiplexer::rect::Rect::new(0, 0, usable_cols, usable_rows);

    let win = match mux.session.active_win() {
        Some(w) => w,
        None => return,
    };

    let rects = win.pane_rects(total_area);

    for (pane_id, rect) in &rects {
        if let Some(pane_state) = mux.panes.get(pane_id) {
            let new_cols = rect.width as usize;
            let new_rows = rect.height as usize;
            let pixel_width = rect.width as f32 * cell_width;
            let pixel_height = rect.height as f32 * cell_height;

            // Resize the terminal grid.
            let mut term = pane_state.terminal.lock();
            let size =
                SizeInfo::new(pixel_width, pixel_height, cell_width, cell_height, 0.0, 0.0, false);
            term.resize(size);
            drop(term);

            // Notify the PTY event loop of the new size.
            let window_size = alacritty_terminal::event::WindowSize {
                num_lines: new_rows as u16,
                num_cols: new_cols as u16,
                cell_width: cell_width as u16,
                cell_height: cell_height as u16,
            };
            let _ = pane_state
                .notifier
                .0
                .send(alacritty_terminal::event_loop::Msg::Resize(window_size));
        }
    }
}
