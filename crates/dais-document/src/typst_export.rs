//! Typst-based annotated slide export.
//!
//! This module composes source PDF pages, ink annotations, whiteboard ink, and
//! Typst text boxes into a new Typst document and exports it to PDF, SVG, or PNG.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;

use dais_sidecar::types::{InkStrokeMeta, PresentationMetadata, TextBoxMeta};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use thiserror::Error;
use typst::Library;
use typst::LibraryExt;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::{FontSearcher, Fonts};

use crate::pdf_hayro::HayroDocument;
use crate::source::DocumentSource;

const MAIN_TYP: &str = "main.typ";
const SOURCE_PDF: &str = "source.pdf";
const PNG_PIXELS_PER_PT: f32 = 2.0;

static FONTS: LazyLock<Fonts> = LazyLock::new(|| FontSearcher::new().search());

/// Output format for annotated exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Svg,
    Png,
}

/// Content layers to include in the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportLayers {
    /// Source PDF pages only.
    Background,
    /// Slide ink only.
    Ink,
    /// Text boxes only.
    Text,
    /// Slide ink and text boxes without source PDF pages.
    Overlays,
    /// Source PDF pages, slide ink, and text boxes.
    All,
}

impl ExportLayers {
    fn background(self) -> bool {
        matches!(self, Self::Background | Self::All)
    }

    fn ink(self) -> bool {
        matches!(self, Self::Ink | Self::Overlays | Self::All)
    }

    fn text(self) -> bool {
        matches!(self, Self::Text | Self::Overlays | Self::All)
    }
}

/// Whiteboard export behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteboardExport {
    None,
    Append,
    Only,
}

/// Inputs for annotated export.
#[derive(Debug, Clone, Copy)]
pub struct AnnotatedExport<'a> {
    /// The source PDF path.
    pub pdf_path: &'a Path,
    /// The metadata containing annotations and text boxes.
    pub metadata: &'a PresentationMetadata,
    /// Output format.
    pub format: ExportFormat,
    /// Content layers to include.
    pub layers: ExportLayers,
    /// Collapse incremental/build pages to one page per logical slide.
    pub handout: bool,
    /// Whiteboard export behavior.
    pub whiteboard: WhiteboardExport,
}

/// A rendered export artifact.
#[derive(Debug, Clone)]
pub struct ExportArtifact {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Errors produced while composing or exporting annotated slides.
#[derive(Debug, Error)]
pub enum TypstExportError {
    /// The input PDF could not be read.
    #[error("failed to read source PDF: {0}")]
    ReadPdf(#[from] std::io::Error),
    /// The source PDF could not be opened for page geometry.
    #[error("failed to open source PDF: {0}")]
    OpenPdf(#[from] crate::source::DocumentError),
    /// Typst compilation failed.
    #[error("Typst compilation failed: {0}")]
    Compile(String),
    /// Typst PDF serialization failed.
    #[error("Typst PDF export failed: {0}")]
    Pdf(String),
    /// PNG serialization failed.
    #[error("PNG export failed: {0}")]
    Png(String),
}

/// Export annotated slides.
pub fn export_annotated(
    request: AnnotatedExport<'_>,
) -> Result<Vec<ExportArtifact>, TypstExportError> {
    let pdf_bytes = std::fs::read(request.pdf_path)?;
    let doc = HayroDocument::open(request.pdf_path)?;
    export_annotated_from_bytes(&doc, pdf_bytes, request)
}

/// Export annotated slides from an already-open document and source PDF bytes.
pub fn export_annotated_from_bytes(
    doc: &dyn DocumentSource,
    pdf_bytes: Vec<u8>,
    request: AnnotatedExport<'_>,
) -> Result<Vec<ExportArtifact>, TypstExportError> {
    let source = build_annotated_typst_source(
        doc,
        request.metadata,
        request.layers,
        request.handout,
        request.whiteboard,
    );
    let document = compile_typst_document(source, pdf_bytes)?;
    match request.format {
        ExportFormat::Pdf => Ok(vec![ExportArtifact {
            name: "export.pdf".to_string(),
            bytes: export_pdf(&document)?,
        }]),
        ExportFormat::Svg => Ok(export_svg(&document)),
        ExportFormat::Png => export_png(&document),
    }
}

fn compile_typst_document(
    source: String,
    pdf_bytes: Vec<u8>,
) -> Result<PagedDocument, TypstExportError> {
    let world = ExportWorld::new(source, pdf_bytes);
    let result = typst::compile::<PagedDocument>(&world);
    result.output.map_err(|errors| TypstExportError::Compile(format!("{errors:?}")))
}

fn export_pdf(document: &PagedDocument) -> Result<Vec<u8>, TypstExportError> {
    typst_pdf::pdf(document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| TypstExportError::Pdf(format!("{errors:?}")))
}

fn export_svg(document: &PagedDocument) -> Vec<ExportArtifact> {
    document
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| ExportArtifact {
            name: numbered_name(index, "svg"),
            bytes: typst_svg::svg(page).into_bytes(),
        })
        .collect()
}

fn export_png(document: &PagedDocument) -> Result<Vec<ExportArtifact>, TypstExportError> {
    document
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let pixmap = typst_render::render(page, PNG_PIXELS_PER_PT);
            let mut bytes = Vec::new();
            PngEncoder::new(&mut bytes)
                .write_image(
                    pixmap.data(),
                    pixmap.width(),
                    pixmap.height(),
                    ColorType::Rgba8.into(),
                )
                .map_err(|err| TypstExportError::Png(err.to_string()))?;
            Ok(ExportArtifact { name: numbered_name(index, "png"), bytes })
        })
        .collect()
}

