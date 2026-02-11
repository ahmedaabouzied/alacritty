//! Leader key state machine for multiplexer input interception.
//!
//! When the user presses a leader key (default: Ctrl-Space or Ctrl-B),
//! the multiplexer enters `WaitingForCommand` mode. The next keypress
//! is mapped to a `MuxCommand`. If no valid key arrives within the
//! timeout, the leader press is forwarded to the PTY.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use winit::event::KeyEvent;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use alacritty_multiplexer::command::{LeaderKeyConfig, MuxCommand};
use alacritty_multiplexer::layout::Direction;

/// Current state of the multiplexer input layer.
#[derive(Debug, Clone)]
pub enum MuxInputState {
    /// Normal mode — all input goes to the PTY.
    Normal,
    /// Leader key was pressed, waiting for the command key.
    WaitingForCommand {
        /// When the leader key was pressed.
        entered_at: Instant,
    },
}

impl Default for MuxInputState {
    fn default() -> Self {
        MuxInputState::Normal
    }
}

/// Result of processing a key through the multiplexer input layer.
#[derive(Debug)]
pub enum MuxKeyResult {
    /// Key was consumed by the multiplexer (leader key or command key).
    Consumed(Option<MuxCommand>),
    /// Key should be forwarded to the PTY as normal.
    Forward,
}

/// Process a key event through the multiplexer input layer.
///
/// Returns how the key should be handled.
pub fn process_mux_key(
    state: &mut MuxInputState,
    key: &KeyEvent,
    mods: ModifiersState,
    leader_config: &LeaderKeyConfig,
    bindings: &HashMap<String, MuxCommand>,
) -> MuxKeyResult {
    let timeout = Duration::from_millis(leader_config.timeout_ms);

    match state {
        MuxInputState::Normal => {
            if is_leader_key(key, mods, leader_config) {
                *state = MuxInputState::WaitingForCommand { entered_at: Instant::now() };
                MuxKeyResult::Consumed(None)
            } else {
                MuxKeyResult::Forward
            }
        },
        MuxInputState::WaitingForCommand { entered_at } => {
            // Check timeout.
            if entered_at.elapsed() > timeout {
                *state = MuxInputState::Normal;
                return MuxKeyResult::Forward;
            }

            // Double-tap leader → send literal leader key to PTY.
            if is_leader_key(key, mods, leader_config) {
                *state = MuxInputState::Normal;
                return MuxKeyResult::Forward;
            }

            // Try to map the key to a command.
            *state = MuxInputState::Normal;

            if let Some(cmd) = map_command_key(key, mods, bindings) {
                MuxKeyResult::Consumed(Some(cmd))
            } else {
                // Unknown key after leader — discard and return to normal.
                MuxKeyResult::Consumed(None)
            }
        },
    }
}

/// Check whether this key event matches one of the configured leader keys.
fn is_leader_key(key: &KeyEvent, mods: ModifiersState, config: &LeaderKeyConfig) -> bool {
    config.keys.iter().any(|k| matches_leader_spec(key, mods, k))
}

/// Match a key event against a leader key specification string.
///
/// Supported formats: "Control-Space", "Control-b", etc.
fn matches_leader_spec(key: &KeyEvent, mods: ModifiersState, spec: &str) -> bool {
    let parts: Vec<&str> = spec.split('-').collect();
    let (required_mods, key_part) = parse_key_spec(&parts);

    if mods != required_mods {
        return false;
    }

    match key_part {
        "Space" => matches!(key.logical_key, Key::Named(NamedKey::Space)),
        s if s.len() == 1 => {
            let ch = s.chars().next().unwrap();
            key.logical_key == Key::Character(ch.to_string().as_str().into())
        },
        _ => false,
    }
}

/// Parse modifier-key spec parts into (modifiers, key_name).
fn parse_key_spec<'a>(parts: &[&'a str]) -> (ModifiersState, &'a str) {
    let mut mods = ModifiersState::empty();
    let mut key_name = "";
    for &part in parts {
        match part {
            "Control" | "Ctrl" => mods |= ModifiersState::CONTROL,
            "Shift" => mods |= ModifiersState::SHIFT,
            "Alt" => mods |= ModifiersState::ALT,
            "Super" => mods |= ModifiersState::SUPER,
            other => key_name = other,
        }
    }
    (mods, key_name)
}

