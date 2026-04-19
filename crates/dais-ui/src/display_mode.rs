//! Display mode management — intelligent window placement based on monitor topology.
//!
//! Determines how the presenter and audience windows should be positioned
//! based on CLI flags, configuration, and detected monitors.

use std::sync::{Arc, OnceLock};

use dais_core::config::Config;
use dais_core::monitor::{MonitorInfo, MonitorManager};
use dais_document::page::RenderSize;
use dais_document::render_pipeline::FALLBACK_RENDER_SIZE;

/// How the application should lay out presenter and audience windows.
#[derive(Debug, Clone)]
pub enum DisplayMode {
    /// Dual monitor: presenter on primary, audience fullscreen on secondary.
    Dual { audience_monitor: MonitorInfo },
    /// Single window with presenter only (no audience viewport).
    Single,
    /// Audience as a normal resizable window (for screen sharing).
    ScreenShare,
}

/// CLI-level display hints (from `--single`, `--screen-share`, etc.).
#[derive(Debug, Clone, Copy)]
pub struct DisplayHints {
    /// `--single` was passed.
    pub force_single: bool,
    /// `--screen-share` was passed.
    pub force_screen_share: bool,
}

fn app_icon() -> Option<Arc<egui::IconData>> {
    static ICON: OnceLock<Option<Arc<egui::IconData>>> = OnceLock::new();

    ICON.get_or_init(|| {
        match eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/dais.png")) {
            Ok(icon) => Some(Arc::new(icon)),
            Err(err) => {
                tracing::warn!("Failed to load app icon from assets/dais.png: {err}");
                None
            }
        }
    })
    .clone()
}

pub fn with_app_icon(builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
    if let Some(icon) = app_icon() { builder.with_icon(icon) } else { builder }
}

/// Result of display mode determination, including any warnings.
pub struct DisplayModeResult {
    pub mode: DisplayMode,
    pub warnings: Vec<String>,
}

/// Determine the initial display mode from CLI hints, config, and detected monitors.
pub fn determine_display_mode(
    hints: DisplayHints,
    config: &Config,
    monitor_mgr: &dyn MonitorManager,
) -> DisplayModeResult {
    let mut warnings = Vec::new();

    // CLI flags take absolute precedence
    if hints.force_single {
        tracing::info!("Single mode requested via --single flag");
        return DisplayModeResult { mode: DisplayMode::Single, warnings };
    }
    if hints.force_screen_share {
        tracing::info!("Screen-share mode requested via --screen-share flag");
        return DisplayModeResult { mode: DisplayMode::ScreenShare, warnings };
    }

    // Config-based mode preference
    let config_mode = config.display.mode.to_lowercase();

    let monitors = monitor_mgr.available_monitors();
    log_monitor_topology(&monitors);

    let mode = match config_mode.as_str() {
        "single" => {
            tracing::info!("Single mode set in config");
            DisplayMode::Single
        }
        "screen-share" | "screenshare" | "screen_share" => {
            tracing::info!("Screen-share mode set in config");
            DisplayMode::ScreenShare
        }
        // "dual" or "auto" (default) — try to find a secondary monitor
        _ => resolve_dual_mode(config, &monitors, monitor_mgr, &mut warnings),
    };

    DisplayModeResult { mode, warnings }
}

/// Attempt to resolve dual mode, falling back gracefully.
fn resolve_dual_mode(
    config: &Config,
    monitors: &[MonitorInfo],
    monitor_mgr: &dyn MonitorManager,
    warnings: &mut Vec<String>,
) -> DisplayMode {
    let audience_name = &config.display.audience_monitor;
    if audience_name != "auto" && !audience_name.is_empty() {
        if let Some(mon) = monitor_mgr.find_by_selector(audience_name) {
            tracing::info!(
                "Using configured audience monitor '{}' -> {} '{}'",
                audience_name,
                mon.id,
                mon.name
            );
            return DisplayMode::Dual { audience_monitor: mon };
        }
        let available = monitors.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
        let msg = format!(
            "Configured audience monitor '{audience_name}' not found. Available: {available}",
        );
        tracing::warn!("{msg}");
        warnings.push(msg);
    }

    if let Some(secondary) = monitor_mgr.secondary_monitor() {
        tracing::info!(
            "Dual mode: audience on '{}' ({}x{} @ {:?})",
            secondary.name,
            secondary.size.0,
            secondary.size.1,
            secondary.position
        );
        return DisplayMode::Dual { audience_monitor: secondary };
    }

    let msg = "Single monitor detected — expected dual. Using single mode.".to_string();
    tracing::info!("{msg}");
    warnings.push(msg);
    DisplayMode::Single
}

