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

[Projector](https://github.com/christopherkenny/projector) converts Quarto documents to Polylux syntax. Because projector outputs Polylux, the compiled PDF contains full pdfpc-compatible metadata. Identical experience to Typst + Polylux.

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

This one addition gives Quarto Beamer users automatic notes and overlay support in Dais.

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
