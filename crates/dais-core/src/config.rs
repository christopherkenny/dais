use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::state::TimerMode;

/// Top-level application configuration, loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub display: DisplayConfig,
    pub timer: TimerConfig,
    pub laser: LaserConfig,
    pub spotlight: SpotlightConfig,
    pub ink: InkConfig,
    pub notes: NotesConfig,
    pub keybindings: HashMap<String, Vec<String>>,
    pub clicker: ClickerConfig,
    /// Sidecar save format: `"dais"` or `"pdfpc"`.
    pub sidecar_format: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialConfig {
    display: Option<PartialDisplayConfig>,
    timer: Option<PartialTimerConfig>,
    laser: Option<PartialLaserConfig>,
    spotlight: Option<PartialSpotlightConfig>,
    ink: Option<PartialInkConfig>,
    notes: Option<PartialNotesConfig>,
    keybindings: Option<HashMap<String, Vec<String>>>,
    clicker: Option<PartialClickerConfig>,
    sidecar_format: Option<String>,
}

/// Display mode and monitor assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Display mode: "dual", "single", or "screen-share".
    pub mode: String,
    /// Audience monitor identifier or "auto".
    pub audience_monitor: String,
    /// Presenter monitor identifier or "auto".
    pub presenter_monitor: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialDisplayConfig {
    mode: Option<String>,
    audience_monitor: Option<String>,
    presenter_monitor: Option<String>,
}

/// Timer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimerConfig {
    /// "countdown" or "elapsed".
    pub mode: TimerMode,
    /// Timer duration in minutes. If omitted in elapsed mode, no limit is shown.
    pub duration_minutes: Option<u32>,
    /// Minutes remaining when warning color activates.
    pub warning_minutes: Option<u32>,
    /// Whether to show red when past duration.
    pub overrun_color: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialTimerConfig {
    mode: Option<TimerMode>,
    duration_minutes: Option<OptionalU32Value>,
    warning_minutes: Option<OptionalU32Value>,
    overrun_color: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
enum OptionalU32Value {
    Value(u32),
    Null(()),
}

impl OptionalU32Value {
    fn into_option(self) -> Option<u32> {
        match self {
            Self::Value(value) => Some(value),
            Self::Null(()) => None,
        }
    }
}

/// Laser pointer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LaserConfig {
    /// Hex color string (e.g., "#FF0000").
    pub color: String,
    /// Size in logical pixels at 1x scale.
    pub size: f32,
    /// Style: "dot", "crosshair", or "arrow".
    pub style: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialLaserConfig {
    color: Option<String>,
    size: Option<f32>,
    style: Option<String>,
}

/// Spotlight configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpotlightConfig {
    /// Radius in logical pixels at 1x scale.
    pub radius: f32,
    /// Opacity of the dimmed area (0.0–1.0).
    pub dim_opacity: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialSpotlightConfig {
    radius: Option<f32>,
    dim_opacity: Option<f32>,
}

/// Ink drawing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InkConfig {
    /// Hex color string.
    pub color: String,
    /// Stroke width in logical pixels.
    pub width: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialInkConfig {
    color: Option<String>,
    width: Option<f32>,
}

/// Clicker/remote hardware configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClickerConfig {
    /// Name of the active clicker profile (e.g., "default", "logitech-spotlight").
    pub profile: String,
    /// Custom profile definitions mapping key names to action names.
    pub profiles: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialClickerConfig {
    profile: Option<String>,
    profiles: Option<HashMap<String, HashMap<String, String>>>,
}

/// Notes panel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotesConfig {
    /// Font size in points.
    pub font_size: f32,
    /// Step size for font size increment/decrement.
    pub font_size_step: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PartialNotesConfig {
    font_size: Option<f32>,
    font_size_step: Option<f32>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            display: DisplayConfig::default(),
            timer: TimerConfig::default(),
            laser: LaserConfig::default(),
            spotlight: SpotlightConfig::default(),
            ink: InkConfig::default(),
            notes: NotesConfig::default(),
            keybindings: HashMap::new(),
            clicker: ClickerConfig::default(),
            sidecar_format: "dais".to_string(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            mode: "dual".to_string(),
            audience_monitor: "auto".to_string(),
            presenter_monitor: "auto".to_string(),
        }
    }
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            mode: TimerMode::Elapsed,
            duration_minutes: None,
            warning_minutes: None,
            overrun_color: true,
        }
    }
}

