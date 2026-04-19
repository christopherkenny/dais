use std::collections::HashMap;

use crate::config::Config;

/// Named actions that can be bound to keys.
///
/// These names are the public API for keybinding configuration — they appear in
/// the TOML config file and in documentation. Renaming one is a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    NextSlide,
    PreviousSlide,
    NextOverlay,
    PreviousOverlay,
    FirstSlide,
    LastSlide,
    GoToSlide,
    ToggleFreeze,
    ToggleBlackout,
    ToggleLaser,
    CycleLaserStyle,
    ToggleInk,
    ClearInk,
    ToggleSpotlight,
    ToggleZoom,
    ToggleOverview,
    ToggleNotes,
    ToggleNotesEdit,
    StartPauseTimer,
    ResetTimer,
    IncrementNotesFont,
    DecrementNotesFont,
    ToggleScreenShare,
    TogglePresentationMode,
    Quit,
    SaveSidecar,
}

impl Action {
    /// The config-file name for this action (e.g., `next_slide`).
    pub fn config_name(self) -> &'static str {
        match self {
            Self::NextSlide => "next_slide",
            Self::PreviousSlide => "previous_slide",
            Self::NextOverlay => "next_overlay",
            Self::PreviousOverlay => "previous_overlay",
            Self::FirstSlide => "first_slide",
            Self::LastSlide => "last_slide",
            Self::GoToSlide => "go_to_slide",
            Self::ToggleFreeze => "toggle_freeze",
            Self::ToggleBlackout => "toggle_blackout",
            Self::ToggleLaser => "toggle_laser",
            Self::CycleLaserStyle => "cycle_laser_style",
            Self::ToggleInk => "toggle_ink",
            Self::ClearInk => "clear_ink",
            Self::ToggleSpotlight => "toggle_spotlight",
            Self::ToggleZoom => "toggle_zoom",
            Self::ToggleOverview => "toggle_overview",
            Self::ToggleNotes => "toggle_notes",
            Self::ToggleNotesEdit => "toggle_notes_edit",
            Self::StartPauseTimer => "start_pause_timer",
            Self::ResetTimer => "reset_timer",
            Self::IncrementNotesFont => "increment_notes_font",
            Self::DecrementNotesFont => "decrement_notes_font",
            Self::ToggleScreenShare => "toggle_screen_share",
            Self::TogglePresentationMode => "toggle_presentation_mode",
            Self::Quit => "quit",
            Self::SaveSidecar => "save_sidecar",
        }
    }

    /// All known actions.
    pub fn all() -> &'static [Action] {
        &[
            Self::NextSlide,
            Self::PreviousSlide,
            Self::NextOverlay,
            Self::PreviousOverlay,
            Self::FirstSlide,
            Self::LastSlide,
            Self::GoToSlide,
            Self::ToggleFreeze,
            Self::ToggleBlackout,
            Self::ToggleLaser,
            Self::CycleLaserStyle,
            Self::ToggleInk,
            Self::ClearInk,
            Self::ToggleSpotlight,
            Self::ToggleZoom,
            Self::ToggleOverview,
            Self::ToggleNotes,
            Self::ToggleNotesEdit,
            Self::StartPauseTimer,
            Self::ResetTimer,
            Self::IncrementNotesFont,
            Self::DecrementNotesFont,
            Self::ToggleScreenShare,
            Self::TogglePresentationMode,
            Self::Quit,
            Self::SaveSidecar,
        ]
    }
}

/// A key combination (key name + modifiers).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    /// The key name (e.g., "Right", "Space", "a", "F1").
    pub key: String,
    /// Whether Shift is held.
    pub shift: bool,
    /// Whether Ctrl (or Cmd on macOS) is held.
    pub ctrl: bool,
    /// Whether Alt is held.
    pub alt: bool,
}

impl KeyCombo {
    /// Parse a key combo string like "Shift+Right" or "Ctrl+s".
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        let mut shift = false;
        let mut ctrl = false;
        let mut alt = false;
        let mut key = None;

        for part in &parts {
            let normalized = part.trim();
            match normalized.to_lowercase().as_str() {
                "shift" => shift = true,
                "ctrl" | "control" | "cmd" | "command" => ctrl = true,
                "alt" | "option" => alt = true,
                _ => key = Some(normalized.to_string()),
            }
        }

        key.map(|key| Self { key, shift, ctrl, alt })
    }
}

/// Maps key combinations to actions.
pub struct KeybindingMap {
    bindings: HashMap<KeyCombo, Action>,
}

impl KeybindingMap {
    /// Build a keybinding map from user config overlaid on defaults.
    pub fn from_config(user_bindings: &HashMap<String, Vec<String>>) -> Self {
        let mut bindings = HashMap::new();

        // Start with defaults
        for (action, keys) in default_keybindings() {
            for key_str in keys {
                if let Some(combo) = KeyCombo::parse(&key_str) {
                    bindings.insert(combo, action);
                }
            }
        }

        // Overlay user config — if a user defines any binding for an action,
        // it replaces all defaults for that action.
        let action_lookup: HashMap<&str, Action> =
            Action::all().iter().map(|a| (a.config_name(), *a)).collect();

        for (action_name, keys) in user_bindings {
            if let Some(&action) = action_lookup.get(action_name.as_str()) {
                // Remove all existing bindings for this action
                bindings.retain(|_, v| *v != action);
                // Add user-defined bindings
                for key_str in keys {
                    if let Some(combo) = KeyCombo::parse(key_str) {
                        bindings.insert(combo, action);
                    }
                }
            } else {
                tracing::warn!("Unknown action in keybinding config: {action_name}");
            }
        }

        Self { bindings }
    }