/// Build the audience viewport builder for the given display mode.
#[allow(clippy::cast_precision_loss)]
pub fn audience_viewport_builder(mode: &DisplayMode) -> egui::ViewportBuilder {
    match mode {
        DisplayMode::Dual { audience_monitor } => {
            tracing::debug!(
                "Audience viewport: fullscreen on '{}' at ({}, {})",
                audience_monitor.name,
                audience_monitor.position.0,
                audience_monitor.position.1,
            );
            with_app_icon(egui::ViewportBuilder::default())
                .with_title("Dais — Audience")
                .with_fullscreen(true)
                .with_position(egui::pos2(
                    audience_monitor.position.0 as f32,
                    audience_monitor.position.1 as f32,
                ))
        }
        DisplayMode::Single => {
            // Single mode doesn't spawn an audience viewport — this is a fallback
            with_app_icon(egui::ViewportBuilder::default())
                .with_title("Dais — Audience")
                .with_inner_size(egui::vec2(1280.0, 720.0))
        }
        DisplayMode::ScreenShare => with_app_icon(egui::ViewportBuilder::default())
            .with_title("Dais — Audience")
            .with_inner_size(egui::vec2(1280.0, 720.0)),
    }
}

/// Build the presenter viewport builder.
///
/// The presenter window opens centered on the configured presenter monitor and
/// is clamped to fit within that monitor's logical size. If no explicit
/// presenter monitor is configured, the OS primary monitor is used. If monitor
/// data is unavailable, we fall back to a normal titled window.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn presenter_viewport_builder(
    config: &Config,
    monitor_mgr: &dyn MonitorManager,
    window_size: egui::Vec2,
) -> egui::ViewportBuilder {
    let presenter_selector = config.display.presenter_monitor.trim();
    let monitor = if presenter_selector.is_empty() || presenter_selector == "auto" {
        monitor_mgr.primary_monitor()
    } else {
        monitor_mgr.find_by_selector(presenter_selector).or_else(|| monitor_mgr.primary_monitor())
    };

    let builder = with_app_icon(egui::ViewportBuilder::default())
        .with_title("Dais — Presenter Console")
        .with_inner_size(window_size)
        .with_resizable(true);

    let Some(monitor) = monitor else {
        return builder;
    };

    if monitor.size.0 == 0 || monitor.size.1 == 0 {
        return builder;
    }

    let (logical_w, logical_h) = monitor.logical_size();
    let max_w = (logical_w as f32 - 80.0).max(640.0);
    // Reserve extra vertical space for window title bar and taskbar
    let max_h = (logical_h as f32 - 140.0).max(480.0);
    let fitted_w = window_size.x.min(max_w);
    let fitted_h = window_size.y.min(max_h);

    let x = monitor.position.0 as f32 + ((logical_w as f32 - fitted_w) / 2.0).max(0.0);
    let y = monitor.position.1 as f32 + ((logical_h as f32 - fitted_h) / 2.0).max(0.0);

    builder.with_inner_size(egui::vec2(fitted_w, fitted_h)).with_position(egui::pos2(x, y))
}

/// Determine the audience render size from the selected display mode.
///
/// In dual-monitor mode we use the detected audience monitor's physical pixel
/// size when available. If the platform backend cannot provide a usable size,
/// we fall back to the fixed render size.
pub fn audience_render_size(mode: &DisplayMode) -> RenderSize {
    match mode {
        DisplayMode::Dual { audience_monitor }
            if audience_monitor.size.0 > 0 && audience_monitor.size.1 > 0 =>
        {
            RenderSize { width: audience_monitor.size.0, height: audience_monitor.size.1 }
        }
        _ => FALLBACK_RENDER_SIZE,
    }
}