impl Default for LaserConfig {
    fn default() -> Self {
        Self { color: "#FF0000".to_string(), size: 12.0, style: "dot".to_string() }
    }
}

impl Default for SpotlightConfig {
    fn default() -> Self {
        Self { radius: 80.0, dim_opacity: 0.6 }
    }
}

impl Default for InkConfig {
    fn default() -> Self {
        Self { color: "#FF0000".to_string(), width: 3.0 }
    }
}

impl Default for ClickerConfig {
    fn default() -> Self {
        Self { profile: "default".to_string(), profiles: HashMap::new() }
    }
}

/// Return the built-in default clicker profile mapping common USB presenter keys to actions.
pub fn default_clicker_profile() -> HashMap<String, String> {
    HashMap::from([
        ("PageDown".to_string(), "next_slide".to_string()),
        ("PageUp".to_string(), "previous_slide".to_string()),
        ("F5".to_string(), "toggle_presentation_mode".to_string()),
        ("b".to_string(), "toggle_blackout".to_string()),
        (".".to_string(), "toggle_blackout".to_string()),
    ])
}

impl Config {
    /// Resolve the active clicker profile into a key -> action map.
    pub fn active_clicker_profile(&self) -> HashMap<String, String> {
        if self.clicker.profile == "default" {
            return default_clicker_profile();
        }

        self.clicker.profiles.get(&self.clicker.profile).cloned().unwrap_or_else(|| {
            tracing::warn!(
                "Configured clicker profile '{}' not found; using default profile",
                self.clicker.profile
            );
            default_clicker_profile()
        })
    }

    /// Normalize the configured sidecar save format to a supported value.
    pub fn normalized_sidecar_format(&self) -> &str {
        if self.sidecar_format.eq_ignore_ascii_case("dais") { "dais" } else { "pdfpc" }
    }
}

impl Default for NotesConfig {
    fn default() -> Self {
        Self { font_size: 16.0, font_size_step: 2.0 }
    }
}

/// Resolve the platform-appropriate config file path.
pub fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "dais").map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Resolve a project-local config path for a PDF.
pub fn project_config_path(pdf_path: &Path) -> Option<PathBuf> {
    pdf_path.parent().map(|dir| dir.join("dais.toml"))
}

/// Load layered config for a document.
///
/// Precedence:
/// 1. Built-in defaults
/// 2. Machine-wide config (`config.toml` in the standard OS config dir)
/// 3. Project-local config (`dais.toml` next to the PDF)
/// 4. Explicit `--config` path, if provided
pub fn load_config_for(pdf_path: &Path, explicit_config: Option<&Path>) -> Config {
    let mut config = Config::default();

    if let Some(path) = config_path() {
        merge_config_file(&mut config, &path);
    } else {
        tracing::warn!("Could not determine config directory, using defaults");
    }

    if let Some(path) = project_config_path(pdf_path) {
        merge_config_file(&mut config, &path);
    }

    if let Some(path) = explicit_config {
        merge_config_file(&mut config, path);
    }

    config
}

fn merge_config_file(config: &mut Config, path: &Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        tracing::debug!("No config file at {}", path.display());
        return;
    };

    match toml::from_str::<PartialConfig>(&contents) {
        Ok(partial) => {
            tracing::info!("Loaded config layer from {}", path.display());
            apply_partial_config(config, partial);
        }
        Err(e) => {
            tracing::warn!("Failed to parse config at {}: {e}", path.display());
        }
    }
}

