//! macOS monitor management via `NSScreen`.

use dais_core::monitor::{MonitorInfo, MonitorManager};
use objc2::ffi::CGFloat;
use objc2::rc::Retained;
use objc2_app_kit::NSScreen;
use objc2_foundation::{MainThreadMarker, NSArray};

pub struct MacOsMonitorManager;

impl MacOsMonitorManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOsMonitorManager {
    fn default() -> Self {
        Self::new()
    }
}

fn cgfloat_to_i32(value: CGFloat) -> i32 {
    let rounded = value.round();

    if !rounded.is_finite() {
        return 0;
    }

    let min = f64::from(i32::MIN);
    let max = f64::from(i32::MAX);
    let clamped = rounded.clamp(min, max);

    clamped as i32
}

fn cgfloat_to_u32(value: CGFloat) -> u32 {
    let rounded = value.round();

    if !rounded.is_finite() {
        return 0;
    }

    let max = f64::from(u32::MAX);
    let clamped = rounded.clamp(0.0, max);

    clamped as u32
}

impl MonitorManager for MacOsMonitorManager {
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        // NSScreen must be accessed from the main thread.
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::warn!("available_monitors called off main thread; returning empty list");
            return Vec::new();
        };

        let screens: Retained<NSArray<NSScreen>> = NSScreen::screens(mtm);
        let main_screen = NSScreen::mainScreen(mtm);

        // The primary screen's height is needed to convert macOS's bottom-left
        // coordinate origin to the top-left convention used by MonitorInfo.
        let primary_height =
            main_screen.as_deref().map_or(0.0, |screen| screen.frame().size.height);

        screens
            .iter()
            .enumerate()
            .map(|(i, screen)| {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                let scale = screen.backingScaleFactor();

                let x = cgfloat_to_i32(frame.origin.x);
                let y = cgfloat_to_i32(primary_height - (frame.origin.y + frame.size.height));
                let w = cgfloat_to_u32(frame.size.width);
                let h = cgfloat_to_u32(frame.size.height);

                let vx = cgfloat_to_i32(visible.origin.x);
                let vy = cgfloat_to_i32(primary_height - (visible.origin.y + visible.size.height));
                let vw = cgfloat_to_u32(visible.size.width);
                let vh = cgfloat_to_u32(visible.size.height);

                let is_primary = main_screen.as_deref().map_or(i == 0, |main| {
                    std::ptr::eq(std::ptr::from_ref(main), &raw const *screen)
                });

                let id = format!("NSScreen:{i}");
                let name = if is_primary {
                    "Built-in Display".to_string()
                } else {
                    format!("Display {}", i + 1)
                };

                MonitorInfo {
                    id,
                    name,
                    position: (x, y),
                    size: (w, h),
                    work_area: (vx, vy, vw, vh),
                    scale_factor: scale,
                    is_primary,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_at_least_one_monitor() {
        let mgr = MacOsMonitorManager::new();
        let monitors = mgr.available_monitors();
        assert!(!monitors.is_empty(), "should detect at least one monitor");
    }

    #[test]
    fn has_one_primary() {
        let mgr = MacOsMonitorManager::new();
        let monitors = mgr.available_monitors();
        let primary_count = monitors.iter().filter(|m| m.is_primary).count();
        assert_eq!(primary_count, 1, "exactly one monitor should be primary");
    }

    #[test]
    fn monitor_has_nonzero_size() {
        let mgr = MacOsMonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(m.size.0 > 0, "monitor width should be > 0: {}", m.name);
            assert!(m.size.1 > 0, "monitor height should be > 0: {}", m.name);
        }
    }

    #[test]
    fn scale_factor_is_reasonable() {
        let mgr = MacOsMonitorManager::new();
        for m in mgr.available_monitors() {
            assert!(
                (0.5..=4.0).contains(&m.scale_factor),
                "scale factor {} out of expected range for {}",
                m.scale_factor,
                m.name
            );
        }
    }

    #[test]
    fn primary_monitor_helper_works() {
        let mgr = MacOsMonitorManager::new();
        let primary = mgr.primary_monitor();
        assert!(primary.is_some(), "primary_monitor() should return Some");
        assert!(primary.unwrap().is_primary);
    }
}
