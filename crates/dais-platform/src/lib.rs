//! Platform-specific monitor management backends for Dais.
//!
//! Each platform has a separate module compiled in via `cfg`.
//! All implement the [`MonitorManager`](dais_engine::monitor::MonitorManager) trait.

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(all(target_os = "linux", feature = "x11"))]
pub mod linux_x11;

#[cfg(all(target_os = "linux", feature = "wayland"))]
pub mod linux_wayland;
