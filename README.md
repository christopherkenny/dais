# Dais — A Native PDF Presenter Console

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

## Building from Source

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

See [docs/configuration.md](docs/configuration.md) for the full reference.

## Keybindings

See [docs/keybindings.md](docs/keybindings.md) for the full reference. All keybindings are remappable via config.

## Design Notes

The original project proposal is at [docs/design-proposal.md](docs/design-proposal.md).
