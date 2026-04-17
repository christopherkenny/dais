//! macOS monitor management via NSScreen.
//!
//! The full backend is not implemented yet, but CI for the macOS target still
//! needs a concrete type so the platform crate compiles on every OS.

use dais_core::monitor::{MonitorInfo, MonitorManager};

/// Placeholder macOS monitor manager.
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
        Vec::new()
    }
}
