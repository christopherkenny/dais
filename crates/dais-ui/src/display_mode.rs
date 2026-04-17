//! Display mode management — intelligent window placement based on monitor topology.
//!
//! Determines how the presenter and audience windows should be positioned
//! based on CLI flags, configuration, and detected monitors.

use dais_core::config::Config;
use dais_core::monitor::{MonitorInfo, MonitorManager};

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

/// Determine the initial display mode from CLI hints, config, and detected monitors.
pub fn determine_display_mode(
    hints: DisplayHints,
    config: &Config,
    monitor_mgr: &dyn MonitorManager,
) -> DisplayMode {
    // CLI flags take absolute precedence
    if hints.force_single {
        tracing::info!("Single mode requested via --single flag");
        return DisplayMode::Single;
    }
    if hints.force_screen_share {
        tracing::info!("Screen-share mode requested via --screen-share flag");
        return DisplayMode::ScreenShare;
    }

    // Config-based mode preference
    let config_mode = config.display.mode.to_lowercase();

    let monitors = monitor_mgr.available_monitors();
    log_monitor_topology(&monitors);

    match config_mode.as_str() {
        "single" => {
            tracing::info!("Single mode set in config");
            DisplayMode::Single
        }
        "screen-share" | "screenshare" | "screen_share" => {
            tracing::info!("Screen-share mode set in config");
            DisplayMode::ScreenShare
        }
        // "dual" or "auto" (default) — try to find a secondary monitor
        _ => resolve_dual_mode(config, &monitors, monitor_mgr),
    }
}

/// Attempt to resolve dual mode, falling back gracefully.
fn resolve_dual_mode(
    config: &Config,
    monitors: &[MonitorInfo],
    monitor_mgr: &dyn MonitorManager,
) -> DisplayMode {
    // If a specific audience monitor selector is configured, try to match it.
    // This accepts a full monitor name/id or a 1-based ordinal like "2".
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
        // Configured name doesn't match — warn and fall back
        tracing::warn!(
            "Configured audience monitor '{}' not found. Available monitors: {}",
            audience_name,
            monitors.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    // Auto-detect: use first non-primary monitor
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

    // Only one monitor — graceful degradation
    tracing::info!("Single monitor detected, using screen-share mode");
    DisplayMode::ScreenShare
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
            egui::ViewportBuilder::default()
                .with_title("Dais — Audience")
                .with_fullscreen(true)
                .with_position(egui::pos2(
                    audience_monitor.position.0 as f32,
                    audience_monitor.position.1 as f32,
                ))
        }
        DisplayMode::Single => {
            // Single mode doesn't spawn an audience viewport — this is a fallback
            egui::ViewportBuilder::default()
                .with_title("Dais — Audience")
                .with_inner_size(egui::vec2(1280.0, 720.0))
        }
        DisplayMode::ScreenShare => egui::ViewportBuilder::default()
            .with_title("Dais — Audience")
            .with_inner_size(egui::vec2(1280.0, 720.0)),
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
        assert!(matches!(determine_display_mode(hints, &config, &mgr), DisplayMode::Single));
    }

    #[test]
    fn cli_screen_share_overrides_everything() {
        let hints = DisplayHints { force_single: false, force_screen_share: true };
        let config = Config::default();
        let mgr = dual_monitors();
        assert!(matches!(determine_display_mode(hints, &config, &mgr), DisplayMode::ScreenShare));
    }

    #[test]
    fn auto_dual_with_two_monitors() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let config = Config::default(); // mode = "dual", audience_monitor = "auto"
        let mgr = dual_monitors();
        let mode = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(mode, DisplayMode::Dual { .. }));
        if let DisplayMode::Dual { audience_monitor } = mode {
            assert_eq!(audience_monitor.name, "DELL U2718Q");
        }
    }

    #[test]
    fn auto_falls_back_to_screen_share_with_one_monitor() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let config = Config::default();
        let mgr = single_monitor();
        assert!(matches!(determine_display_mode(hints, &config, &mgr), DisplayMode::ScreenShare));
    }

    #[test]
    fn configured_monitor_name_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "DELL U2718Q".to_string();
        let mgr = dual_monitors();
        let mode = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(mode, DisplayMode::Dual { .. }));
    }

    #[test]
    fn configured_monitor_numeric_selector_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "2".to_string();
        let mgr = dual_monitors();
        let mode = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(mode, DisplayMode::Dual { .. }));
        if let DisplayMode::Dual { audience_monitor } = mode {
            assert_eq!(audience_monitor.name, "DELL U2718Q");
        }
    }

    #[test]
    fn configured_monitor_name_mismatch_falls_back() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "NONEXISTENT".to_string();
        let mgr = dual_monitors();
        // Should still find the secondary via auto-detection
        let mode = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(mode, DisplayMode::Dual { .. }));
    }

    #[test]
    fn config_screen_share_mode() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.mode = "screen-share".to_string();
        let mgr = dual_monitors();
        assert!(matches!(determine_display_mode(hints, &config, &mgr), DisplayMode::ScreenShare));
    }
}
