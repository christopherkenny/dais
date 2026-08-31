# `dais`: A Native PDF Presenter Console <img src='assets/dais.png' align="right" height="150" />

[![CI](https://github.com/christopherkenny/dais/actions/workflows/ci.yml/badge.svg)](https://github.com/christopherkenny/dais/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/dais.svg)](https://crates.io/crates/dais)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`dais` is a cross-platform PDF presentation console written in Rust for researchers and academics who build slides in LaTeX/Beamer, Typst, PowerPoint, or Keynote.
`dais` is designed for straightforward installation, reliable operation in real presentation setups, and compatibility with existing slide workflows.

## Features

- Multi-monitor presenter view with an audience display and a presenter console with notes, timer, and navigation.
- Overlay and build-step support for `pdfpc` metadata, Beamer `\pdfpc`, and manual grouping.
- Presentation tools including laser pointer styles, freehand ink, highlighter, eraser, whiteboard, spotlight, zoom, freeze, blackout, and a slide overview grid.
- Browser remote at `/remote` for phone and tablet control, with slide previews, notes editing, annotation, text boxes, timer controls, navigation, pairing URLs, and QR codes.
- Local remote-control API and `dais remote ...` subcommands for scripts, Stream Decks, classroom automation, and other external controllers.
- Per-logical-slide target durations in `.dais` sidecars, with presenter timer color changes when a slide runs over.
- Markdown speaker notes from `--notes <path>`, with edits saved back to the Markdown file.
- Annotated export with saved ink annotations, Typst text boxes, whiteboard pages, SVG/PNG output, layer selection, and handout export.
- `.pdfpc` compatibility and a native `.dais` sidecar format.
- Fully remappable keybindings with presenter-console defaults.
- Single-binary distribution with no runtime dependencies or installers.

## Installation

Install from [crates.io](https://crates.io/crates/dais) with Cargo (requires Rust 1.92+):

```bash
cargo install dais
```

Or download a pre-built binary from [GitHub Releases](https://github.com/christopherkenny/dais/releases).

## Quick Start

```bash
dais presentation.pdf
```

## Usage

```
dais <file.pdf>                  # Present with auto-detected display mode
dais --single <file.pdf>         # Single-monitor mode (no audience window)
dais --screen-share <file.pdf>   # Screen-share mode (audience as normal window)
dais --edit <file.pdf>           # Open the slide grouping editor
dais --config <path> <file.pdf>  # Use a specific config file
dais --notes <path> <file.pdf>   # Use a Markdown speaker notes file
dais --portable <file.pdf>       # Skip OS user config for portable/USB use
dais --time-ignore <file.pdf>    # Do not update slide timing data on save
dais --test-input                # Diagnostic mode for clicker/remote setup
dais export <file.pdf> --out <file.pdf> [--handout]
                                  # Export a copy with annotations and text boxes applied
dais --remote <file.pdf>         # Start the local remote-control API and web remote
dais --remote-lan <file.pdf>     # Start the web remote for phone/tablet pairing
dais remote action next_slide    # Control a running presentation
```

### Display Modes

- **Dual** (default with 2+ monitors): Presenter console on primary, audience fullscreen on secondary. Press `F6` to swap the presenter and audience monitors for the current session.
- **Single** (`--single`): Single-window mode. Press `F5` to switch between the presenter console and the presentation HUD.
- **Screen-share** (`--screen-share`): Audience is a normal resizable window for Zoom/Teams sharing.
- **Remote** (`--remote`, `--remote-lan`): Local HTTP API, browser remote at `/remote`, presenter QR pairing, and `dais remote ...` commands for scripts, Stream Decks, phone/tablet controls, and other external adapters.

With one monitor, `dais` automatically falls back to single mode.

### Grouping Editor

For PDFs without embedded overlay metadata (e.g., PowerPoint exports), use the built-in editor:

```bash
dais --edit slides.pdf
```

Click between thumbnails to set group boundaries. Save writes the configured sidecar format. When loading, `dais` checks `.dais` before `.pdfpc`.

## Building from Source

Requires Rust 1.92+ (for the hayro PDF renderer).
For a local version, simply run:

```bash
cargo install --path crates/dais
```

To install from the git repository without cloning:

```bash
cargo install --git https://github.com/christopherkenny/dais.git dais --bin dais
```

The binary will be at `target/release/dais` (or `dais.exe` on Windows).

## Configuration

- **Windows:** `%APPDATA%\dais\config.toml`
- **macOS:** `~/Library/Application Support/dais/config.toml`
- **Linux:** `~/.config/dais/config.toml`

`dais` also reads a project-local `dais.toml` next to the PDF you open, and `--config <path>` can override both.
Use `--portable` to skip the OS user config layer while still reading project-local and explicit config files.

Use `--notes <path>` to keep speaker notes in a Markdown file that `dais` can load, edit, and save during the presentation.

See [docs/configuration.md](docs/configuration.md) for the full reference.
See [docs/notes.md](docs/notes.md) for Markdown speaker notes.
See [docs/export.md](docs/export.md) for annotated PDF, SVG, PNG, whiteboard, and handout export.

For display assignment, `audience_monitor` and `presenter_monitor` can be monitor names or simple display numbers like `"1"` and `"2"`.

## Keybindings

See [docs/keybindings.md](docs/keybindings.md) for the full reference.

## Clicker & Remote Support

See [docs/clicker-setup.md](docs/clicker-setup.md) for clicker profiles, custom mappings, and the `--test-input` diagnostic mode.
See [docs/remote.md](docs/remote.md) for the browser remote, REST API, CLI remote commands, LAN pairing, and external-controller examples.

## Architecture

`dais` is organized as an 8-crate Cargo workspace:

| Crate | Role |
|---|---|
| `dais` | Binary for CLI parsing and app launch |
| `dais-core` | Commands, state types, command bus, config, keybindings |
| `dais-engine` | Presentation engine that processes commands and owns state |
| `dais-document` | `DocumentSource` trait, hayro PDF renderer, and page cache |
| `dais-sidecar` | `.pdfpc` parser/writer, metadata extraction |
| `dais-platform` | Platform-specific monitor enumeration |
| `dais-remote` | HTTP remote-control server, browser remote, and REST API |
| `dais-ui` | egui UI for the presenter console, audience window, and grouping editor |

Key architectural decisions:

- **Command bus**: All user actions flow through a `Command` enum dispatched via `crossbeam-channel`. New input sources (REST API, remote control) just get another sender.
- **State broadcast**: The engine owns the authoritative `PresentationState`. UI reads via `Arc<RwLock<>>` and never mutates state directly.
- **`DocumentSource` trait**: PDF rendering is isolated behind a document-source abstraction. The default backend is `hayro`.
- **`SidecarFormat` trait**: Pluggable sidecar formats for `.pdfpc` compatibility and `dais`-native metadata.

## Contributing

```bash
# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Format
cargo fmt --all
```

CI runs on all three platforms (Windows, macOS, Linux) on every push and PR.

## Design Notes

The original project proposal is archived at [docs/design-proposal.md](docs/design-proposal.md).

## License

MIT. See [LICENSE](LICENSE).
