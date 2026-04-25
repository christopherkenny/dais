# Dais: A Native PDF Presenter Console <img src='assets/dais.png' align="right" height="150" />

[![CI](https://github.com/christopherkenny/dais/actions/workflows/ci.yml/badge.svg)](https://github.com/christopherkenny/dais/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Dais is a cross-platform PDF presentation console written in Rust for researchers and academics who build slides in LaTeX/Beamer, Typst, PowerPoint, or Keynote.
Dais is designed for straightforward installation, reliable operation in real presentation setups, and compatibility with existing slide workflows.

## Features

- Multi-monitor presenter view with an audience display and a presenter console with notes, timer, and navigation.
- Overlay and build-step support for `pdfpc` metadata, Beamer `\pdfpc`, and manual grouping.
- Presentation tools including a laser pointer, freehand ink, spotlight, and zoom.
- `.pdfpc` compatibility and a native `.dais` sidecar format.
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
- **Single** (`--single`): Single-window mode. Press `F5` to switch between the presenter console and the presentation HUD.
- **Screen-share** (`--screen-share`): Both windows visible; audience is a normal resizable window for Zoom/Teams sharing.

With one monitor, Dais automatically falls back to single mode.

### Grouping Editor

For PDFs without embedded overlay metadata (e.g., PowerPoint exports), use the built-in editor:

```bash
dais --edit slides.pdf
```

Click between thumbnails to set group boundaries. Save writes the configured sidecar format. When loading, Dais checks `.dais` before `.pdfpc`.

## Building from Source

Requires Rust 1.92+ (for the hayro PDF renderer).

```bash
git clone https://github.com/christopherkenny/dais.git
cd dais
cargo build --release
```

The binary will be at `target/release/dais` (or `dais.exe` on Windows).

## Configuration

- **Windows:** `%APPDATA%\dais\config.toml`
- **macOS:** `~/Library/Application Support/dais/config.toml`
- **Linux:** `~/.config/dais/config.toml`

Dais also reads a project-local `dais.toml` next to the PDF you open, and `--config <path>`
can override both.

See [docs/configuration.md](docs/configuration.md) for the full reference.

For display assignment, `audience_monitor` can be a monitor name or a simple display number like `"2"`.

## Keybindings

See [docs/keybindings.md](docs/keybindings.md) for the full reference.

## Clicker & Remote Support

See [docs/clicker-setup.md](docs/clicker-setup.md) for clicker profiles, custom mappings, and the `--test-input` diagnostic mode.

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

The original project proposal is archived at [docs/design-proposal.md](docs/design-proposal.md).

## License

MIT. See [LICENSE](LICENSE).
