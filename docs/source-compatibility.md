# Source Compatibility Guide

Dais works with any PDF, but gets the best experience when overlay grouping and notes metadata are available. Here's how to get the most out of each authoring tool.

## Typst + Polylux/touying (Recommended)

**Best experience — everything automatic.**

[Polylux](https://github.com/andreasabel/polylux) and [touying](https://github.com/touying-typst/touying) embed pdfpc-compatible metadata directly into the compiled PDF. Dais reads this automatically — correct overlay grouping, slide notes, and slide counting with zero configuration.

```bash
typst compile slides.typ
dais slides.pdf
```

## Quarto + projector

[Projector](https://github.com/christopherkenny/projector) converts Quarto documents to Polylux syntax. That makes it a natural authoring path for Dais, but Dais should not assume metadata is embedded in the PDF automatically. If your workflow emits a sidecar file, keep it next to the PDF so Dais can load it.

```bash
quarto render slides.qmd
dais slides.pdf
```

## Beamer + `\pdfpc` LaTeX Package

Add one line to your Beamer preamble to embed overlay and notes metadata in the PDF:

```latex
\usepackage{pdfpc}
```

This gives Dais automatic overlay grouping and notes. No sidecar file needed.

For notes, use `\pdfpcnote{Your note text}` in your slides:

```latex
\begin{frame}{My Slide}
  Content here.
  \pdfpcnote{Remember to mention the key finding.}
\end{frame}
```

## Quarto + Beamer

Add this to your Quarto YAML front matter to route notes through the pdfpc package:

```yaml
format:
  beamer:
    include-in-header:
      text: |
        \usepackage[overridenote]{pdfpc}
```

Speaker notes work with Quarto's native `::: {.notes}` syntax:

```markdown
## My Slide

Content here.

::: {.notes}
Remember to mention the key finding.
:::
```

The `overridenote` option intercepts Beamer's `\note` command so that Quarto's notes syntax flows through to pdfpc metadata automatically.

### Troubleshooting Quarto Workflows

- **Notes not appearing?** Verify `\usepackage[overridenote]{pdfpc}` is in the header. Without `overridenote`, Beamer's `\note` is consumed and not written to PDF metadata.
- **Overlay grouping missing?** Quarto Beamer uses `\pause` which the `\pdfpc` package tracks. If your deck doesn't use pauses, grouping is 1:1 (one page = one slide).
- **Quarto + Typst path**: If using Quarto with Typst via projector, verify whether your workflow emits a sidecar file or embeds metadata directly. Dais can load sidecars, but this path should not be treated as automatic embedded metadata by default.

## Beamer without `\pdfpc`

If you can't add the `\pdfpc` package, use the built-in grouping editor:

```bash
dais --edit slides.pdf
```

Set page group boundaries manually, then save. Dais creates a `.pdfpc` sidecar file next to your PDF. On subsequent runs, it loads automatically.

## PowerPoint PDF Export

PowerPoint animations expand to separate PDF pages. Use the grouping editor to define slide boundaries:

```bash
dais --edit slides.pdf
```

## Keynote PDF Export

Keynote PDF exports typically have one page per slide with no animation expansion. Dais works out of the box:

```bash
dais slides.pdf
```

## Other PDF Sources

Any PDF works. Without grouping metadata, Dais treats each page as one slide. If you need grouping, use the editor.