    /// Build a keybinding map from config plus the active clicker profile.
    pub fn from_full_config(config: &Config) -> Self {
        let mut map = Self::from_config(&config.keybindings);
        map.apply_clicker_profile(&config.active_clicker_profile());
        map
    }

    /// Look up the action bound to a key combination.
    pub fn lookup(&self, combo: &KeyCombo) -> Option<Action> {
        self.bindings.get(combo).copied()
    }

    fn apply_clicker_profile(&mut self, clicker_bindings: &HashMap<String, String>) {
        let action_lookup: HashMap<&str, Action> =
            Action::all().iter().map(|a| (a.config_name(), *a)).collect();

        for (key_name, action_name) in clicker_bindings {
            let Some(&action) = action_lookup.get(action_name.as_str()) else {
                tracing::warn!("Unknown action in clicker profile: {action_name}");
                continue;
            };

            if let Some(combo) = KeyCombo::parse(key_name) {
                self.bindings.insert(combo, action);
            } else {
                tracing::warn!("Invalid key in clicker profile: {key_name}");
            }
        }
    }
}

/// Default keybindings (pdfpc-compatible where applicable).
fn default_keybindings() -> Vec<(Action, Vec<String>)> {
    vec![
        (Action::NextSlide, vec!["Right".into(), "Space".into(), "Down".into(), "PageDown".into()]),
        (Action::PreviousSlide, vec!["Left".into(), "Up".into(), "PageUp".into()]),
        (Action::NextOverlay, vec!["Shift+Right".into(), "Shift+Down".into()]),
        (Action::PreviousOverlay, vec!["Shift+Left".into(), "Shift+Up".into()]),
        (Action::FirstSlide, vec!["Home".into()]),
        (Action::LastSlide, vec!["End".into()]),
        (Action::GoToSlide, vec!["g".into()]),
        (Action::ToggleFreeze, vec!["f".into()]),
        (Action::ToggleBlackout, vec!["b".into(), ".".into()]),
        (Action::ToggleLaser, vec!["l".into()]),
        (Action::CycleLaserStyle, vec!["Ctrl+l".into()]),
        (Action::ToggleInk, vec!["d".into()]),
        (Action::ClearInk, vec!["c".into()]),
        (Action::ToggleSpotlight, vec!["s".into()]),
        (Action::ToggleZoom, vec!["z".into()]),
        (Action::ToggleOverview, vec!["o".into()]),
        (Action::ToggleNotes, vec!["n".into()]),
        (Action::ToggleNotesEdit, vec!["Ctrl+n".into()]),
        (Action::StartPauseTimer, vec!["t".into()]),
        (Action::ResetTimer, vec!["Shift+t".into()]),
        (Action::IncrementNotesFont, vec!["+".into(), "Shift+=".into()]),
        (Action::DecrementNotesFont, vec!["-".into()]),
        (Action::ToggleScreenShare, vec!["Shift+s".into()]),
        (Action::TogglePresentationMode, vec!["F5".into()]),
        (Action::Quit, vec!["q".into(), "Escape".into()]),
        (Action::SaveSidecar, vec!["Ctrl+s".into()]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_key() {
        let combo = KeyCombo::parse("Right").unwrap();
        assert_eq!(combo.key, "Right");
        assert!(!combo.shift);
        assert!(!combo.ctrl);
    }

    #[test]
    fn parse_shift_combo() {
        let combo = KeyCombo::parse("Shift+Right").unwrap();
        assert_eq!(combo.key, "Right");
        assert!(combo.shift);
        assert!(!combo.ctrl);
    }

    #[test]
    fn parse_ctrl_combo() {
        let combo = KeyCombo::parse("Ctrl+s").unwrap();
        assert_eq!(combo.key, "s");
        assert!(!combo.shift);
        assert!(combo.ctrl);
    }

    #[test]
    fn default_bindings_load() {
        let map = KeybindingMap::from_config(&HashMap::new());
        let right = KeyCombo::parse("Right").unwrap();
        assert_eq!(map.lookup(&right), Some(Action::NextSlide));
    }

    #[test]
    fn user_override_replaces_defaults() {
        let mut user = HashMap::new();
        user.insert("next_slide".to_string(), vec!["x".to_string()]);
        let map = KeybindingMap::from_config(&user);

        // "x" should now be next_slide
        let x = KeyCombo::parse("x").unwrap();
        assert_eq!(map.lookup(&x), Some(Action::NextSlide));

        // Default "Right" should no longer be bound to next_slide
        let right = KeyCombo::parse("Right").unwrap();
        assert_ne!(map.lookup(&right), Some(Action::NextSlide));
    }

    #[test]
    fn clicker_profile_overlays_bindings() {
        let mut config = Config::default();
        config.clicker.profile = "custom".to_string();
        config.clicker.profiles.insert(
            "custom".to_string(),
            HashMap::from([("Escape".to_string(), "toggle_blackout".to_string())]),
        );

        let map = KeybindingMap::from_full_config(&config);
        let escape = KeyCombo::parse("Escape").unwrap();
        assert_eq!(map.lookup(&escape), Some(Action::ToggleBlackout));
    }
}
