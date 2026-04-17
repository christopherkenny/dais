# Dais: A Native PDF Presenter Console

[![CI](https://github.com/christopherkenny/dais/actions/workflows/ci.yml/badge.svg)](https://github.com/christopherkenny/dais/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Dais is a cross-platform PDF presentation console written in Rust for researchers and academics who build slides in LaTeX/Beamer, Typst, PowerPoint, or Keynote.
Dais is designed for straightforward installation, reliable operation in real presentation setups, and compatibility with existing slide workflows.

## Features

- Multi-monitor presenter view with an audience display and a presenter console with notes, timer, and navigation.
- Overlay and build-step support for Polylux, touying, Beamer `\pdfpc` metadata, and manual grouping.
- Presentation tools including a laser pointer, freehand ink, spotlight, and zoom.
- `.pdfpc` sidecar compatibility for existing notes and grouping metadata.
- Fully remappable keybindings with presenter-console defaults.
- Single-binary distribution with no runtime dependencies or installers.

## Quick Start

```bash
# Download the latest release for your platform from GitHub Releases, then run:
dais presentation.pdf
```

## Usage

```
dais <file.pdf>                  # Present with auto-detected display mode
dais --single <file.pdf>         # Single-monitor mode (no audience window)
dais --screen-share <file.pdf>   # Screen-share mode (audience as normal window)
dais --edit <file.pdf>           # Open the slide grouping editor
```

### Display Modes

- **Dual** (default with 2+ monitors): Presenter console on primary, audience fullscreen on secondary.
- **Single** (`--single`): Presenter-only view with no audience window.
- **Screen-share** (`--screen-share`): Both windows visible; audience is a normal resizable window for Zoom/Teams sharing.

With one monitor, Dais automatically falls back to screen-share mode.

### Grouping Editor

For PDFs without embedded overlay metadata (e.g., PowerPoint exports), use the built-in editor:

```bash
dais --edit slides.pdf
```

Click between thumbnails to set group boundaries. Save writes a `.pdfpc` sidecar file that Dais loads automatically on future runs.

## Building from Source

Requires Rust 1.92+ (for the hayro PDF renderer).

```bash
git clone https://github.com/christopherkenny/dais.git
cd dais
cargo build --release
```

The binary will be at `target/release/dais` (or `dais.exe` on Windows).

## Source Compatibility

| Source | Overlay grouping | Notes |
|---|---|---|
| Typst + Polylux/touying | Automatic | Recommended workflow. |
| Quarto + projector | Automatic | Outputs Polylux. |
| Beamer + `\pdfpc` package | Automatic | One-line preamble addition. |
| Quarto + Beamer + pdfpc header | Automatic | One-line YAML addition. |
| Beamer without `\pdfpc` | Manual sidecar | Built-in editor. |
| PowerPoint PDF export | Manual sidecar | Animations expand to separate pages. |
| Keynote PDF export | Automatic | No animations in export. |

## Configuration

- **Windows:** `%APPDATA%\dais\config.toml`
- **macOS:** `~/Library/Application Support/dais/config.toml`
- **Linux:** `~/.config/dais/config.toml`

Dais also reads a project-local `dais.toml` next to the PDF you open, and `--config <path>`
can override both.

See [docs/configuration.md](docs/configuration.md) for the full reference.

For display assignment, `audience_monitor` can be a monitor name or a simple display number like `"2"`.

## Keybindings

See [docs/keybindings.md](docs/keybindings.md) for the full reference. All keybindings are remappable via config.

## Architecture

Dais is organized as a 7-crate Cargo workspace:

| Crate | Role |
|---|---|
| `dais` | Binary for CLI parsing and app launch |
| `dais-core` | Commands, state types, command bus, config, keybindings |
| `dais-engine` | Presentation engine that processes commands and owns state |
| `dais-document` | `DocumentSource` trait, hayro PDF renderer, and page cache |
| `dais-sidecar` | `.pdfpc` parser/writer, metadata extraction |
| `dais-platform` | Platform-specific monitor enumeration |
| `dais-ui` | egui UI for the presenter console, audience window, and grouping editor |

Key architectural decisions:

- **Command bus**: All user actions flow through a `Command` enum dispatched via `crossbeam-channel`. New input sources (REST API, remote control) just get another sender.
- **State broadcast**: The engine owns the authoritative `PresentationState`. UI reads via `Arc<RwLock<>>` and never mutates state directly.
- **`DocumentSource` trait**: Feature-flagged PDF backends. `hayro` (pure Rust) is default; `mupdf` is a future fallback.
- **`SidecarFormat` trait**: Pluggable sidecar formats with `.pdfpc` today and `.dais` in the future.

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

The original project proposal is at [docs/design-proposal.md](docs/design-proposal.md).

## License

MIT. See [LICENSE](LICENSE).
