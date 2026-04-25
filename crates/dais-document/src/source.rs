use crate::page::{PageDimensions, RenderSize, RenderedPage};

/// Abstraction over document sources.
///
/// In v1, the only implementation is a PDF loader (hayro or mupdf-rs).
/// This trait is the extension point for native Typst source support —
/// a live-compiled Typst document becomes another `DocumentSource` implementation.
pub trait DocumentSource: Send + Sync {
    /// Total number of pages in the document.
    fn page_count(&self) -> usize;

    /// Dimensions of a specific page (in points).
    fn page_dimensions(&self, page_index: usize) -> PageDimensions;

    /// Render a page to an RGBA bitmap at the specified target size.
    fn render_page(
        &self,
        page_index: usize,
        target_size: RenderSize,
    ) -> Result<RenderedPage, DocumentError>;

    /// Extract embedded metadata (pdfpc-compatible), if present.
    fn embedded_metadata(&self) -> Option<EmbeddedMetadata>;

    /// PDF outline/bookmark entries, if present.
    fn outline(&self) -> Option<Vec<OutlineEntry>>;
}

/// Metadata embedded in the PDF by `Polylux`/touying/pdfpc LaTeX package.
#[derive(Debug, Clone)]
pub struct EmbeddedMetadata {
    /// Raw pdfpc metadata string from the PDF info dictionary.
    pub pdfpc_data: Option<String>,
}

/// A bookmark/outline entry in the document.
#[derive(Debug, Clone)]
pub struct OutlineEntry {
    /// Bookmark title as displayed by the document.
    pub title: String,
    /// Destination page index, 0-based.
    pub page_index: usize,
    /// Nesting depth in the outline tree, starting at 0.
    pub level: usize,
}

/// Errors from document operations.
#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    /// The document could not be opened or parsed.
    #[error("Failed to open document: {0}")]
    Open(String),

    /// A page render failed after the document was opened successfully.
    #[error("Failed to render page {page_index}: {message}")]
    Render { page_index: usize, message: String },

    /// The requested page index is outside the document.
    #[error("Page index {0} out of range")]
    PageOutOfRange(usize),

    /// An underlying filesystem operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
