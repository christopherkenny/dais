//! Linux/Wayland monitor management (best-effort).
//!
//! Wayland does not expose window placement to applications.
//! Fullscreen requests via `xdg_toplevel::set_fullscreen(output)` are
//! compositor-dependent. This is a documented limitation.

// TODO: Implement in Phase 1.2 (multi-monitor prototype)