fn numbered_name(index: usize, extension: &str) -> String {
    format!("page-{:03}.{extension}", index + 1)
}

struct ExportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    main_id: FileId,
    pdf_id: FileId,
    source: Source,
    pdf_bytes: Bytes,
}

impl ExportWorld {
    fn new(markup: String, pdf_bytes: Vec<u8>) -> Self {
        let main_id = FileId::new(None, VirtualPath::new(MAIN_TYP));
        let pdf_id = FileId::new(None, VirtualPath::new(SOURCE_PDF));
        let source = Source::new(main_id, markup);
        let fonts = &*FONTS;
        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(fonts.book.clone()),
            main_id,
            pdf_id,
            source,
            pdf_bytes: Bytes::new(pdf_bytes),
        }
    }
}

impl typst_library::World for ExportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.pdf_id {
            Ok(self.pdf_bytes.clone())
        } else {
            Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

fn build_annotated_typst_source(
    doc: &dyn DocumentSource,
    metadata: &PresentationMetadata,
    layers: ExportLayers,
    handout: bool,
    whiteboard: WhiteboardExport,
) -> String {
    let items = export_items(doc, metadata, handout, whiteboard);
    let mut out = String::from("#set page(margin: 0pt)\n#set par(leading: 0pt)\n");

    for item in &items {
        write_page_source(&mut out, item, layers);
    }
    out
}

fn export_items<'a>(
    doc: &dyn DocumentSource,
    metadata: &'a PresentationMetadata,
    handout: bool,
    whiteboard: WhiteboardExport,
) -> Vec<ExportItem<'a>> {
    let mut items = Vec::new();
    if !matches!(whiteboard, WhiteboardExport::Only) {
        for page_index in selected_page_indices(doc.page_count(), metadata, handout) {
            let dims = doc.page_dimensions(page_index);
            items.push(ExportItem {
                width_pts: dims.width_pts,
                height_pts: dims.height_pts,
                content: ExportItemContent::Slide {
                    page_index,
                    strokes: metadata.slide_annotations.get(&page_index).map_or(&[], Vec::as_slice),
                    text_boxes: metadata
                        .slide_text_boxes
                        .get(&page_index)
                        .map_or(&[], Vec::as_slice),
                },
            });
        }
    }

    if matches!(whiteboard, WhiteboardExport::Append | WhiteboardExport::Only) {
        let dims = if doc.page_count() > 0 {
            doc.page_dimensions(0)
        } else {
            crate::page::PageDimensions { width_pts: 960.0, height_pts: 540.0 }
        };
        items.push(ExportItem {
            width_pts: dims.width_pts,
            height_pts: dims.height_pts,
            content: ExportItemContent::Whiteboard { strokes: &metadata.whiteboard_annotations },
        });
    }
    items
}

fn selected_page_indices(
    page_count: usize,
    metadata: &PresentationMetadata,
    handout: bool,
) -> Vec<usize> {
    if !handout {
        return (0..page_count).collect();
    }

    if metadata.groups.is_empty() {
        return (0..page_count).collect();
    }

    metadata
        .groups
        .iter()
        .filter_map(|group| (group.end_page < page_count).then_some(group.end_page))
        .collect()
}

