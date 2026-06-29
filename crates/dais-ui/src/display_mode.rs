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
    /// Dual monitor: presenter console on one monitor, audience fullscreen on another.
    Dual { presenter_monitor: MonitorInfo, audience_monitor: MonitorInfo },
    /// Single window with presenter only (no audience viewport).
    Single,
    /// Audience as a normal resizable window (for screen sharing).
    ScreenShare,
}

/// Which presentation surface to use in single-monitor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleMonitorView {
    /// Fullscreen slide with hoverable notes bar.
    Hud,
    /// Audience slide beside a compact presenter strip.
    Split,
}

impl SingleMonitorView {
    /// Parse a config value into a single-monitor view mode.
    ///
    /// Unknown values fall back to [`SingleMonitorView::Hud`].
    pub fn from_config(value: &str) -> Self {
        if value.eq_ignore_ascii_case("split") { Self::Split } else { Self::Hud }
    }
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
        match eframe::icon_data::from_png_bytes(include_bytes!("../assets/dais.png")) {
            Ok(icon) => Some(Arc::new(icon)),
            Err(err) => {
                tracing::warn!("Failed to load app icon from bundled assets/dais.png: {err}");
                None
            }
        }
    })
    .clone()
}

/// Attach the bundled Dais icon to an egui viewport builder when it can be decoded.
pub fn with_app_icon(builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
    if let Some(icon) = app_icon() { builder.with_icon(icon) } else { builder }
}

/// Result of display mode determination, including any warnings.
pub struct DisplayModeResult {
    /// Selected display mode.
    pub mode: DisplayMode,
    /// Human-readable warnings for recoverable monitor/config issues.
    pub warnings: Vec<String>,
    /// Recovery data when the configured audience monitor was unavailable.
    pub audience_reassignment: Option<AudienceReassignmentPrompt>,
}

/// Interactive recovery data when the configured audience monitor is unavailable.
#[derive(Debug, Clone)]
pub struct AudienceReassignmentPrompt {
    /// The configured monitor selector that could not be resolved.
    pub missing_selector: String,
    /// Non-primary monitor chosen as a temporary fallback, if any.
    pub attempted_fallback: Option<MonitorInfo>,
    /// Available monitors that the user could choose from.
    pub available_monitors: Vec<MonitorInfo>,
}

/// Runtime presenter viewport placement for a specific monitor.
#[derive(Debug, Clone, Copy)]
pub struct PresenterViewportPlacement {
    /// Outer window position.
    pub position: egui::Pos2,
    /// Inner window size.
    pub inner_size: egui::Vec2,
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
        return DisplayModeResult {
            mode: DisplayMode::Single,
            warnings,
            audience_reassignment: None,
        };
    }
    if hints.force_screen_share {
        tracing::info!("Screen-share mode requested via --screen-share flag");
        return DisplayModeResult {
            mode: DisplayMode::ScreenShare,
            warnings,
            audience_reassignment: None,
        };
    }

    // Config-based mode preference
    let config_mode = config.display.mode.to_lowercase();

    let monitors = monitor_mgr.available_monitors();
    log_monitor_topology(&monitors);

    let mut audience_reassignment = None;
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
        _ => {
            let (mode, prompt) = resolve_dual_mode(config, &monitors, monitor_mgr, &mut warnings);
            audience_reassignment = prompt;
            mode
        }
    };

    DisplayModeResult { mode, warnings, audience_reassignment }
}

/// Attempt to resolve dual mode, falling back gracefully.
fn resolve_dual_mode(
    config: &Config,
    monitors: &[MonitorInfo],
    monitor_mgr: &dyn MonitorManager,
    warnings: &mut Vec<String>,
) -> (DisplayMode, Option<AudienceReassignmentPrompt>) {
    let audience_name = &config.display.audience_monitor;
    if audience_name != "auto" && !audience_name.is_empty() {
        if let Some(mon) = monitor_mgr.find_by_selector(audience_name) {
            let presenter = resolve_presenter_monitor(config, monitors, monitor_mgr, &mon);
            tracing::info!(
                "Using configured audience monitor '{}' -> {} '{}'",
                audience_name,
                mon.id,
                mon.name
            );
            return (
                DisplayMode::Dual { presenter_monitor: presenter, audience_monitor: mon },
                None,
            );
        }
        let available = monitors.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ");
        let msg = format!(
            "Configured audience monitor '{audience_name}' not found. Available: {available}",
        );
        tracing::warn!("{msg}");
        warnings.push(msg);

        let attempted_fallback = monitor_mgr.secondary_monitor();
        let available_monitors = monitors.to_vec();
        let prompt = Some(AudienceReassignmentPrompt {
            missing_selector: audience_name.clone(),
            attempted_fallback: attempted_fallback.clone(),
            available_monitors,
        });

        if let Some(secondary) = attempted_fallback {
            let presenter = resolve_presenter_monitor(config, monitors, monitor_mgr, &secondary);
            tracing::info!(
                "Dual mode fallback: audience on '{}' ({}x{} @ {:?})",
                secondary.name,
                secondary.size.0,
                secondary.size.1,
                secondary.position
            );
            return (
                DisplayMode::Dual { presenter_monitor: presenter, audience_monitor: secondary },
                prompt,
            );
        }

        let msg = "Single monitor detected — expected dual. Using single mode.".to_string();
        tracing::info!("{msg}");
        warnings.push(msg);
        return (DisplayMode::Single, prompt);
    }

    if let Some(secondary) = monitor_mgr.secondary_monitor() {
        let presenter = resolve_presenter_monitor(config, monitors, monitor_mgr, &secondary);
        tracing::info!(
            "Dual mode: audience on '{}' ({}x{} @ {:?})",
            secondary.name,
            secondary.size.0,
            secondary.size.1,
            secondary.position
        );
        return (
            DisplayMode::Dual { presenter_monitor: presenter, audience_monitor: secondary },
            None,
        );
    }

    let msg = "Single monitor detected — expected dual. Using single mode.".to_string();
    tracing::info!("{msg}");
    warnings.push(msg);
    (DisplayMode::Single, None)
}

