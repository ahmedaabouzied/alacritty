//! Multiplexer configuration schema.
//!
//! Corresponds to the `[multiplexer]` section in `alacritty.toml`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level multiplexer configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MultiplexerConfig {
    /// Whether the multiplexer is enabled.
    pub enabled: bool,
    /// Whether to show the status bar.
    pub status_bar: bool,
    /// Leader key(s) that activate command mode.
    pub leader_keys: Vec<String>,
    /// Timeout in ms before leader mode expires.
    pub leader_timeout_ms: u64,
    /// Key → action bindings for leader mode.
    pub keybindings: KeybindingsConfig,
    /// Status bar appearance.
    pub status_bar_config: StatusBarConfig,
}

impl Default for MultiplexerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            status_bar: true,
            leader_keys: vec!["Control-Space".into(), "Control-b".into()],
            leader_timeout_ms: 1000,
            keybindings: KeybindingsConfig::default(),
            status_bar_config: StatusBarConfig::default(),
        }
    }
}

/// Keybindings for leader mode (key pressed after leader).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    /// Key for horizontal split.
    pub split_horizontal: String,
    /// Alternate key for horizontal split.
    pub split_horizontal_alt: String,
    /// Key for vertical split.
    pub split_vertical: String,
    /// Alternate key for vertical split.
    pub split_vertical_alt: String,
    /// Key to close pane.
    pub close_pane: String,
    /// Key for next pane.
    pub next_pane: String,
    /// Key for previous pane.
    pub prev_pane: String,
    /// Key to create new window.
    pub new_window: String,
    /// Key for next window.
    pub next_window: String,
    /// Key for previous window.
    pub prev_window: String,
    /// Key to detach.
    pub detach: String,
    /// Key to rename window.
    pub rename_window: String,
    /// Key to toggle zoom.
    pub toggle_zoom: String,
    /// Key to enter scrollback mode.
    pub scrollback_mode: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            split_horizontal: "\"".into(),
            split_horizontal_alt: "-".into(),
            split_vertical: "%".into(),
            split_vertical_alt: "|".into(),
            close_pane: "x".into(),
            next_pane: "o".into(),
            prev_pane: ";".into(),
            new_window: "c".into(),
            next_window: "n".into(),
            prev_window: "p".into(),
            detach: "d".into(),
            rename_window: ",".into(),
            toggle_zoom: "z".into(),
            scrollback_mode: "[".into(),
        }
    }
}

impl KeybindingsConfig {
    /// Convert keybindings config into a key → action HashMap.
    pub fn to_bindings_map(&self) -> HashMap<String, crate::command::MuxCommand> {
        use crate::command::MuxCommand;

        let mut m = HashMap::new();
        m.insert(self.split_horizontal.clone(), MuxCommand::SplitHorizontal);
        m.insert(self.split_horizontal_alt.clone(), MuxCommand::SplitHorizontal);
        m.insert(self.split_vertical.clone(), MuxCommand::SplitVertical);
        m.insert(self.split_vertical_alt.clone(), MuxCommand::SplitVertical);
        m.insert(self.close_pane.clone(), MuxCommand::ClosePane);
        m.insert(self.next_pane.clone(), MuxCommand::NextPane);
        m.insert(self.prev_pane.clone(), MuxCommand::PrevPane);
        m.insert(self.new_window.clone(), MuxCommand::NewWindow);
        m.insert(self.next_window.clone(), MuxCommand::NextWindow);
        m.insert(self.prev_window.clone(), MuxCommand::PrevWindow);
        m.insert(self.detach.clone(), MuxCommand::DetachSession);
        m.insert(self.toggle_zoom.clone(), MuxCommand::ToggleZoom);
        m.insert(self.scrollback_mode.clone(), MuxCommand::ScrollbackMode);

        // Window switching by number (hardcoded).
        for i in 0..=9u8 {
            m.insert(i.to_string(), MuxCommand::SwitchToWindow(i));
        }

        m
    }
}

/// Status bar appearance configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusBarConfig {
    /// Format string for left section.
    pub format_left: String,
    /// Format string for center section.
    pub format_center: String,
    /// Format string for right section.
    pub format_right: String,
    /// Foreground color (hex).
    pub fg: String,
    /// Background color (hex).
    pub bg: String,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            format_left: "[{session}]".into(),
            format_center: "{windows}".into(),
            format_right: "{time}".into(),
            fg: "#a0a0a0".into(),
            bg: "#1a1a1a".into(),
        }
    }
}

// --- SerdeReplace implementations for hot-reload (behind config-integration) ---

#[cfg(feature = "config-integration")]
mod serde_replace_impls {
    use std::error::Error;

