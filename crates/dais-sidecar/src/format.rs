use std::path::Path;

use crate::types::PresentationMetadata;

/// Abstraction for sidecar file formats.
///
/// In v1, the only implementation is `.pdfpc`. Future formats (e.g., `.dais`)
/// implement this same trait and slot in without changing downstream code.
pub trait SidecarFormat {
    /// Read metadata from a sidecar file.
    fn read(&self, path: &Path) -> Result<PresentationMetadata, SidecarError>;

    /// Write metadata to a sidecar file.
    fn write(&self, path: &Path, metadata: &PresentationMetadata) -> Result<(), SidecarError>;

    /// The file extension this format uses (e.g., "pdfpc").
    fn file_extension(&self) -> &'static str;
}

/// Errors that can occur during sidecar operations.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("Missing required section: {0}")]
    MissingSection(String),
}
