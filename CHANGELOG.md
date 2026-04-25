# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Initial release of Dais — a cross-platform native PDF presenter console.
- Multi-monitor presenter view with audience display and presenter console.
- Overlay and build-step support via Polylux, touying, and Beamer `\pdfpc` metadata.
- Presentation aids: laser pointer, freehand ink, spotlight, and zoom.
- `.pdfpc` sidecar compatibility for notes and overlay grouping.
- Built-in slide grouping editor (`dais --edit <file.pdf>`).
- Fully remappable keybindings with pdfpc-compatible defaults.
- Countdown and elapsed timer with color-coded warning/overrun phases.
- Display modes: dual, single, and screen-share.
- Automatic monitor detection with graceful single-monitor fallback.
- Markdown notes rendering via egui_commonmark.
- Slide overview grid with keyboard navigation.
- Freeze and blackout audience display controls.
- TOML configuration with platform-appropriate paths.
- Single-binary distribution (no runtime dependencies).
- CI on Windows, macOS, and Linux.
