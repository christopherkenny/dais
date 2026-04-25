//! Hayro-backed PDF document source.
//!
//! Uses the pure-Rust hayro crate for CPU-based PDF rendering.
//! No system library dependencies required.

use crate::page::{PageDimensions, RenderSize, RenderedPage};
use crate::source::{DocumentError, DocumentSource, EmbeddedMetadata, OutlineEntry};

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{RenderCache, RenderSettings, render};

/// A PDF document backed by the hayro renderer.
pub struct HayroDocument {
    pdf: Pdf,
}

impl HayroDocument {
    /// Open a PDF file from a byte buffer.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, DocumentError> {
        let pdf = Pdf::new(data)
            .map_err(|e| DocumentError::Open(format!("Failed to parse PDF: {e:?}")))?;
        Ok(Self { pdf })
    }

    /// Open a PDF file from a path.
    pub fn open(path: &std::path::Path) -> Result<Self, DocumentError> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }
}

impl DocumentSource for HayroDocument {
    fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    fn page_dimensions(&self, page_index: usize) -> PageDimensions {
        let pages = self.pdf.pages();
        let page = &pages[page_index];
        let (w, h) = page.render_dimensions();
        PageDimensions { width_pts: w, height_pts: h }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn render_page(
        &self,
        page_index: usize,
        target_size: RenderSize,
    ) -> Result<RenderedPage, DocumentError> {
        let pages = self.pdf.pages();
        if page_index >= pages.len() {
            return Err(DocumentError::PageOutOfRange(page_index));
        }

        let page = &pages[page_index];
        let (page_w, page_h) = page.render_dimensions();

        // Target dimensions clamped to u16 range for hayro's RenderSettings
        let target_w = u16::try_from(target_size.width).unwrap_or(u16::MAX);
        let target_h = u16::try_from(target_size.height).unwrap_or(u16::MAX);

        // Preserve aspect ratio by fitting the page within the target box.
        // Independent x/y scaling distorts slides and makes previews look wrong.
        let x_scale = f32::from(target_w) / page_w;
        let y_scale = f32::from(target_h) / page_h;
        let scale = x_scale.min(y_scale);
        let render_w = (page_w * scale).round().clamp(1.0, f32::from(target_w)) as u16;
        let render_h = (page_h * scale).round().clamp(1.0, f32::from(target_h)) as u16;

        let cache = RenderCache::new();
        let interpreter_settings = InterpreterSettings::default();

        let render_settings = RenderSettings {
            x_scale: scale,
            y_scale: scale,
            width: Some(render_w),
            height: Some(render_h),
            ..Default::default()
        };

        let pixmap = render(page, &cache, &interpreter_settings, &render_settings);

        let width = pixmap.width();
        let height = pixmap.height();
        // data_as_u8_slice() returns premultiplied RGBA8 bytes. For opaque PDF content
        // (white background, no transparency), premultiplied == unpremultiplied.
        let data = pixmap.data_as_u8_slice().to_vec();

        Ok(RenderedPage { data, width: u32::from(width), height: u32::from(height) })
    }

    fn embedded_metadata(&self) -> Option<EmbeddedMetadata> {
        // hayro-syntax's Metadata struct exposes standard PDF info dict fields
        // (title, author, subject, keywords, creator, producer) but not custom
        // properties like pdfpc embeds. For now, we check if the subject or
        // keywords field contains pdfpc-formatted data as a heuristic.
        // Full XMP metadata extraction is a future enhancement.
        let metadata = self.pdf.metadata();
        let subject = metadata.subject.as_ref().and_then(|b| String::from_utf8(b.clone()).ok());

        // Check if subject contains pdfpc-style metadata
        if let Some(ref s) = subject
            && (s.contains("[notes]") || s.contains("[overlay]"))
        {
            return Some(EmbeddedMetadata { pdfpc_data: Some(s.clone()) });
        }

        None
    }

    fn outline(&self) -> Option<Vec<OutlineEntry>> {
        // Outline/bookmark extraction is not available in hayro-syntax's
        // public API in v0.6. Return None — non-critical for v1.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn test_pdf_path() -> std::path::PathBuf {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop(); // crates/
        path.pop(); // repo root
        path.push("tests");
        path.push("fixtures");
        path.push("test.pdf");
        path
    }

    #[test]
    fn load_pdf_and_query_page_count() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        assert_eq!(doc.page_count(), 1);
    }

    #[test]
    fn query_page_dimensions() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        let dims = doc.page_dimensions(0);
        // US Letter: 612 x 792 points
        assert!((dims.width_pts - 612.0).abs() < 1.0);
        assert!((dims.height_pts - 792.0).abs() < 1.0);
        assert!(dims.aspect_ratio() > 0.0);
    }

    #[test]
    fn render_page_to_rgba() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        let size = RenderSize { width: 800, height: 600 };
        let rendered = doc.render_page(0, size).expect("should render page");
        assert_eq!(rendered.width, 464);
        assert_eq!(rendered.height, 600);
        // RGBA: 4 bytes per pixel
        assert_eq!(rendered.data.len(), 464 * 600 * 4);
    }

    #[test]
    fn render_at_1080p_under_500ms() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        let size = RenderSize { width: 1920, height: 1080 };
        let start = Instant::now();
        let rendered = doc.render_page(0, size).expect("should render page");
        let elapsed = start.elapsed();
        assert_eq!(rendered.width, 835);
        assert_eq!(rendered.height, 1080);
        assert!(
            elapsed.as_millis() < 500,
            "Render took {}ms, expected <500ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn render_page_preserves_aspect_ratio() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        let rendered = doc
            .render_page(0, RenderSize { width: 1280, height: 720 })
            .expect("should render page");

        let aspect = f64::from(rendered.width) / f64::from(rendered.height);
        let expected = 612.0_f64 / 792.0_f64;
        assert!((aspect - expected).abs() < 0.01, "got aspect {aspect}, expected {expected}");
    }

    #[test]
    fn page_out_of_range_returns_error() {
        let doc = HayroDocument::open(&test_pdf_path()).expect("should load test PDF");
        let size = RenderSize { width: 100, height: 100 };
        assert!(doc.render_page(99, size).is_err());
    }

    #[test]
    fn from_bytes_works() {
        let data = std::fs::read(test_pdf_path()).expect("should read file");
        let doc = HayroDocument::from_bytes(data).expect("should load from bytes");
        assert_eq!(doc.page_count(), 1);
    }
}
