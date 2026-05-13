use std::collections::HashMap;

use crate::config::Config;

/// Named actions that can be bound to keys.
///
/// These names are the public API for keybinding configuration — they appear in
/// the TOML config file and in documentation. Renaming one is a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Advance to the next logical slide or overlay-aware build step.
    NextSlide,
    /// Move to the previous logical slide or overlay-aware build step.
    PreviousSlide,
    /// Advance by one raw PDF page.
    NextOverlay,
    /// Move back by one raw PDF page.
    PreviousOverlay,
    /// Jump to the first logical slide.
    FirstSlide,
    /// Jump to the last logical slide.
    LastSlide,
    /// Enter a numeric slide jump.
    GoToSlide,
    /// Freeze or unfreeze the audience display.
    ToggleFreeze,
    /// Black out or restore the audience display.
    ToggleBlackout,
    /// Show or hide the whiteboard canvas.
    ToggleWhiteboard,
    /// Enable or disable the laser pointer.
    ToggleLaser,
    /// Rotate through configured laser pointer styles.
    CycleLaserStyle,
    /// Enable or disable freehand ink mode.
    ToggleInk,
    /// Clear ink from the active slide or whiteboard.
    ClearInk,
    /// Rotate through configured ink colors.
    CycleInkColor,
    /// Rotate through configured ink widths.
    CycleInkWidth,
    /// Enable or disable the spotlight overlay.
    ToggleSpotlight,
    /// Enable or disable the zoom overlay.
    ToggleZoom,
    /// Show or hide the slide overview.
    ToggleOverview,
    /// Show or hide slide notes.
    ToggleNotes,
    /// Toggle markdown editing for slide notes.
    ToggleNotesEdit,
    /// Start, pause, or resume the main timer.
    StartPauseTimer,
    /// Reset the main timer.
    ResetTimer,
    /// Increase the notes font size.
    IncrementNotesFont,
    /// Decrease the notes font size.
    DecrementNotesFont,
    /// Toggle screen-share window mode.
    ToggleScreenShare,
    /// Toggle the single-monitor presentation surface.
    TogglePresentationMode,
    /// Enable or disable text box placement mode.
    ToggleTextBoxMode,
    /// Request application shutdown.
    Quit,
    /// Save the active sidecar file.
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
            Self::ToggleWhiteboard => "toggle_whiteboard",
            Self::ToggleLaser => "toggle_laser",
            Self::CycleLaserStyle => "cycle_laser_style",
            Self::ToggleInk => "toggle_ink",
            Self::ClearInk => "clear_ink",
            Self::CycleInkColor => "cycle_ink_color",
            Self::CycleInkWidth => "cycle_ink_width",
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
            Self::ToggleTextBoxMode => "toggle_text_box_mode",
            Self::Quit => "quit",
            Self::SaveSidecar => "save_sidecar",
        }
    }

    /// Human-readable description for the help overlay.
    pub fn description(self) -> &'static str {
        match self {
            Self::NextSlide => "Next slide/build step",
            Self::PreviousSlide => "Previous slide/build step",
            Self::NextOverlay => "Next overlay step",
            Self::PreviousOverlay => "Previous overlay step",
            Self::FirstSlide => "First slide",
            Self::LastSlide => "Last slide",
            Self::GoToSlide => "Go to slide…",
            Self::ToggleFreeze => "Freeze audience",
            Self::ToggleBlackout => "Black out audience",
            Self::ToggleWhiteboard => "Whiteboard",
            Self::ToggleLaser => "Laser pointer",
            Self::CycleLaserStyle => "Cycle laser style",
            Self::ToggleInk => "Drawing mode",
            Self::ClearInk => "Clear ink",
            Self::CycleInkColor => "Cycle pen color",
            Self::CycleInkWidth => "Cycle pen width",
            Self::ToggleSpotlight => "Spotlight",
            Self::ToggleZoom => "Zoom mode",
            Self::ToggleOverview => "Slide overview",
            Self::ToggleNotes => "Toggle notes",
            Self::ToggleNotesEdit => "Edit notes",
            Self::StartPauseTimer => "Start / pause timer",
            Self::ResetTimer => "Reset timer",
            Self::IncrementNotesFont => "Increase notes font",
            Self::DecrementNotesFont => "Decrease notes font",
            Self::ToggleScreenShare => "Screen-share mode",
            Self::TogglePresentationMode => "Presentation mode",
            Self::ToggleTextBoxMode => "Text box mode",
            Self::Quit => "Quit",
            Self::SaveSidecar => "Save sidecar",
        }
    }

    /// Grouping label for the help overlay.
    pub fn group(self) -> &'static str {
        match self {
            Self::NextSlide
            | Self::PreviousSlide
            | Self::NextOverlay
            | Self::PreviousOverlay
            | Self::FirstSlide
            | Self::LastSlide
            | Self::GoToSlide => "Navigation",

            Self::ToggleFreeze
            | Self::ToggleBlackout
            | Self::ToggleWhiteboard
            | Self::ToggleScreenShare
            | Self::TogglePresentationMode => "Display",

            Self::ToggleLaser
            | Self::CycleLaserStyle
            | Self::ToggleInk
            | Self::ClearInk
            | Self::CycleInkColor
            | Self::CycleInkWidth
            | Self::ToggleSpotlight
            | Self::ToggleZoom
            | Self::ToggleTextBoxMode => "Presenter Tools",

            Self::StartPauseTimer | Self::ResetTimer => "Timer",

            Self::ToggleOverview
            | Self::ToggleNotes
            | Self::ToggleNotesEdit
            | Self::IncrementNotesFont
            | Self::DecrementNotesFont => "Notes & Panels",

            Self::Quit | Self::SaveSidecar => "System",
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
            Self::ToggleWhiteboard,
            Self::ToggleLaser,
            Self::CycleLaserStyle,
            Self::ToggleInk,
            Self::ClearInk,
            Self::CycleInkColor,
            Self::CycleInkWidth,
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
            Self::ToggleTextBoxMode,
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

        key.map(|key| Self { key: normalize_key_name(&key), shift, ctrl, alt })
    }

    /// Human-readable display string (e.g., "Ctrl+Shift+S").
    pub fn display_name(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        // Capitalise single-character keys for display.
        let key_display: String =
            if self.key.len() == 1 { self.key.to_uppercase() } else { self.key.clone() };
        parts.push(&key_display);
        parts.join("+")
    }
}

