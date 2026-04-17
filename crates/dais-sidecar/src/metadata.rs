use crate::types::{PresentationMetadata, SlideGroupMeta};

/// Attempt to extract pdfpc-compatible metadata from a raw pdfpc-format string
/// embedded in a PDF's info dictionary (typically the "pdfpc" or "pdfpcFormat" key).
///
/// Polylux, touying, and the `\pdfpc` LaTeX package embed metadata directly
/// into the compiled PDF (typically in the Info dictionary or XMP stream).
///
/// Returns `None` if the input is empty or cannot be parsed.
pub fn extract_embedded_metadata(raw_pdfpc_data: Option<&str>) -> Option<PresentationMetadata> {
    let data = raw_pdfpc_data?.trim();
    if data.is_empty() {
        return None;
    }

    // The embedded format is the same INI-like pdfpc format
    let meta = crate::pdfpc::parser::parse_pdfpc_str(data);

    // Only return if we actually extracted something useful
    if meta.groups.is_empty() && meta.notes.is_empty() && meta.end_slide.is_none() {
        return None;
    }

    Some(meta)
}

/// Load presentation metadata using the priority chain:
///
/// 1. Embedded PDF metadata (highest priority)
/// 2. `.pdfpc` sidecar file (if found next to PDF)
/// 3. No metadata (empty default)
///
/// The `pdf_path` is the path to the PDF file — the sidecar is looked up
/// by replacing the extension with `.pdfpc`.
pub fn load_metadata(
    pdf_path: &std::path::Path,
    embedded_pdfpc_data: Option<&str>,
) -> (PresentationMetadata, MetadataSource) {
    // Priority 1: Embedded PDF metadata
    if let Some(meta) = extract_embedded_metadata(embedded_pdfpc_data) {
        return (meta, MetadataSource::Embedded);
    }

    // Priority 2: .pdfpc sidecar file
    let sidecar_path = pdf_path.with_extension("pdfpc");
    if sidecar_path.exists() {
        use crate::format::SidecarFormat;
        let format = crate::pdfpc::PdfpcFormat;
        if let Ok(meta) = format.read(&sidecar_path) {
            return (meta, MetadataSource::Sidecar(sidecar_path));
        }
        tracing::warn!("Failed to parse sidecar file: {}", sidecar_path.display());
    }

    // Priority 3: No metadata
    (PresentationMetadata::default(), MetadataSource::None)
}

/// Where the metadata was loaded from.
#[derive(Debug, Clone)]
pub enum MetadataSource {
    /// Extracted from PDF info dictionary / XMP.
    Embedded,
    /// Loaded from a sidecar file.
    Sidecar(std::path::PathBuf),
    /// No metadata found — using 1:1 page-to-slide mapping.
    None,
}

/// Parse overlay group definitions from a pdfpc-style overlay string.
///
/// Each line: `start_page end_page` (1-based).
pub fn parse_overlay_groups(overlay_str: &str) -> Vec<SlideGroupMeta> {
    let mut groups = Vec::new();
    for line in overlay_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2
            && let (Ok(start), Ok(end)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>())
        {
            groups.push(SlideGroupMeta {
                start_page: start.saturating_sub(1),
                end_page: end.saturating_sub(1),
            });
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_none_from_empty() {
        assert!(extract_embedded_metadata(None).is_none());
        assert!(extract_embedded_metadata(Some("")).is_none());
        assert!(extract_embedded_metadata(Some("  ")).is_none());
    }

    #[test]
    fn extract_from_embedded_pdfpc_string() {
        let data = "[notes]\n### 1\nHello\n[overlay]\n1 3\n";
        let meta = extract_embedded_metadata(Some(data)).unwrap();
        assert_eq!(meta.notes.len(), 1);
        assert_eq!(meta.notes[&0], "Hello");
        assert_eq!(meta.groups.len(), 1);
    }

    #[test]
    fn extract_returns_none_if_no_useful_content() {
        let data = "[file]\ntest.pdf\n";
        assert!(extract_embedded_metadata(Some(data)).is_none());
    }

    #[test]
    fn load_metadata_with_no_sources() {
        let (meta, source) = load_metadata(std::path::Path::new("nonexistent.pdf"), None);
        assert!(meta.groups.is_empty());
        assert!(matches!(source, MetadataSource::None));
    }

    #[test]
    fn load_metadata_embedded_takes_priority() {
        let data = "[overlay]\n1 3\n";
        let (meta, source) = load_metadata(std::path::Path::new("nonexistent.pdf"), Some(data));
        assert_eq!(meta.groups.len(), 1);
        assert!(matches!(source, MetadataSource::Embedded));
    }

    #[test]
    fn parse_overlay_groups_basic() {
        let groups = parse_overlay_groups("1 3\n4 5\n");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].start_page, 0);
        assert_eq!(groups[0].end_page, 2);
        assert_eq!(groups[1].start_page, 3);
        assert_eq!(groups[1].end_page, 4);
    }
}
