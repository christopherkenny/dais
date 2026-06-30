# dais-remote

`dais-remote` provides the local HTTP remote-control server for Dais.

It exposes a REST API, server-sent events stream, browser remote UI, and CLI client used by `dais remote ...` subcommands.
Slide control commands enter the same command bus as keyboard input, so the presentation engine remains the sole owner of presentation state.

Most users should run the `dais` binary directly.
This crate is an internal implementation detail of the Dais workspace.
