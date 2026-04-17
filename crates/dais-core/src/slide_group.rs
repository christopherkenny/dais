/// A logical slide group — one or more PDF pages representing build steps
/// of a single logical slide.
#[derive(Debug, Clone)]
pub struct SlideGroup {
    /// 0-based logical slide index.
    pub logical_index: usize,
    /// Raw PDF page indices belonging to this group (ordered).
    pub pages: Vec<usize>,
    /// Markdown notes for this logical slide, if any.
    pub notes: Option<String>,
}

/// How the grouping information was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingSource {
    /// Polylux/touying/pdfpc LaTeX package metadata embedded in the PDF.
    EmbeddedMetadata,
    /// Explicit grouping from a `.pdfpc` sidecar file.
    SidecarFile,
    /// User-set via the manual grouping editor.
    Manual,
    /// No grouping — 1:1 page-to-slide mapping.
    None,
}

/// Build a default 1:1 page-to-slide grouping for a document with `page_count` pages.
pub fn default_grouping(page_count: usize) -> Vec<SlideGroup> {
    (0..page_count).map(|i| SlideGroup { logical_index: i, pages: vec![i], notes: None }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grouping_creates_one_group_per_page() {
        let groups = default_grouping(5);
        assert_eq!(groups.len(), 5);
        for (i, group) in groups.iter().enumerate() {
            assert_eq!(group.logical_index, i);
            assert_eq!(group.pages, vec![i]);
            assert!(group.notes.is_none());
        }
    }

    #[test]
    fn default_grouping_empty_document() {
        let groups = default_grouping(0);
        assert!(groups.is_empty());
    }
}