struct ExportItem<'a> {
    width_pts: f32,
    height_pts: f32,
    content: ExportItemContent<'a>,
}

enum ExportItemContent<'a> {
    Slide { page_index: usize, strokes: &'a [InkStrokeMeta], text_boxes: &'a [TextBoxMeta] },
    Whiteboard { strokes: &'a [InkStrokeMeta] },
}

fn write_page_source(out: &mut String, item: &ExportItem<'_>, layers: ExportLayers) {
    writeln!(
        out,
        "#page(width: {}pt, height: {}pt, margin: 0pt, fill: white)[",
        fmt(item.width_pts),
        fmt(item.height_pts)
    )
    .expect("writing to String cannot fail");
    write_item_source(out, item, layers);
    out.push_str("]\n");
}

fn write_item_source(out: &mut String, item: &ExportItem<'_>, layers: ExportLayers) {
    match &item.content {
        ExportItemContent::Slide { page_index, strokes, text_boxes } => {
            if layers.background() {
                write_pdf_background_source(out, *page_index, item.width_pts, item.height_pts);
            }
            if layers.ink() {
                for stroke in *strokes {
                    write_stroke_source(out, stroke, item.width_pts, item.height_pts);
                }
            }
            if layers.text() {
                for text_box in *text_boxes {
                    write_text_box_source(out, text_box, item.width_pts, item.height_pts);
                }
            }
        }
        ExportItemContent::Whiteboard { strokes } => {
            writeln!(
                out,
                "#place(dx: 0pt, dy: 0pt, rect(width: {}pt, height: {}pt, fill: white))",
                fmt(item.width_pts),
                fmt(item.height_pts)
            )
            .expect("writing to String cannot fail");
            if layers.ink() || matches!(layers, ExportLayers::All | ExportLayers::Background) {
                for stroke in *strokes {
                    write_stroke_source(out, stroke, item.width_pts, item.height_pts);
                }
            }
        }
    }
}

fn write_pdf_background_source(
    out: &mut String,
    page_index: usize,
    width_pts: f32,
    height_pts: f32,
) {
    writeln!(
        out,
        "#place(dx: 0pt, dy: 0pt, image({}, format: \"pdf\", page: {}, width: {}pt, height: {}pt, fit: \"stretch\"))",
        typst_string(SOURCE_PDF),
        page_index + 1,
        fmt(width_pts),
        fmt(height_pts)
    )
    .expect("writing to String cannot fail");
}

fn write_stroke_source(out: &mut String, stroke: &InkStrokeMeta, width_pts: f32, height_pts: f32) {
    let color = typst_rgba(stroke.color);
    let thickness = fmt(stroke.width);
    if stroke.points.len() >= 2 {
        write!(
            out,
            "#curve(stroke: (paint: {color}, thickness: {thickness}pt, cap: \"round\", join: \"round\")"
        )
        .expect("writing to String cannot fail");
        let (x, y) = denormalize(stroke.points[0], width_pts, height_pts);
        write!(out, ", curve.move(({}pt, {}pt))", fmt(x), fmt(y))
            .expect("writing to String cannot fail");
        for &point in &stroke.points[1..] {
            let (x, y) = denormalize(point, width_pts, height_pts);
            write!(out, ", curve.line(({}pt, {}pt))", fmt(x), fmt(y))
                .expect("writing to String cannot fail");
        }
        out.push_str(")\n");
    }

    let radius = stroke.width * 0.5;
    for &point in &stroke.points {
        let (x, y) = denormalize(point, width_pts, height_pts);
        writeln!(
            out,
            "#place(dx: {}pt, dy: {}pt, circle(radius: {}pt, fill: {color}))",
            fmt(x - radius),
            fmt(y - radius),
            fmt(radius)
        )
        .expect("writing to String cannot fail");
    }
}

