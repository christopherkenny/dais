//! Monitor management trait and types.
//!
//! Platform-specific implementations live in `dais-platform`.
//! The trait is defined here so both platform backends and the engine can use it.

/// Information about a connected display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Unique identifier for this monitor (platform-specific).
    pub id: String,
    /// Human-readable name (e.g., "DELL U2718Q").
    pub name: String,
    /// Position on the virtual desktop, top-left corner (pixels).
    pub position: (i32, i32),
    /// Physical resolution (width, height in pixels).
    pub size: (u32, u32),
    /// DPI scale factor (1.0 = 96dpi, 2.0 = Retina/HiDPI).
    pub scale_factor: f64,
    /// Whether this is the OS primary monitor.
    pub is_primary: bool,
}

impl MonitorInfo {
    /// The effective logical size (physical size / scale factor).
    pub fn logical_size(&self) -> (f64, f64) {
        (f64::from(self.size.0) / self.scale_factor, f64::from(self.size.1) / self.scale_factor)
    }
}

/// Trait for platform-specific monitor enumeration.
///
/// Separate implementations are compiled via `cfg` for each platform
/// in the `dais-platform` crate.
pub trait MonitorManager: Send + Sync {
    /// Enumerate all currently connected monitors.
    fn available_monitors(&self) -> Vec<MonitorInfo>;

    /// Get the primary monitor, if one is designated.
    fn primary_monitor(&self) -> Option<MonitorInfo> {
        self.available_monitors().into_iter().find(|m| m.is_primary)
    }

    /// Get a non-primary monitor (for audience display in dual mode).
    fn secondary_monitor(&self) -> Option<MonitorInfo> {
        self.available_monitors().into_iter().find(|m| !m.is_primary)
    }

    /// Find a monitor by name (for config-based assignment).
    fn find_by_name(&self, name: &str) -> Option<MonitorInfo> {
        self.available_monitors().into_iter().find(|m| m.name == name)
    }

    /// Find a monitor by a user-facing selector.
    ///
    /// Supported forms:
    /// - Exact monitor name
    /// - Exact monitor id
    /// - 1-based ordinal like "1", "2", "3" using the detected monitor list order
    fn find_by_selector(&self, selector: &str) -> Option<MonitorInfo> {
        let monitors = self.available_monitors();

        if let Ok(index) = selector.parse::<usize>()
            && index > 0
        {
            return monitors.get(index - 1).cloned();
        }

        monitors
            .iter()
            .find(|m| m.name == selector || m.id == selector)
            .cloned()
    }
}
