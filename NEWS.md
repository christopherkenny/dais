# dais 0.2.0

## New features

- Remote control can run alongside a presentation with `--remote` or `--remote-lan`.
- Built-in browser remote at `/remote` supports current and next slide previews, notes, timer controls, previous/next navigation, goto slide, blackout, freeze, whiteboard, and laser controls.
- Presenter console now shows remote pairing URLs, QR codes, pairing code, connected remote clients, and the last remote command.
- New `dais remote ...` subcommands can query state and send actions, goto, pointer, and timer commands to a running presentation.
- Remote settings can be configured under `[remote]` in `config.toml` or project-local `dais.toml`.
- `--time-ignore` can disable per-slide timing updates when saving sidecars.

## Bug fixes

- Remote LAN mode now accepts valid IP-literal hosts when bound to `0.0.0.0`.
- Custom remote tokens are restricted to URL-safe characters so browser pairing links work reliably.

# dais 0.1.1

## New features

- Text boxes can use a configured Typst prelude from `[text_boxes].typst_prelude` in `dais.toml`.

## Bug fixes

- Ctrl+L laser-style cycling now advances one style per keypress and reaches every pointer style.
- Normal forward/back navigation now steps through incremental build pages instead of skipping to the next logical slide.
- Text boxes and other slide overlays now stay aligned with the slide while zoom is active.

# dais 0.1.0

## New features

- Initial release of Dais, a native Rust PDF presenter console for PDF slides, such as those made with LaTeX/Beamer or Typst.
- Presenter console with current slide, next-slide preview, notes, timer, slide thumbnails, and audience display control.
- Dual-monitor, single-monitor, and screen-share display modes, with automatic monitor detection and graceful single-monitor fallback.
- Overlay/build-step grouping from Dais sidecars, `.pdfpc` sidecars, and embedded pdfpc metadata produced by tools such as Polylux, touying, and Beamer.
- Native `.dais` sidecar format for slide grouping, notes, ink annotations, whiteboard strokes, and text boxes, with `.pdfpc` read/write compatibility for notes and overlay grouping.
- Built-in slide grouping editor (`dais --edit <file.pdf>`) for PDFs without embedded overlay metadata.
- Presentation tools including configurable laser pointer styles, freehand ink, whiteboard, spotlight, zoom, freeze, and blackout.
- Typst-rendered text boxes that can be placed, edited, moved, resized, styled, and saved with the presentation.
- Markdown speaker notes with inline editing, adjustable font size, and sidecar persistence.
- Slide overview grid with keyboard navigation and direct slide jumping.
- Elapsed and countdown timers with warning and overrun phases.
- Fully remappable keyboard actions plus clicker/remote profiles and a `--test-input` diagnostic mode.
- Layered TOML configuration from platform config paths, project-local `dais.toml`, and explicit `--config` overrides.
- Single-binary distribution through Cargo or pre-built release artifacts.