fn normalize_key_name(key: &str) -> String {
    if key.chars().count() == 1 { key.to_lowercase() } else { key.to_string() }
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

    /// Return the active bindings grouped by action with human-readable key names.
    ///
    /// Each action appears in `Action::all()` order. The associated `Vec` contains
    /// the display strings for every key combo currently mapped to that action.
    pub fn action_bindings(&self) -> Vec<(Action, Vec<String>)> {
        Action::all()
            .iter()
            .map(|&action| {
                let mut keys: Vec<String> = self
                    .bindings
                    .iter()
                    .filter(|&(_, &a)| a == action)
                    .map(|(combo, _)| combo.display_name())
                    .collect();
                keys.sort();
                (action, keys)
            })
            .collect()
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
        (Action::ToggleWhiteboard, vec!["w".into()]),
        (Action::ToggleLaser, vec!["l".into()]),
        (Action::CycleLaserStyle, vec!["Ctrl+l".into()]),
        (Action::ToggleInk, vec!["d".into()]),
        (Action::ClearInk, vec!["c".into()]),
        (Action::CycleInkColor, vec!["Ctrl+d".into()]),
        (Action::CycleInkWidth, vec!["Shift+d".into()]),
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
        (Action::ToggleTextBoxMode, vec!["x".into()]),
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
    fn parse_single_character_keys_case_insensitively() {
        let lower = KeyCombo::parse("Ctrl+l").unwrap();
        let upper = KeyCombo::parse("Ctrl+L").unwrap();

        assert_eq!(lower, upper);
        assert_eq!(upper.key, "l");
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

    #[test]
    fn display_name_simple_key() {
        let combo = KeyCombo::parse("Right").unwrap();
        assert_eq!(combo.display_name(), "Right");
    }

    #[test]
    fn display_name_modifier_combo() {
        let combo = KeyCombo::parse("Ctrl+Shift+s").unwrap();
        assert_eq!(combo.display_name(), "Ctrl+Shift+S");
    }

    #[test]
    fn display_name_single_char_uppercase() {
        let combo = KeyCombo::parse("g").unwrap();
        assert_eq!(combo.display_name(), "G");
    }

    #[test]
    fn action_bindings_returns_all_actions() {
        let map = KeybindingMap::from_config(&HashMap::new());
        let bindings = map.action_bindings();
        assert_eq!(bindings.len(), Action::all().len());
    }

    #[test]
    fn cycle_laser_style_accepts_uppercase_config_binding() {
        let mut user = HashMap::new();
        user.insert("cycle_laser_style".to_string(), vec!["Ctrl+L".to_string()]);
        let map = KeybindingMap::from_config(&user);

        let live_key = KeyCombo::parse("Ctrl+l").unwrap();
        assert_eq!(map.lookup(&live_key), Some(Action::CycleLaserStyle));
    }

    #[test]
    fn action_bindings_reflects_overrides() {
        let mut user = HashMap::new();
        user.insert("quit".to_string(), vec!["x".to_string()]);
        let map = KeybindingMap::from_config(&user);
        let bindings = map.action_bindings();

        let quit_keys: Vec<String> =
            bindings.iter().find(|(a, _)| *a == Action::Quit).unwrap().1.clone();
        assert_eq!(quit_keys, vec!["X"]);
    }

    #[test]
    fn every_action_has_description_and_group() {
        for action in Action::all() {
            assert!(!action.description().is_empty(), "{action:?} missing description");
            assert!(!action.group().is_empty(), "{action:?} missing group");
        }
    }
}