    use alacritty_config::SerdeReplace;
    use toml::Value;

    use super::*;

    impl SerdeReplace for MultiplexerConfig {
        fn replace(&mut self, value: Value) -> Result<(), Box<dyn Error>> {
            match value.as_table() {
                Some(table) => {
                    for (field, next_value) in table {
                        let next_value = next_value.clone();
                        match field.as_str() {
                            "enabled" => self.enabled.replace(next_value)?,
                            "status_bar" => self.status_bar.replace(next_value)?,
                            "leader_keys" => self.leader_keys.replace(next_value)?,
                            "leader_timeout_ms" => self.leader_timeout_ms.replace(next_value)?,
                            "keybindings" => self.keybindings.replace(next_value)?,
                            "status_bar_config" => self.status_bar_config.replace(next_value)?,
                            _ => {
                                return Err(
                                    format!("Unknown multiplexer field: \"{field}\"").into()
                                );
                            },
                        }
                    }
                },
                None => *self = serde::Deserialize::deserialize(value)?,
            }
            Ok(())
        }
    }

    impl SerdeReplace for KeybindingsConfig {
        fn replace(&mut self, value: Value) -> Result<(), Box<dyn Error>> {
            match value.as_table() {
                Some(table) => {
                    for (field, next_value) in table {
                        let next_value = next_value.clone();
                        match field.as_str() {
                            "split_horizontal" => self.split_horizontal.replace(next_value)?,
                            "split_horizontal_alt" => {
                                self.split_horizontal_alt.replace(next_value)?
                            },
                            "split_vertical" => self.split_vertical.replace(next_value)?,
                            "split_vertical_alt" => self.split_vertical_alt.replace(next_value)?,
                            "close_pane" => self.close_pane.replace(next_value)?,
                            "next_pane" => self.next_pane.replace(next_value)?,
                            "prev_pane" => self.prev_pane.replace(next_value)?,
                            "new_window" => self.new_window.replace(next_value)?,
                            "next_window" => self.next_window.replace(next_value)?,
                            "prev_window" => self.prev_window.replace(next_value)?,
                            "detach" => self.detach.replace(next_value)?,
                            "rename_window" => self.rename_window.replace(next_value)?,
                            "toggle_zoom" => self.toggle_zoom.replace(next_value)?,
                            "scrollback_mode" => self.scrollback_mode.replace(next_value)?,
                            _ => {
                                return Err(format!("Unknown keybinding field: \"{field}\"").into());
                            },
                        }
                    }
                },
                None => *self = serde::Deserialize::deserialize(value)?,
            }
            Ok(())
        }
    }

    impl SerdeReplace for StatusBarConfig {
        fn replace(&mut self, value: Value) -> Result<(), Box<dyn Error>> {
            match value.as_table() {
                Some(table) => {
                    for (field, next_value) in table {
                        let next_value = next_value.clone();
                        match field.as_str() {
                            "format_left" => self.format_left.replace(next_value)?,
                            "format_center" => self.format_center.replace(next_value)?,
                            "format_right" => self.format_right.replace(next_value)?,
                            "fg" => self.fg.replace(next_value)?,
                            "bg" => self.bg.replace(next_value)?,
                            _ => {
                                return Err(format!("Unknown status_bar field: \"{field}\"").into());
                            },
                        }
                    }
                },
                None => *self = serde::Deserialize::deserialize(value)?,
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = MultiplexerConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.status_bar);
        assert_eq!(cfg.leader_keys.len(), 2);
        assert_eq!(cfg.leader_timeout_ms, 1000);
    }

    #[test]
    fn keybindings_to_map() {
        let cfg = KeybindingsConfig::default();
        let map = cfg.to_bindings_map();
        assert!(map.contains_key("\""));
        assert!(map.contains_key("%"));
        assert!(map.contains_key("x"));
        assert!(map.contains_key("c"));
        assert!(map.contains_key("0"));
        assert!(map.contains_key("9"));
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = MultiplexerConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: MultiplexerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.leader_keys, cfg.leader_keys);
        assert_eq!(restored.leader_timeout_ms, cfg.leader_timeout_ms);
    }

    #[test]
    fn config_partial_eq() {
        let a = MultiplexerConfig::default();
        let b = MultiplexerConfig::default();
        assert_eq!(a, b);
    }

    #[test]
    fn keybindings_partial_eq() {
        let a = KeybindingsConfig::default();
        let b = KeybindingsConfig::default();
        assert_eq!(a, b);
    }

    #[test]
    fn status_bar_config_partial_eq() {
        let a = StatusBarConfig::default();
        let b = StatusBarConfig::default();
        assert_eq!(a, b);
    }
}
