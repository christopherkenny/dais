# dais-engine

`dais-engine` is the state authority for Dais presentations.

It consumes typed commands from `dais-core`, updates the authoritative
`PresentationState`, manages navigation, timers, presentation aids, and
annotation persistence, then publishes state for UI consumers.

Most users should run the `dais` binary. This crate is intended for the Dais
runtime and for advanced integrations that need to drive the presentation engine
programmatically.
