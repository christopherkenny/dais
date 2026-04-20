//! Platform-specific monitor management backends for Dais.
//!
//! Each platform has a separate module compiled in via `cfg`.
//! All implement the [`MonitorManager`](dais_core::monitor::MonitorManager) trait.

// Re-export the trait for convenience.
pub use dais_core::monitor::{MonitorInfo, MonitorManager};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(all(target_os = "linux", feature = "x11"))]
pub mod linux_x11;

#[cfg(all(target_os = "linux", feature = "wayland"))]
pub mod linux_wayland;

/// Create a platform-appropriate [`MonitorManager`] implementation.
#[cfg(target_os = "windows")]
pub fn create_monitor_manager() -> impl MonitorManager {
    windows::WindowsMonitorManager::new()
}

#[cfg(target_os = "macos")]
pub fn create_monitor_manager() -> impl MonitorManager {
    macos::MacOsMonitorManager::new()
}

#[cfg(all(target_os = "linux", feature = "x11"))]
pub fn create_monitor_manager() -> impl MonitorManager {
    linux_x11::X11MonitorManager::new()
}

#[cfg(all(target_os = "linux", not(feature = "x11")))]
pub fn create_monitor_manager() -> impl MonitorManager {
    LinuxFallbackMonitorManager
}

/// Returned when no Linux display backend feature is enabled.
#[cfg(all(target_os = "linux", not(feature = "x11")))]
struct LinuxFallbackMonitorManager;

#[cfg(all(target_os = "linux", not(feature = "x11")))]
impl MonitorManager for LinuxFallbackMonitorManager {
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        tracing::warn!("no Linux display backend compiled in; build with --features x11");
        Vec::new()
    }
}