fn resolve_presenter_monitor(
    config: &Config,
    monitors: &[MonitorInfo],
    monitor_mgr: &dyn MonitorManager,
    audience_monitor: &MonitorInfo,
) -> MonitorInfo {
    let presenter_selector = config.display.presenter_monitor.trim();
    let configured = if presenter_selector.is_empty() || presenter_selector == "auto" {
        None
    } else {
        monitor_mgr.find_by_selector(presenter_selector)
    };

    configured
        .or_else(|| monitor_mgr.primary_monitor())
        .filter(|monitor| monitor.id != audience_monitor.id)
        .or_else(|| monitors.iter().find(|monitor| monitor.id != audience_monitor.id).cloned())
        .unwrap_or_else(|| audience_monitor.clone())
}

/// Return the active dual-mode monitor assignment after applying a runtime swap.
pub fn effective_display_mode(mode: &DisplayMode, displays_swapped: bool) -> DisplayMode {
    match mode {
        DisplayMode::Dual { presenter_monitor, audience_monitor } if displays_swapped => {
            DisplayMode::Dual {
                presenter_monitor: audience_monitor.clone(),
                audience_monitor: presenter_monitor.clone(),
            }
        }
        _ => mode.clone(),
    }
}

/// Build the audience viewport builder for the given display mode.
#[allow(clippy::cast_precision_loss)]
pub fn audience_viewport_builder(mode: &DisplayMode) -> egui::ViewportBuilder {
    match mode {
        DisplayMode::Dual { audience_monitor, .. } => {
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
/// The presenter window opens centered horizontally on the configured presenter
/// monitor and positioned in the upper portion of that monitor's usable work
/// area so it avoids overlapping OS taskbars/docks. If no explicit presenter
/// monitor is configured, the OS primary monitor is used. If monitor data is
/// unavailable, we fall back to a normal titled window.
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

    match monitor {
        Some(monitor) => presenter_viewport_builder_for_monitor(&monitor, window_size),
        None => presenter_viewport_builder_without_monitor(),
    }
}

fn presenter_viewport_builder_without_monitor() -> egui::ViewportBuilder {
    with_app_icon(egui::ViewportBuilder::default())
        .with_title("Dais — Presenter Console")
        .with_resizable(true)
        .with_maximized(true)
}

/// Build the presenter viewport on a specific monitor.
#[allow(clippy::cast_precision_loss)]
pub fn presenter_viewport_builder_for_monitor(
    monitor: &MonitorInfo,
    window_size: egui::Vec2,
) -> egui::ViewportBuilder {
    let builder = presenter_viewport_builder_without_monitor();

    let Some(placement) = presenter_viewport_placement(monitor, window_size) else {
        return builder;
    };

    builder.with_inner_size(placement.inner_size).with_position(placement.position)
}

/// Calculate presenter viewport placement for a specific monitor.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn presenter_viewport_placement(
    monitor: &MonitorInfo,
    window_size: egui::Vec2,
) -> Option<PresenterViewportPlacement> {
    if monitor.size.0 == 0 || monitor.size.1 == 0 {
        return None;
    }
    let (_logical_work_x, _logical_work_y, logical_work_w, logical_work_h) =
        monitor.logical_work_area();
    let (logical_monitor_w, logical_monitor_h) = monitor.logical_size();
    let usable_w = if monitor.work_area.2 > 0 { logical_work_w } else { logical_monitor_w };
    let usable_h = if monitor.work_area.3 > 0 { logical_work_h } else { logical_monitor_h };

    let max_w = (usable_w as f32 - 20.0).max(640.0);
    let max_h = (usable_h as f32 - 60.0).max(480.0);
    let target_w = window_size.x.min(max_w);
    let target_h = window_size.y.min(max_h);

    let work_x = if monitor.work_area.2 > 0 {
        monitor.work_area.0 as f32 / monitor.scale_factor as f32
    } else {
        monitor.position.0 as f32 / monitor.scale_factor as f32
    };
    let work_y = if monitor.work_area.3 > 0 {
        monitor.work_area.1 as f32 / monitor.scale_factor as f32
    } else {
        monitor.position.1 as f32 / monitor.scale_factor as f32
    };
    let x = work_x + ((usable_w as f32 - target_w) / 2.0).max(0.0);
    let top_margin: f32 = 24.0;
    let y = work_y + top_margin.min((usable_h as f32 - target_h).max(0.0));

    Some(PresenterViewportPlacement {
        position: egui::pos2(x, y),
        inner_size: egui::vec2(target_w, target_h),
    })
}

/// Determine the audience render size from the selected display mode.
///
/// In dual-monitor mode we use the detected audience monitor's physical pixel
/// size when available. If the platform backend cannot provide a usable size,
/// we fall back to the fixed render size.
pub fn audience_render_size(mode: &DisplayMode) -> RenderSize {
    match mode {
        DisplayMode::Dual { audience_monitor, .. }
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
                work_area: (0, 0, 1920, 1040),
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
                    work_area: (0, 0, 1920, 1040),
                    scale_factor: 1.0,
                    is_primary: true,
                },
                MonitorInfo {
                    id: "m2".into(),
                    name: "DELL U2718Q".into(),
                    position: (1920, 0),
                    size: (3840, 2160),
                    work_area: (1920, 0, 3840, 2120),
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
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Single));
        assert!(result.audience_reassignment.is_none());
    }

    #[test]
    fn cli_screen_share_overrides_everything() {
        let hints = DisplayHints { force_single: false, force_screen_share: true };
        let config = Config::default();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::ScreenShare));
        assert!(result.audience_reassignment.is_none());
    }

    #[test]
    fn auto_dual_with_two_monitors() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let config = Config::default();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        assert!(result.audience_reassignment.is_none());
        if let DisplayMode::Dual { audience_monitor, .. } = result.mode {
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
        assert!(result.audience_reassignment.is_none());
    }

    #[test]
    fn configured_monitor_name_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "DELL U2718Q".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        assert!(result.audience_reassignment.is_none());
    }

    #[test]
    fn configured_monitor_numeric_selector_matches() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "2".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::Dual { .. }));
        assert!(result.audience_reassignment.is_none());
        if let DisplayMode::Dual { audience_monitor, .. } = result.mode {
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
        let prompt = result.audience_reassignment.expect("missing reassignment prompt");
        assert_eq!(prompt.missing_selector, "NONEXISTENT");
        assert!(prompt.attempted_fallback.is_some());
        assert_eq!(prompt.available_monitors.len(), 2);
    }

    #[test]
    fn configured_monitor_mismatch_on_one_monitor_can_reassign_to_primary() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.audience_monitor = "NONEXISTENT".to_string();
        let mgr = single_monitor();
        let result = determine_display_mode(hints, &config, &mgr);

        assert!(matches!(result.mode, DisplayMode::Single));
        let prompt = result.audience_reassignment.expect("missing reassignment prompt");
        assert!(prompt.attempted_fallback.is_none());
        assert_eq!(prompt.available_monitors.len(), 1);
        assert!(prompt.available_monitors[0].is_primary);
    }

    #[test]
    fn config_screen_share_mode() {
        let hints = DisplayHints { force_single: false, force_screen_share: false };
        let mut config = Config::default();
        config.display.mode = "screen-share".to_string();
        let mgr = dual_monitors();
        let result = determine_display_mode(hints, &config, &mgr);
        assert!(matches!(result.mode, DisplayMode::ScreenShare));
        assert!(result.audience_reassignment.is_none());
    }

    #[test]
    fn audience_render_size_uses_monitor_size_when_available() {
        let mgr = dual_monitors();
        let mode = DisplayMode::Dual {
            presenter_monitor: mgr.monitors[0].clone(),
            audience_monitor: mgr.monitors[1].clone(),
        };
        let size = audience_render_size(&mode);
        assert_eq!(size.width, 3840);
        assert_eq!(size.height, 2160);
    }

    #[test]
    fn effective_display_mode_swaps_dual_monitors() {
        let mgr = dual_monitors();
        let mode = DisplayMode::Dual {
            presenter_monitor: mgr.monitors[0].clone(),
            audience_monitor: mgr.monitors[1].clone(),
        };

        let swapped = effective_display_mode(&mode, true);

        if let DisplayMode::Dual { presenter_monitor, audience_monitor } = swapped {
            assert_eq!(presenter_monitor.id, "m2");
            assert_eq!(audience_monitor.id, "m1");
        } else {
            panic!("dual mode should remain dual after swapping");
        }
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
