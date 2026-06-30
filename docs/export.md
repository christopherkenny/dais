# Export

Dais can export a copy of a presentation with Dais annotations and text boxes applied.
The original PDF is never modified.

```powershell
dais export slides.pdf --out slides-annotated.pdf
```

The exporter loads metadata using the same priority as presentation mode: `.dais` sidecar, `.pdfpc` sidecar, then embedded PDF metadata.
Rich slide annotations and text boxes are stored in `.dais` sidecars, so annotated export is most useful when a `.dais` file exists next to the PDF.

By default, export writes PDF.
Use `--format svg` or `--format png` to write one file per exported page into the output directory.

```powershell
dais export slides.pdf --format svg --out slides-svg
dais export slides.pdf --format png --out slides-png
```

Recurring export choices can be configured in `config.toml` or a project-local `dais.toml`.
Command-line flags override config values for a single export.

```toml
[export]
format = "pdf"
layers = "all"
handout = true
whiteboard = "append"
```

With those defaults, a regular annotated handout export can use the short command:

```powershell
dais export slides.pdf --out slides-annotated.pdf
```

Use `--layers` to control which content is included.

```powershell
dais export slides.pdf --out ink-only.pdf --layers ink
dais export slides.pdf --out overlays.pdf --layers overlays
dais export slides.pdf --out text-only.pdf --layers text
```

Available layers are `all`, `background`, `overlays`, `ink`, and `text`.
`all` is the default.

Use `--whiteboard append` to add the saved whiteboard as a final page.
Use `--whiteboard only` to export only the whiteboard.

```powershell
dais export slides.pdf --out with-whiteboard.pdf --whiteboard append
dais export slides.pdf --out whiteboard.pdf --whiteboard only
```

Use `--handout` to export one page per logical slide.
For slides with incremental build pages, Dais exports the final page of each slide group.
This does not arrange slides into grids or multiple slides per sheet.

```powershell
dais export slides.pdf --out handout.pdf --handout
```

Use `--no-handout` to export every PDF page when handout export is enabled in config.

PDF exports use the source PDF pages as Typst PDF image backgrounds.
Ink strokes and Typst text boxes are composed above each page at their saved positions.
