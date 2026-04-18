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

// Linux stub — will use X11 or Wayland depending on features
#[cfg(target_os = "linux")]
pub fn create_monitor_manager() -> impl MonitorManager {
    // TODO: detect Wayland vs X11 at runtime and use real backends
    LinuxStubMonitorManager
}

/// Placeholder Linux monitor manager until X11/Wayland backends land.
#[cfg(target_os = "linux")]
struct LinuxStubMonitorManager;

#[cfg(target_os = "linux")]
impl MonitorManager for LinuxStubMonitorManager {
    fn available_monitors(&self) -> Vec<MonitorInfo> {
        Vec::new()
    }
}
