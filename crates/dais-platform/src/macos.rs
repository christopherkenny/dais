//! macOS monitor management via NSScreen.

// TODO: Implement in Phase 1.2 (multi-monitor prototype)
// Uses: NSScreen::screens(), NSWindow::setFrame:display:, backingScaleFactor
// Known issue: egui/winit tab-instead-of-window, workaround: setTabbingMode(.disallowed)