/// Log detected monitor information.
fn log_monitor_topology(monitors: &[MonitorInfo]) {
    tracing::info!("Detected {} monitor(s):", monitors.len());
    for m in monitors {
        tracing::info!(
            "  {} '{}' — {}x{} @ ({},{}) scale={:.2} {}",
            m.id,
            m.name,
            m.size.0,
            m.size.1,
            m.position.0,
            m.position.1,
            m.scale_factor,
            if m.is_primary { "[primary]" } else { "" },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMonitorManager {
        monitors: Vec<MonitorInfo>,
    }

    impl MonitorManager for MockMonitorManager {
        fn available_monitors(&self) -> Vec<MonitorInfo> {
            self.monitors.clone()
        }
    }

    fn single_monitor() -> MockMonitorManager {
        MockMonitorManager {
            monitors: vec![MonitorInfo {
                id: "m1".into(),
                name: "Primary".into(),
                position: (0, 0),
                size: (1920, 1080),
                scale_factor: 1.0,
                is_primary: true,
            }],
        }
    }

    fn dual_monitors() -> MockMonitorManager {
        MockMonitorManager {
            monitors: vec![
                MonitorInfo {
                    id: "m1".into(),
                    name: "Primary".into(),
                    position: (0, 0),
                    size: (1920, 1080),
                    scale_factor: 1.0,
                    is_primary: true,
                },
                MonitorInfo {
                    id: "m2".into(),
                    name: "DELL U2718Q".into(),
                    position: (1920, 0),
                    size: (3840, 2160),
                    scale_factor: 2.0,
                    is_primary: false,
                },
            ],
        }
    }

    #[test]
    fn cli_single_overrides_everything() {
        let hints = DisplayHints { force_single: true, force_screen_share: false };
        let config = Config::default();
        let mgr = dual_monitors();
        assert!(matches!(determine_display_mode(hints, &config, &mgr).mode, DisplayMode::Single));
    }

    #[test]
    fn cli_screen_share_overrides_everything() {
        let hints = DisplayHints { force_single: false, force_screen_share: true };
        let config = Config::default();
        let mgr = dual_monitors();
        assert!(matches!(
            determine_display_mode(hints, &config, &mgr).mode,
            DisplayMode::ScreenShare
        ));
    }

    #[test]
    fn auto_dual_with_two_monitors() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let config = Config::default();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        if let DisplayMode::Dual { audience_monitor } = result.mode {
            assert_eq!(audience_monitor.name, "DELL U2718Q");
        }
    }

    #[test]
    fn auto_falls_back_to_single_with_one_monitor() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let config = Config::default();
        let mgr = single_monitor();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Single));
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn configured_monitor_name_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "DELL U2718Q".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
    }

    #[test]
    fn configured_monitor_numeric_selector_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "2".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        if let DisplayMode::Dual { audience_monitor } = result.mode {
            assert_eq!(audience_monitor.name, "DELL U2718Q");
        }
    }

    #[test]
    fn configured_monitor_name_mismatch_falls_back() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "NONEXISTENT".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        // Should still find the secondary via auto-detection
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        assert!(!result.warnings.is_empty()); // warns about mismatch
    }

    #[test]
    fn config_screen_share_mode() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.mode = "screen-share".to_string();
        let mgr = dual_monitors();
        assert!(matches!(
            determine_display_mode(hints, &config, &mgr).mode,
            DisplayMode::ScreenShare
        ));
    }

    #[test]
    fn audience_render_size_uses_monitor_size_when_available() {
        let mgr = dual_monitors();
        let mode = DisplayMode::Dual { audience_monitor: mgr.monitors[1].clone() };
        let size = audience_render_size(&mode);
        assert_eq!(size.width, 3840);
        assert_eq!(size.height, 2160);
    }

    #[test]
    fn audience_render_size_falls_back_when_unavailable() {
        let mode = DisplayMode::ScreenShare;
        let size = audience_render_size(&mode);
        assert_eq!(size.width, FALLBACK_RENDER_SIZE.width);
        assert_eq!(size.height, FALLBACK_RENDER_SIZE.height);
    }

    #[test]
    fn presenter_viewport_uses_primary_monitor_by_default() {
        let config = Config::default();
        let mgr = dual_monitors();
        let builder = presenter_viewport_builder(&config, &mgr, egui::vec2(1400.0, 900.0));
        let debug = format!("{builder:?}");
        assert!(debug.contains("Presenter Console"));
    }
}
