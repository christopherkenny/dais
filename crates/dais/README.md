# `dais`: A Native PDF Presenter Console

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

Install from crates.io with Cargo:

```bash
cargo install dais
```

`dais` requires Rust 1.92+ when installing from source.
Pre-built binaries are available from GitHub Releases.

## Quick Start

```bash
dais presentation.pdf
```

## Usage

```bash
dais <file.pdf>                  # Present with auto-detected display mode
dais --single <file.pdf>         # Single-monitor mode
dais --screen-share <file.pdf>   # Audience view as a resizable window
dais --edit <file.pdf>           # Open the slide grouping editor
dais --notes <path> <file.pdf>   # Use a Markdown speaker notes file
dais --portable <file.pdf>       # Skip OS user config for portable/USB use
dais --time-ignore <file.pdf>    # Do not update slide timing data on save
dais export <file.pdf> --out <file.pdf> [--handout]
dais --remote <file.pdf>         # Start the local remote API and web remote
dais --remote-lan <file.pdf>     # Start the web remote for phone/tablet pairing
dais remote action next_slide    # Control a running presentation
```

## Documentation

The full user documentation lives at <https://christophertkenny.com/dais/>.
The source repository is <https://github.com/christopherkenny/dais>.
