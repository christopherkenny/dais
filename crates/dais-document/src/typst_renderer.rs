use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::LazyLock;

use tracing::warn;
use typst::Library;
use typst::LibraryExt;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::{FontSearcher, Fonts};

/// RGBA bitmap output from a Typst render.
pub struct RenderedTextBox {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// Load system fonts once; font loading is expensive.
static FONTS: LazyLock<Fonts> = LazyLock::new(|| FontSearcher::new().search());

struct MinimalWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    source: Source,
    main_id: FileId,
}

impl MinimalWorld {
    fn new(markup: String) -> Self {
        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let source = Source::new(main_id, markup);
        let fonts = &*FONTS;
        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(fonts.book.clone()),
            source,
            main_id,
        }
    }
}

impl typst_library::World for MinimalWorld {
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
        Err(FileError::NotFound(id.vpath().as_rootless_path().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}

/// Render a text box to an RGBA bitmap.
///
/// Returns `None` if compilation or rasterization fails.
pub fn render_text_box(
    content: &str,
    typst_prelude: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> Option<RenderedTextBox> {
    let markup =
        build_markup(content, typst_prelude, px_width, px_height, font_size, color, background);
    if let Some(rendered) = compile_markup(&markup) {
        return Some(rendered);
    }

    let fallback_markup = build_plain_text_fallback(
        content,
        typst_prelude,
        px_width,
        px_height,
        font_size,
        color,
        background,
    );
    let fallback = compile_markup(&fallback_markup);
    if fallback.is_some() {
        warn!("Typst text box render failed; fell back to plain-text rendering");
    } else {
        warn!("Typst text box render failed, including plain-text fallback");
    }
    fallback
}

/// Render a text box to SVG markup.
///
/// Returns `None` if compilation fails.
pub fn render_text_box_svg(
    content: &str,
    typst_prelude: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> Option<String> {
    let markup =
        build_markup(content, typst_prelude, px_width, px_height, font_size, color, background);
    if let Some(svg) = compile_markup_svg(&markup) {
        return Some(svg);
    }

    let fallback_markup = build_plain_text_fallback(
        content,
        typst_prelude,
        px_width,
        px_height,
        font_size,
        color,
        background,
    );
    let fallback = compile_markup_svg(&fallback_markup);
    if fallback.is_some() {
        warn!("Typst text box SVG render failed; fell back to plain-text rendering");
    } else {
        warn!("Typst text box SVG render failed, including plain-text fallback");
    }
    fallback
}

fn build_markup(
    content: &str,
    typst_prelude: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> String {
    let bg = match background {
        Some([r, g, b, a]) => format!("rgb({r}, {g}, {b}, {a})"),
        None => "rgb(0, 0, 0, 0)".to_string(),
    };
    let [r, g, b, a] = color;
    let x_margin = (font_size * 0.2).clamp(1.0, 4.0);
    let top_margin = (font_size * 0.35).max(2.0);
    let prelude =
        if typst_prelude.trim().is_empty() { String::new() } else { format!("{typst_prelude}\n") };
    format!(
        "#set page(width: {px_width}pt, height: {px_height}pt, margin: (x: {x_margin}pt, y: 4pt, top: {top_margin}pt), fill: {bg})\n\
         #set text(size: {font_size}pt, fill: rgb({r}, {g}, {b}, {a}))\n\
         {prelude}\
         {content}"
    )
}

fn build_plain_text_fallback(
    content: &str,
    typst_prelude: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> String {
    let escaped = escape_typst_string(content);
    format!(
        "{}\n#{}",
        build_markup("", typst_prelude, px_width, px_height, font_size, color, background),
        escaped
    )
}

fn escape_typst_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('"');
    for ch in s.chars() {
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

fn compile_markup(markup: &str) -> Option<RenderedTextBox> {
    let world = MinimalWorld::new(markup.to_owned());
    let result = typst::compile::<typst_library::layout::PagedDocument>(&world);
    let document = result.output.ok()?;
    let page = document.pages.into_iter().next()?;

    // pixel_per_pt = 1.0 because we set page dimensions in pt equal to px_width/px_height
    let pixmap = typst_render::render(&page, 1.0);

    // tiny-skia outputs premultiplied RGBA; convert to straight for egui.
    let src = pixmap.data();
    let mut rgba = Vec::with_capacity(src.len());
    for chunk in src.chunks_exact(4) {
        let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if a == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.push(unpremultiply_channel(r, a));
            rgba.push(unpremultiply_channel(g, a));
            rgba.push(unpremultiply_channel(b, a));
            rgba.push(a);
        }
    }

    Some(RenderedTextBox { data: rgba, width: pixmap.width(), height: pixmap.height() })
}

fn compile_markup_svg(markup: &str) -> Option<String> {
    let world = MinimalWorld::new(markup.to_owned());
    let result = typst::compile::<typst_library::layout::PagedDocument>(&world);
    let document = result.output.ok()?;
    let page = document.pages.into_iter().next()?;
    Some(typst_svg::svg(&page))
}

fn unpremultiply_channel(channel: u8, alpha: u8) -> u8 {
    let numerator = u16::from(channel) * 255 + (u16::from(alpha) / 2);
    let value = numerator / u16::from(alpha);
    u8::try_from(value).expect("unpremultiplied channel is always in u8 range")
}

/// Cache key for rendered text boxes.
#[derive(PartialEq, Eq, Hash, Clone)]
struct CacheKey {
    content_hash: u64,
    prelude_hash: u64,
    width: u32,
    height: u32,
    font_size_bits: u32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
}

fn hash_str(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Cache mapping (content, size, style) → rendered bitmap.
pub struct TextBoxRenderCache {
    entries: HashMap<CacheKey, RenderedTextBox>,
}

/// Inputs for rendering and caching one text box.
#[derive(Clone, Copy)]
pub struct TextBoxRenderRequest<'a> {
    pub content: &'a str,
    pub typst_prelude: &'a str,
    pub px_width: u32,
    pub px_height: u32,
    pub font_size: f32,
    pub color: [u8; 4],
    pub background: Option<[u8; 4]>,
}

impl Default for TextBoxRenderCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBoxRenderCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    /// Get or render a text box bitmap.
    pub fn get_or_render(&mut self, request: TextBoxRenderRequest<'_>) -> Option<&RenderedTextBox> {
        let key = CacheKey {
            content_hash: hash_str(request.content),
            prelude_hash: hash_str(request.typst_prelude),
            width: request.px_width,
            height: request.px_height,
            font_size_bits: request.font_size.to_bits(),
            color: request.color,
            background: request.background,
        };
        if !self.entries.contains_key(&key)
            && let Some(rendered) = render_text_box(
                request.content,
                request.typst_prelude,
                request.px_width,
                request.px_height,
                request.font_size,
                request.color,
                request.background,
            )
        {
            self.entries.insert(key.clone(), rendered);
        }
        self.entries.get(&key)
    }

    /// Remove all cached entries for a given content string (call after edit).
    pub fn invalidate(&mut self, content: &str) {
        let h = hash_str(content);
        self.entries.retain(|k, _| k.content_hash != h);
    }
}

#[cfg(test)]
mod tests {
    use super::{render_text_box, render_text_box_svg};

    fn top_row_has_visible_pixels(rendered: &super::RenderedTextBox) -> bool {
        rendered.data[..rendered.width as usize * 4].chunks_exact(4).any(|pixel| pixel[3] > 0)
    }

    fn first_visible_pixel(rendered: &super::RenderedTextBox) -> Option<[u8; 4]> {
        rendered
            .data
            .chunks_exact(4)
            .find(|pixel| pixel[3] > 0)
            .map(|pixel| <[u8; 4]>::try_from(pixel).expect("chunk size is exactly four bytes"))
    }

    #[test]
    fn renders_valid_typst_markup() {
        let rendered =
            render_text_box("Hello, *Typst*!", "", 240, 80, 20.0, [255, 255, 255, 255], None);
        assert!(rendered.is_some());
    }

    #[test]
    fn renders_valid_typst_markup_to_svg() {
        let svg =
            render_text_box_svg("Hello, *Typst*!", "", 240, 80, 20.0, [255, 255, 255, 255], None)
                .expect("valid typst should render to SVG");

        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn falls_back_for_invalid_typst_markup() {
        let rendered = render_text_box("#let x = ", "", 240, 80, 20.0, [255, 255, 255, 255], None)
            .expect("fallback should still produce a bitmap");
        assert_eq!(rendered.width, 240);
        assert_eq!(rendered.height, 80);
        assert_eq!(rendered.data.len(), 240 * 80 * 4);
    }

    #[test]
    fn transparent_background_renders_without_white_matte() {
        let rendered = render_text_box("$pi r^2$", "", 240, 80, 24.0, [0, 0, 0, 255], None)
            .expect("formula should render");

        assert_eq!(&rendered.data[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn first_line_superscript_has_top_padding() {
        let rendered = render_text_box("$pi r^2$", "", 240, 80, 24.0, [0, 0, 0, 255], None)
            .expect("formula should render");

        assert!(!top_row_has_visible_pixels(&rendered));
    }

    #[test]
    fn typst_prelude_can_override_default_text_settings() {
        let rendered = render_text_box(
            "Override",
            "#set text(fill: rgb(255, 0, 0))",
            240,
            80,
            20.0,
            [0, 0, 0, 255],
            None,
        )
        .expect("prelude should render");

        let pixel = first_visible_pixel(&rendered).expect("text should have visible pixels");
        assert!(pixel[0] > pixel[1]);
        assert!(pixel[0] > pixel[2]);
    }
}
