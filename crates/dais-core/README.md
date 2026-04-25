# dais-core

`dais-core` contains the shared types used by the Dais presentation tool.

This crate defines the command bus, presentation state, configuration model,
keybinding model, slide grouping types, and monitor abstraction used by the
runtime crates.

Most applications should use the `dais` binary crate directly. This crate is
primarily useful for Dais internals and integrations that need to exchange typed
commands or inspect presentation state.