fn write_text_box_source(
    out: &mut String,
    text_box: &TextBoxMeta,
    width_pts: f32,
    height_pts: f32,
) {
    let (x, y, w, h) = text_box.rect;
    let x = x * width_pts;
    let y = y * height_pts;
    let w = w * width_pts;
    let h = h * height_pts;
    let fill = text_box.background.map_or_else(|| "none".to_string(), typst_rgba);
    let text_color = typst_rgba(text_box.color);

    writeln!(
        out,
        "#place(dx: {}pt, dy: {}pt, block(width: {}pt, height: {}pt, fill: {fill}, inset: 0pt)[",
        fmt(x),
        fmt(y),
        fmt(w),
        fmt(h)
    )
    .expect("writing to String cannot fail");
    writeln!(out, "#set text(size: {}pt, fill: {text_color})", fmt(text_box.font_size))
        .expect("writing to String cannot fail");
    if !text_box.typst_prelude.trim().is_empty() {
        out.push_str(&text_box.typst_prelude);
        out.push('\n');
    }
    out.push_str(&text_box.content);
    out.push_str("\n])\n");
}

fn denormalize(point: (f32, f32), width_pts: f32, height_pts: f32) -> (f32, f32) {
    (point.0 * width_pts, point.1 * height_pts)
}

fn typst_rgba([r, g, b, a]: [u8; 4]) -> String {
    format!("rgb({r}, {g}, {b}, {a})")
}

