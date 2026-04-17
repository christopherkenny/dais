//! Monitor management trait and assignment logic.

/// Information about a connected monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Unique identifier for this monitor.
    pub id: String,
    /// Human-readable name (e.g., "DELL U2718Q").
    pub name: String,
    /// Position on the virtual desktop (pixels).
    pub position: (i32, i32),
    /// Physical resolution (pixels).
    pub size: (u32, u32),
    /// DPI scale factor (1.0 = 96dpi, 2.0 = Retina).
    pub scale_factor: f64,
    /// Whether this is the OS primary monitor.
    pub is_primary: bool,
}

/// Trait for platform-specific monitor enumeration and window placement.
///
/// Separate implementations are compiled in via `cfg` for each platform.
pub trait MonitorManager: Send + Sync {
    /// Enumerate all currently connected monitors.
    fn available_monitors(&self) -> Vec<MonitorInfo>;

    /// Get the primary monitor, if one is designated.
    fn primary_monitor(&self) -> Option<MonitorInfo>;
}