/// Map a command key to a MuxCommand using the bindings table.
fn map_command_key(
    key: &KeyEvent,
    mods: ModifiersState,
    bindings: &HashMap<String, MuxCommand>,
) -> Option<MuxCommand> {
    let key_str = key_to_string(key, mods)?;
    bindings.get(&key_str).cloned()
}

/// Convert a key event to a string representation for binding lookup.
fn key_to_string(key: &KeyEvent, mods: ModifiersState) -> Option<String> {
    let base = match &key.logical_key {
        Key::Character(c) => c.to_string(),
        Key::Named(NamedKey::ArrowUp) => "Up".into(),
        Key::Named(NamedKey::ArrowDown) => "Down".into(),
        Key::Named(NamedKey::ArrowLeft) => "Left".into(),
        Key::Named(NamedKey::ArrowRight) => "Right".into(),
        Key::Named(NamedKey::Space) => "Space".into(),
        _ => return None,
    };

    if mods.contains(ModifiersState::CONTROL) { Some(format!("Ctrl-{base}")) } else { Some(base) }
}

/// Build the default keybinding map (leader-mode second key → command).
pub fn default_bindings() -> HashMap<String, MuxCommand> {
    let mut m = HashMap::new();

    // Pane splitting.
    m.insert("\"".into(), MuxCommand::SplitHorizontal);
    m.insert("-".into(), MuxCommand::SplitHorizontal);
    m.insert("%".into(), MuxCommand::SplitVertical);
    m.insert("|".into(), MuxCommand::SplitVertical);

    // Pane management.
    m.insert("x".into(), MuxCommand::ClosePane);
    m.insert("o".into(), MuxCommand::NextPane);
    m.insert(";".into(), MuxCommand::PrevPane);
    m.insert("z".into(), MuxCommand::ToggleZoom);

    // Pane navigation.
    m.insert("Up".into(), MuxCommand::NavigatePane(Direction::Horizontal));
    m.insert("Down".into(), MuxCommand::NavigatePane(Direction::Horizontal));
    m.insert("Left".into(), MuxCommand::NavigatePane(Direction::Vertical));
    m.insert("Right".into(), MuxCommand::NavigatePane(Direction::Vertical));

    // Pane resize (Ctrl+arrow).
    m.insert("Ctrl-Up".into(), MuxCommand::ResizePane(Direction::Horizontal, -1));
    m.insert("Ctrl-Down".into(), MuxCommand::ResizePane(Direction::Horizontal, 1));
    m.insert("Ctrl-Left".into(), MuxCommand::ResizePane(Direction::Vertical, -1));
    m.insert("Ctrl-Right".into(), MuxCommand::ResizePane(Direction::Vertical, 1));

    // Window management.
    m.insert("c".into(), MuxCommand::NewWindow);
    m.insert("n".into(), MuxCommand::NextWindow);
    m.insert("p".into(), MuxCommand::PrevWindow);
    m.insert("d".into(), MuxCommand::DetachSession);
    m.insert("[".into(), MuxCommand::ScrollbackMode);

    // Window switching by number.
    for i in 0..=9u8 {
        m.insert(i.to_string(), MuxCommand::SwitchToWindow(i));
    }

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leader_config() -> LeaderKeyConfig {
        LeaderKeyConfig::default()
    }

    #[test]
    fn default_bindings_has_split() {
        let b = default_bindings();
        assert_eq!(b.get("\""), Some(&MuxCommand::SplitHorizontal));
        assert_eq!(b.get("%"), Some(&MuxCommand::SplitVertical));
        assert_eq!(b.get("c"), Some(&MuxCommand::NewWindow));
        assert_eq!(b.get("d"), Some(&MuxCommand::DetachSession));
    }

    #[test]
    fn default_bindings_has_window_numbers() {
        let b = default_bindings();
        for i in 0..=9u8 {
            assert_eq!(b.get(&i.to_string()), Some(&MuxCommand::SwitchToWindow(i)));
        }
    }

    #[test]
    fn parse_key_spec_ctrl_space() {
        let parts = vec!["Control", "Space"];
        let (mods, key) = parse_key_spec(&parts);
        assert!(mods.contains(ModifiersState::CONTROL));
        assert_eq!(key, "Space");
    }

    #[test]
    fn parse_key_spec_ctrl_b() {
        let parts = vec!["Control", "b"];
        let (mods, key) = parse_key_spec(&parts);
        assert!(mods.contains(ModifiersState::CONTROL));
        assert_eq!(key, "b");
    }
}