fn typst_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn fmt(value: f32) -> String {
    let mut s = format!("{value:.4}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" { "0".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::page::{PageDimensions, RenderSize, RenderedPage};
    use crate::source::{DocumentError, EmbeddedMetadata, OutlineEntry};

    struct StubDocument {
        pages: Vec<PageDimensions>,
    }

    impl DocumentSource for StubDocument {
        fn page_count(&self) -> usize {
            self.pages.len()
        }

        fn page_dimensions(&self, page_index: usize) -> PageDimensions {
            self.pages[page_index]
        }

        fn render_page(
            &self,
            _page_index: usize,
            _target_size: RenderSize,
        ) -> Result<RenderedPage, DocumentError> {
            unimplemented!("export tests do not render pages")
        }

        fn embedded_metadata(&self) -> Option<EmbeddedMetadata> {
            None
        }

        fn outline(&self) -> Option<Vec<OutlineEntry>> {
            None
        }
    }

    fn one_page_doc() -> StubDocument {
        StubDocument { pages: vec![PageDimensions { width_pts: 200.0, height_pts: 100.0 }] }
    }

    fn request(metadata: &PresentationMetadata, format: ExportFormat) -> AnnotatedExport<'_> {
        AnnotatedExport {
            pdf_path: Path::new("slides.pdf"),
            metadata,
            format,
            layers: ExportLayers::All,
            handout: false,
            whiteboard: WhiteboardExport::None,
        }
    }

    #[test]
    fn basic_typst_export_produces_pdf_bytes() {
        let document = compile_typst_document(
            "#set page(width: 100pt, height: 50pt, margin: 0pt)\nHello".to_string(),
            Vec::new(),
        )
        .expect("basic Typst document should compile");
        let pdf = export_pdf(&document).expect("basic Typst document should export");

        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn ink_source_maps_points_and_preserves_style() {
        let mut annotations = HashMap::new();
        annotations.insert(
            0,
            vec![InkStrokeMeta {
                points: vec![(0.0, 0.0), (1.0, 1.0)],
                color: [255, 128, 0, 77],
                width: 3.5,
            }],
        );
        let meta = PresentationMetadata { slide_annotations: annotations, ..Default::default() };

        let source = build_annotated_typst_source(
            &one_page_doc(),
            &meta,
            ExportLayers::All,
            false,
            WhiteboardExport::None,
        );

        assert!(source.contains("rgb(255, 128, 0, 77)"));
        assert!(source.contains("thickness: 3.5pt"));
        assert!(source.contains("curve.move((0pt, 0pt))"));
        assert!(source.contains("curve.line((200pt, 100pt))"));
        assert!(source.contains("circle(radius: 1.75pt"));
    }

    #[test]
    fn single_point_stroke_emits_dot_without_curve() {
        let mut annotations = HashMap::new();
        annotations.insert(
            0,
            vec![InkStrokeMeta { points: vec![(0.5, 0.5)], color: [0, 0, 255, 255], width: 4.0 }],
        );
        let meta = PresentationMetadata { slide_annotations: annotations, ..Default::default() };

        let source = build_annotated_typst_source(
            &one_page_doc(),
            &meta,
            ExportLayers::All,
            false,
            WhiteboardExport::None,
        );

        assert!(!source.contains("#curve("));
        assert!(source.contains("#place(dx: 98pt, dy: 48pt, circle(radius: 2pt"));
    }

    #[test]
    fn text_box_source_maps_rect_and_preserves_typst_content() {
        let mut boxes = HashMap::new();
        boxes.insert(
            0,
            vec![TextBoxMeta {
                id: 1,
                rect: (0.1, 0.2, 0.3, 0.4),
                content: "$pi r^2$".to_string(),
                font_size: 18.0,
                color: [1, 2, 3, 255],
                background: Some([240, 241, 242, 128]),
                typst_prelude: "#set align(horizon)".to_string(),
            }],
        );
        let meta = PresentationMetadata { slide_text_boxes: boxes, ..Default::default() };

        let source = build_annotated_typst_source(
            &one_page_doc(),
            &meta,
            ExportLayers::All,
            false,
            WhiteboardExport::None,
        );

        assert!(source.contains("#place(dx: 20pt, dy: 20pt, block(width: 60pt, height: 40pt"));
        assert!(source.contains("fill: rgb(240, 241, 242, 128)"));
        assert!(source.contains("#set text(size: 18pt, fill: rgb(1, 2, 3, 255))"));
        assert!(source.contains("#set align(horizon)"));
        assert!(source.contains("$pi r^2$"));
    }

    #[test]
    fn layer_selection_can_omit_background() {
        let source = build_annotated_typst_source(
            &one_page_doc(),
            &PresentationMetadata::default(),
            ExportLayers::Overlays,
            false,
            WhiteboardExport::None,
        );

        assert!(!source.contains("image(\"source.pdf\""));
    }

    #[test]
    fn pdf_background_source_uses_pdf_image_page() {
        let source = build_annotated_typst_source(
            &one_page_doc(),
            &PresentationMetadata::default(),
            ExportLayers::All,
            false,
            WhiteboardExport::None,
        );

        assert!(source.contains("image(\"source.pdf\", format: \"pdf\", page: 1"));
        assert!(source.contains("width: 200pt, height: 100pt, fit: \"stretch\""));
    }

    #[test]
    fn whiteboard_append_adds_page() {
        let meta = PresentationMetadata {
            whiteboard_annotations: vec![InkStrokeMeta {
                points: vec![(0.0, 0.0), (1.0, 1.0)],
                color: [0, 0, 0, 255],
                width: 2.0,
            }],
            ..Default::default()
        };

        let source = build_annotated_typst_source(
            &one_page_doc(),
            &meta,
            ExportLayers::All,
            false,
            WhiteboardExport::Append,
        );

        assert_eq!(source.matches("#page(").count(), 2);
    }

    #[test]
    fn handout_uses_final_page_of_each_logical_slide() {
        let doc = StubDocument {
            pages: vec![
                PageDimensions { width_pts: 200.0, height_pts: 100.0 },
                PageDimensions { width_pts: 200.0, height_pts: 100.0 },
                PageDimensions { width_pts: 200.0, height_pts: 100.0 },
            ],
        };
        let meta = PresentationMetadata {
            groups: vec![
                dais_sidecar::types::SlideGroupMeta { start_page: 0, end_page: 1 },
                dais_sidecar::types::SlideGroupMeta { start_page: 2, end_page: 2 },
            ],
            ..Default::default()
        };
        let source = build_annotated_typst_source(
            &doc,
            &meta,
            ExportLayers::All,
            true,
            WhiteboardExport::None,
        );

        assert_eq!(source.matches("#page(").count(), 2);
        assert!(!source.contains("page: 1,"));
        assert!(source.contains("page: 2,"));
        assert!(source.contains("page: 3,"));
    }

    #[test]
    fn svg_and_png_exports_return_one_artifact_per_page() {
        let pdf = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/test.pdf"),
        )
        .unwrap();
        let meta = PresentationMetadata::default();
        let svg = export_annotated_from_bytes(
            &one_page_doc(),
            pdf.clone(),
            request(&meta, ExportFormat::Svg),
        )
        .expect("svg export should work");
        let png =
            export_annotated_from_bytes(&one_page_doc(), pdf, request(&meta, ExportFormat::Png))
                .expect("png export should work");

        assert_eq!(svg.len(), 1);
        assert!(svg[0].bytes.starts_with(b"<svg"));
        assert_eq!(png.len(), 1);
        assert!(png[0].bytes.starts_with(b"\x89PNG"));
    }
}
