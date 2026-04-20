//! Linux/X11 monitor management via `x11rb` and the RandR extension.

use dais_core::monitor::{MonitorInfo, MonitorManager};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

pub struct X11MonitorManager;

impl X11MonitorManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for X11MonitorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorManager for X11MonitorManager {
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        match enumerate_monitors() {
            Ok(monitors) => monitors,
            Err(e) => {
                tracing::warn!("X11 monitor enumeration failed: {e}");
                Vec::new()
            }
        }
    }
}

fn enumerate_monitors() -> Result<Vec<MonitorInfo>, Box<dyn std::error::Error>> {
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Ask RandR for the current screen resources (CRTCs, outputs, modes).
    let resources = conn.randr_get_screen_resources_current(root)?.reply()?;

    // Find the primary output so we can mark it.
    let primary_output = conn.randr_get_output_primary(root)?.reply()?.output;

    let mut monitors = Vec::new();

    for &crtc in &resources.crtcs {
        let info = conn.randr_get_crtc_info(crtc, 0)?.reply()?;

        // Skip CRTCs with no active outputs or zero size.
        if info.outputs.is_empty() || info.width == 0 || info.height == 0 {
            continue;
        }

        // Use the first connected output on this CRTC for name/primary info.
        let output = info.outputs[0];
        let output_info = conn.randr_get_output_info(output, 0)?.reply()?;
        let name = String::from_utf8_lossy(&output_info.name).into_owned();

        let is_primary = output == primary_output;

        // RandR doesn't expose DPI per-output directly; derive from mm size if available.
        let scale_factor = if output_info.mm_width > 0 {
            let dpi = f64::from(info.width) / (f64::from(output_info.mm_width) / 25.4);
            // Round to nearest common factor (96/144/192 → 1.0/1.5/2.0).
            (dpi / 96.0 * 4.0).round() / 4.0
        } else {
            1.0
        };
        let scale_factor = scale_factor.max(0.5);

        let x = i32::from(info.x);
        let y = i32::from(info.y);
        let w = u32::from(info.width);
        let h = u32::from(info.height);
        let id = format!("CRTC:{crtc}:{name}");

        // X11/RandR has no concept of work area; report full geometry.
        monitors.push(MonitorInfo {
            id,
            name,
            position: (x, y),
            size: (w, h),
            work_area: (x, y, w, h),
            scale_factor,
            is_primary,
        });
    }

    // If RandR returned nothing (e.g. no extension), fall back to the root window geometry.
    if monitors.is_empty() {
        let w = u32::from(screen.width_in_pixels);
        let h = u32::from(screen.height_in_pixels);
        monitors.push(MonitorInfo {
            id: "root:0".into(),
            name: "Default Screen".into(),
            position: (0, 0),
            size: (w, h),
            work_area: (0, 0, w, h),
            scale_factor: 1.0,
            is_primary: true,
        });
    }

    Ok(monitors)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require a live X11 display; they are skipped in CI unless
    // DISPLAY is set.
    fn has_display() -> bool {
        std::env::var("DISPLAY").is_ok()
    }

    #[test]
    fn enumerate_at_least_one_monitor() {
        if !has_display() {
            return;
        }
        let mgr = X11MonitorManager::new();
        let monitors = mgr.available_monitors();
        assert!(!monitors.is_empty(), "should detect at least one monitor");
    }

    #[test]
    fn monitor_has_nonzero_size() {
        if !has_display() {
            return;
        }
        let mgr = X11MonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(m.size.0 > 0, "monitor width should be > 0: {}", m.name);
            assert!(m.size.1 > 0, "monitor height should be > 0: {}", m.name);
        }
    }

    #[test]
    fn scale_factor_is_reasonable() {
        if !has_display() {
            return;
        }
        let mgr = X11MonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(
                (0.5..=4.0).contains(&m.scale_factor),
                "scale factor {} out of expected range for {}",
                m.scale_factor,
                m.name
            );
        }
    }
}
