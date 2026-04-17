use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::TimerMode;

/// Top-level application configuration, loaded from TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub display: DisplayConfig,
    pub timer: TimerConfig,
    pub pointer: PointerConfig,
    pub spotlight: SpotlightConfig,
    pub ink: InkConfig,
    pub notes: NotesConfig,
    pub keybindings: HashMap<String, Vec<String>>,
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

/// Timer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimerConfig {
    /// "countdown" or "elapsed".
    pub mode: TimerMode,
    /// Timer duration in minutes.
    pub duration_minutes: u32,
    /// Minutes remaining when warning color activates.
    pub warning_minutes: u32,
    /// Whether to show red when past duration.
    pub overrun_color: bool,
}

/// Laser/mouse pointer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PointerConfig {
    /// Hex color string (e.g., "#FF0000").
    pub color: String,
    /// Size in logical pixels at 1x scale.
    pub size: f32,
    /// Style: "dot", "crosshair", or "arrow".
    pub style: String,
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

/// Ink drawing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InkConfig {
    /// Hex color string.
    pub color: String,
    /// Stroke width in logical pixels.
    pub width: f32,
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
            mode: TimerMode::Countdown,
            duration_minutes: 20,
            warning_minutes: 5,
            overrun_color: true,
        }
    }
}

impl Default for PointerConfig {
    fn default() -> Self {
        Self { color: "#FF0000".to_string(), size: 12.0, style: "dot".to_string() }
    }
}

impl Default for SpotlightConfig {
    fn default() -> Self {
        Self { radius: 150.0, dim_opacity: 0.6 }
    }
}

impl Default for InkConfig {
    fn default() -> Self {
        Self { color: "#FF0000".to_string(), width: 3.0 }
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

/// Load config from the default path, returning defaults if the file doesn't exist.
pub fn load_config() -> Config {
    let Some(path) = config_path() else {
        tracing::warn!("Could not determine config directory, using defaults");
        return Config::default();
    };

    let Ok(contents) = std::fs::read_to_string(&path) else {
        tracing::info!("No config file at {}, using defaults", path.display());
        return Config::default();
    };

    match toml::from_str(&contents) {
        Ok(config) => {
            tracing::info!("Loaded config from {}", path.display());
            config
        }
        Err(e) => {
            tracing::warn!("Failed to parse config at {}: {e}, using defaults", path.display());
            Config::default()
        }
    }
}
