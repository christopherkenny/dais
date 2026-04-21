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
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> Option<RenderedTextBox> {
    let markup = build_markup(content, px_width, px_height, font_size, color, background);
    if let Some(rendered) = compile_markup(&markup) {
        return Some(rendered);
    }

    let fallback_markup =
        build_plain_text_fallback(content, px_width, px_height, font_size, color, background);
    let fallback = compile_markup(&fallback_markup);
    if fallback.is_some() {
        warn!("Typst text box render failed; fell back to plain-text rendering");
    } else {
        warn!("Typst text box render failed, including plain-text fallback");
    }
    fallback
}

fn build_markup(
    content: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> String {
    let bg = match background {
        Some([r, g, b, a]) => format!("rgb({r}, {g}, {b}, {a})"),
        None => "none".to_string(),
    };
    let [r, g, b, a] = color;
    format!(
        "#set page(width: {px_width}pt, height: {px_height}pt, margin: 4pt, fill: {bg})\n\
         #set text(size: {font_size}pt, fill: rgb({r}, {g}, {b}, {a}))\n\
         {content}"
    )
}

fn build_plain_text_fallback(
    content: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> String {
    let escaped = escape_typst_string(content);
    format!("{}\n#{}", build_markup("", px_width, px_height, font_size, color, background), escaped)
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

fn unpremultiply_channel(channel: u8, alpha: u8) -> u8 {
    let numerator = u16::from(channel) * 255 + (u16::from(alpha) / 2);
    let value = numerator / u16::from(alpha);
    u8::try_from(value).expect("unpremultiplied channel is always in u8 range")
}

/// Cache key for rendered text boxes.
#[derive(PartialEq, Eq, Hash, Clone)]
struct CacheKey {
    content_hash: u64,
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
    pub fn get_or_render(
        &mut self,
        content: &str,
        px_width: u32,
        px_height: u32,
        font_size: f32,
        color: [u8; 4],
        background: Option<[u8; 4]>,
    ) -> Option<&RenderedTextBox> {
        let key = CacheKey {
            content_hash: hash_str(content),
            width: px_width,
            height: px_height,
            font_size_bits: font_size.to_bits(),
            color,
            background,
        };
        if !self.entries.contains_key(&key)
            && let Some(rendered) =
                render_text_box(content, px_width, px_height, font_size, color, background)
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
    use super::render_text_box;

    #[test]
    fn renders_valid_typst_markup() {
        let rendered =
            render_text_box("Hello, *Typst*!", 240, 80, 20.0, [255, 255, 255, 255], None);
        assert!(rendered.is_some());
    }

    #[test]
    fn falls_back_for_invalid_typst_markup() {
        let rendered = render_text_box("#let x = ", 240, 80, 20.0, [255, 255, 255, 255], None)
            .expect("fallback should still produce a bitmap");
        assert_eq!(rendered.width, 240);
        assert_eq!(rendered.height, 80);
        assert_eq!(rendered.data.len(), 240 * 80 * 4);
    }
}
