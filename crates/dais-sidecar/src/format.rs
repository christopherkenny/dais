use std::path::Path;

use crate::types::PresentationMetadata;

/// Abstraction for sidecar file formats.
///
/// Implementations read and write Dais presentation metadata using a concrete
/// file format such as `.pdfpc` or the Dais-native sidecar format.
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
    /// Filesystem read/write failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The sidecar file could not be parsed.
    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    /// A required sidecar section was absent.
    #[error("Missing required section: {0}")]
    MissingSection(String),
}