fn apply_partial_config(config: &mut Config, partial: PartialConfig) {
    if let Some(display) = partial.display {
        if let Some(mode) = display.mode {
            config.display.mode = mode;
        }
        if let Some(audience_monitor) = display.audience_monitor {
            config.display.audience_monitor = audience_monitor;
        }
        if let Some(presenter_monitor) = display.presenter_monitor {
            config.display.presenter_monitor = presenter_monitor;
        }
    }

    if let Some(timer) = partial.timer {
        if let Some(mode) = timer.mode {
            config.timer.mode = mode;
        }
        if let Some(duration_minutes) = timer.duration_minutes {
            config.timer.duration_minutes = duration_minutes.into_option();
        }
        if let Some(warning_minutes) = timer.warning_minutes {
            config.timer.warning_minutes = warning_minutes.into_option();
        }
        if let Some(overrun_color) = timer.overrun_color {
            config.timer.overrun_color = overrun_color;
        }
    }

    if let Some(laser) = partial.laser {
        if let Some(color) = laser.color {
            config.laser.color = color;
        }
        if let Some(size) = laser.size {
            config.laser.size = size;
        }
        if let Some(style) = laser.style {
            config.laser.style = style;
        }
    }

    if let Some(spotlight) = partial.spotlight {
        if let Some(radius) = spotlight.radius {
            config.spotlight.radius = radius;
        }
        if let Some(dim_opacity) = spotlight.dim_opacity {
            config.spotlight.dim_opacity = dim_opacity;
        }
    }

    if let Some(ink) = partial.ink {
        if let Some(color) = ink.color {
            config.ink.color = color;
        }
        if let Some(width) = ink.width {
            config.ink.width = width;
        }
    }

    if let Some(notes) = partial.notes {
        if let Some(font_size) = notes.font_size {
            config.notes.font_size = font_size;
        }
        if let Some(font_size_step) = notes.font_size_step {
            config.notes.font_size_step = font_size_step;
        }
    }

    if let Some(keybindings) = partial.keybindings {
        config.keybindings.extend(keybindings);
    }

    if let Some(clicker) = partial.clicker {
        if let Some(profile) = clicker.profile {
            config.clicker.profile = profile;
        }
        if let Some(profiles) = clicker.profiles {
            config.clicker.profiles.extend(profiles);
        }
    }

    if let Some(sidecar_format) = partial.sidecar_format {
        config.sidecar_format = sidecar_format;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_overrides_selected_fields() {
        let mut config = Config::default();
        let partial = PartialConfig {
            display: Some(PartialDisplayConfig {
                mode: Some("screen-share".to_string()),
                audience_monitor: Some("Projector".to_string()),
                presenter_monitor: None,
            }),
            timer: Some(PartialTimerConfig {
                mode: Some(TimerMode::Countdown),
                duration_minutes: Some(OptionalU32Value::Value(45)),
                warning_minutes: Some(OptionalU32Value::Value(10)),
                overrun_color: Some(false),
            }),
            ..Default::default()
        };

        apply_partial_config(&mut config, partial);

        assert_eq!(config.display.mode, "screen-share");
        assert_eq!(config.display.audience_monitor, "Projector");
        assert_eq!(config.timer.mode, TimerMode::Countdown);
        assert_eq!(config.timer.duration_minutes, Some(45));
        assert_eq!(config.timer.warning_minutes, Some(10));
        assert!(!config.timer.overrun_color);
    }

    #[test]
    fn partial_config_can_clear_optional_timer_values() {
        let mut config = Config::default();
        config.timer.duration_minutes = Some(20);
        config.timer.warning_minutes = Some(5);

        let partial = PartialConfig {
            timer: Some(PartialTimerConfig {
                duration_minutes: Some(OptionalU32Value::Null(())),
                warning_minutes: Some(OptionalU32Value::Null(())),
                ..Default::default()
            }),
            ..Default::default()
        };

        apply_partial_config(&mut config, partial);

        assert_eq!(config.timer.duration_minutes, None);
        assert_eq!(config.timer.warning_minutes, None);
    }
}
