# dais-sidecar

`dais-sidecar` reads and writes presentation metadata stored next to a PDF.

It defines Dais metadata types, a sidecar format trait, the native `.dais`
format, and `.pdfpc` compatibility for notes and overlay grouping.

```rust
use dais_sidecar::format::SidecarFormat;
use dais_sidecar::pdfpc::PdfpcFormat;

let metadata = PdfpcFormat.read(std::path::Path::new("slides.pdfpc"))?;
# Ok::<(), dais_sidecar::format::SidecarError>(())
```
