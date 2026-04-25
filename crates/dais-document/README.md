# dais-document

`dais-document` provides document loading and rendering primitives for Dais.

The crate defines the `DocumentSource` abstraction, rendered page types, page
cache, render pipeline, and the default Hayro-backed PDF implementation.

```rust
use dais_document::page::RenderSize;
use dais_document::pdf_hayro::HayroDocument;
use dais_document::source::DocumentSource;

let doc = HayroDocument::open(std::path::Path::new("slides.pdf"))?;
let page = doc.render_page(0, RenderSize { width: 1280, height: 720 })?;
# Ok::<(), dais_document::source::DocumentError>(())
```

The `mupdf` feature is reserved for a future backend and is not implemented yet.
