use std::collections::HashMap;

/// Dais's internal presentation metadata — the authoritative representation.
///
/// This is NOT tied to any specific file format. `.pdfpc` is one serialization;
/// a future `.dais` format will be another. The engine and UI work with these
/// types exclusively.
#[derive(Debug, Clone, Default)]
pub struct PresentationMetadata {
    /// Presentation title, if known.
    pub title: Option<String>,
    /// Slide group definitions (page ranges).
    pub groups: Vec<SlideGroupMeta>,
    /// Per-page notes (`page_index` → markdown content).
    pub notes: HashMap<usize, String>,
    /// Optional "end" slide marker (page index after which slides are backup).
    pub end_slide: Option<usize>,
    /// Timer duration hint from sidecar, in minutes.
    pub last_minutes: Option<u32>,
    /// Per-slide timing data (logical slide index → seconds spent).
    pub slide_timings: HashMap<usize, f64>,
}

/// A contiguous range of PDF pages forming one logical slide.
#[derive(Debug, Clone)]
pub struct SlideGroupMeta {
    /// First page index in this group (0-based, inclusive).
    pub start_page: usize,
    /// Last page index in this group (0-based, inclusive).
    pub end_page: usize,
}
