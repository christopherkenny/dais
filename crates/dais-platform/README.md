# dais-platform

`dais-platform` contains platform-specific monitor discovery for Dais.

It exposes a common monitor manager interface backed by Windows, macOS, and
Linux implementations where available. Dais uses this crate to choose presenter
and audience displays and to fall back gracefully on single-monitor setups.

The `wayland` feature is reserved for a future Linux Wayland backend and is not
implemented yet.
