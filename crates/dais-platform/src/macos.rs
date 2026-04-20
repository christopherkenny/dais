//! macOS monitor management via `NSScreen`.

use dais_core::monitor::{MonitorInfo, MonitorManager};
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

impl MonitorManager for MacOsMonitorManager {
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        // NSScreen must be accessed from the main thread.
        let mtm = match MainThreadMarker::new() {
            Some(m) => m,
            None => {
                tracing::warn!("available_monitors called off main thread; returning empty list");
                return Vec::new();
            }
        };

        let screens: Retained<NSArray<NSScreen>> = NSScreen::screens(mtm);
        let main_screen = NSScreen::mainScreen(mtm);

        // The primary screen's height is needed to convert macOS's bottom-left
        // coordinate origin to the top-left convention used by MonitorInfo.
        let primary_height =
            main_screen.as_deref().map(|s| unsafe { s.frame() }.size.height).unwrap_or(0.0);

        screens
            .iter()
            .enumerate()
            .map(|(i, screen)| {
                let frame = unsafe { screen.frame() };
                let visible = unsafe { screen.visibleFrame() };
                let scale = unsafe { screen.backingScaleFactor() };

                let x = frame.origin.x as i32;
                let y = (primary_height - (frame.origin.y + frame.size.height)) as i32;
                let w = frame.size.width as u32;
                let h = frame.size.height as u32;

                let vx = visible.origin.x as i32;
                let vy = (primary_height - (visible.origin.y + visible.size.height)) as i32;
                let vw = visible.size.width as u32;
                let vh = visible.size.height as u32;

                let is_primary = main_screen
                    .as_deref()
                    .map(|s| std::ptr::eq(s as *const NSScreen, &*screen as *const NSScreen))
                    .unwrap_or(i == 0);

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
