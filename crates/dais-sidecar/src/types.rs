use std::collections::HashMap;

/// Dais's internal presentation metadata — the authoritative representation.
///
/// This is not tied to any specific file format. `.pdfpc` compatibility and
/// Dais-native sidecar files are serializations of these shared types.
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
    /// Planned per-slide durations (logical slide index → target seconds).
    pub slide_target_durations: HashMap<usize, f64>,
    /// Per-page slide annotations (`page_index` → completed strokes).
    pub slide_annotations: HashMap<usize, Vec<InkStrokeMeta>>,
    /// Whiteboard annotations (not tied to any slide).
    pub whiteboard_annotations: Vec<InkStrokeMeta>,
    /// Per-page text box overlays (`page_index` → boxes).
    pub slide_text_boxes: HashMap<usize, Vec<TextBoxMeta>>,
}

/// A contiguous range of PDF pages forming one logical slide.
#[derive(Debug, Clone)]
pub struct SlideGroupMeta {
    /// First page index in this group (0-based, inclusive).
    pub start_page: usize,
    /// Last page index in this group (0-based, inclusive).
    pub end_page: usize,
}

/// A serializable ink stroke for sidecar persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct InkStrokeMeta {
    /// Points along the stroke (normalized 0..1 coordinates).
    pub points: Vec<(f32, f32)>,
    /// Stroke color as RGBA.
    pub color: [u8; 4],
    /// Stroke width in logical pixels.
    pub width: f32,
}

/// A serializable text box for sidecar persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBoxMeta {
    /// Unique identifier.
    pub id: u64,
    /// Normalized position and size: (x, y, w, h) in 0..1 coordinates.
    pub rect: (f32, f32, f32, f32),
    /// Text/Typst content.
    pub content: String,
    /// Font size in points.
    pub font_size: f32,
    /// Text color as RGBA.
    pub color: [u8; 4],
    /// Optional background fill as RGBA.
    pub background: Option<[u8; 4]>,
    /// Additional Typst setup inserted after Dais defaults and before content.
    pub typst_prelude: String,
}
