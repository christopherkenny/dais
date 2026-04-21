# Text Box Feature Plan

Text boxes are positioned Typst-rendered overlays on slides — placed by the user in a draw-like interaction, stored as normalized rects with Typst markup content, compiled to RGBA bitmaps and composited over the PDF.

## Dependencies to Add

```toml
typst = "0.14"
typst-render = "0.14"   # raster/PNG output
typst-library = "0.14"  # bundled standard library + fonts
typst-kit = "0.14"      # world utilities (font loading, package resolution)
```

Add to `dais-document/Cargo.toml`.

## New Data Structures

**`dais-core/src/state.rs`**

```rust
pub struct TextBox {
    pub id: u64,
    pub rect: (f32, f32, f32, f32),  // normalized x, y, w, h (0..1)
    pub content: String,              // Typst markup
    pub font_size: f32,               // pt
    pub color: [u8; 4],               // RGBA text color
    pub background: Option<[u8; 4]>, // optional fill
}
```

Added to `PresentationState`:
- `slide_text_boxes_by_page: HashMap<usize, Vec<TextBox>>`
- `text_box_mode: bool`
- `selected_text_box: Option<u64>`
- `text_box_editing: bool` — distinct from selected (selected = handles shown, editing = text input focused)

## New Commands

**`dais-core/src/commands.rs`**

```
ToggleTextBoxMode
PlaceTextBox { x, y, w, h }        // normalized, creates box on current page
EditTextBoxContent { id, content }
MoveTextBox { id, x, y }
ResizeTextBox { id, w, h }
DeleteTextBox { id }
SelectTextBox(u64)
DeselectTextBox
SetTextBoxFontSize { id, size }
SetTextBoxColor { id, color }
SetTextBoxBackground { id, color: Option<[u8; 4]> }
```

## Typst Rendering

**`dais-document/src/typst_renderer.rs`** (new file)

Implement `MinimalWorld` satisfying `typst::World`:
- No filesystem access — content is inline only
- Font loading via `typst-kit`'s font database utilities
- Fixed wall time (irrelevant for text boxes)

```rust
pub fn render_text_box(
    content: &str,
    px_width: u32,
    px_height: u32,
    font_size: f32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
) -> Result<RenderedPage>
```

Wraps content in a minimal document template:

```typst
#set page(width: Wpx, height: Hpx, margin: 4pt, fill: BG)
#set text(size: Fpt, fill: rgb(R, G, B, A))
CONTENT
```

Returns the same `RenderedPage { data: Vec<u8>, width, height }` used by the PDF renderer — composited via the existing egui texture upload path.

**Cache**: `TextBoxRenderCache` keyed on `(content_hash, width, height, font_size_bits, color)`. Invalidated on any content or style change.

## Input Handling

**`dais-ui/src/input.rs`** — new `InputMode::TextBox` variant.

| State | Trigger | Command |
|-------|---------|---------|
| Mode active, nothing selected | Click-drag | `PlaceTextBox` |
| Click existing box | Click | `SelectTextBox` |
| Double-click / Enter on selected | — | `text_box_editing = true` |
| Drag box body | Drag | `MoveTextBox` |
| Drag corner/edge handle | Drag | `ResizeTextBox` |
| Delete/Backspace (selected, not editing) | Key | `DeleteTextBox` |
| Escape | Key | `DeselectTextBox` or exit editing |

## Rendering

**`dais-ui/src/widgets/text_box_canvas.rs`** (new file)

```rust
pub fn draw_text_boxes(
    painter: &egui::Painter,
    boxes: &[TextBox],
    selected_id: Option<u64>,
    editing_id: Option<u64>,
    render_cache: &mut TextBoxRenderCache,
    slide_rect: egui::Rect,
)
```

For each box on the current page:
1. Denormalize rect to screen space using `slide_rect`
2. Look up or request Typst render
3. Draw texture via `painter.image()`
4. If selected: draw dashed border + 8 resize handles (4 corners + 4 midpoints)
5. If editing: overlay a transparent `egui::TextEdit::multiline` exactly over the rendered texture; on commit (Ctrl+Enter or click away) recompile Typst and update cache

Integrated into `dais-ui/src/audience/overlays.rs` alongside `draw_ink_strokes` — called after PDF render, before laser/spotlight.

## Serialization

**`dais-sidecar/src/types.rs`**

```rust
pub struct TextBoxMeta {
    pub id: u64,
    pub rect: (f32, f32, f32, f32),
    pub content: String,
    pub font_size: f32,
    pub color: [u8; 4],
    pub background: Option<[u8; 4]>,
}
```

Added to `PresentationMetadata`:
```rust
pub slide_text_boxes: HashMap<usize, Vec<TextBoxMeta>>,
```

Serialized in EON format in `dais-sidecar/src/dais_format.rs` alongside existing `slide_annotations`. Same persistence lifecycle as ink — saved on `save_sidecar()`, loaded via `load_annotations_into_state()`.

## Implementation Phases

| Phase | Scope | Key Files |
|-------|-------|-----------|
| 1. State + Commands | Add structs/commands, engine handles mutations, no rendering | `dais-core/src/state.rs`, `commands.rs`, `dais-engine/src/engine.rs` |
| 2. Placeholder render | Colored rect with egui label — UX testable without Typst | `dais-ui/src/widgets/text_box_canvas.rs`, `audience/overlays.rs` |
| 3. Input + interaction | Place, select, move, resize, delete | `dais-ui/src/input.rs`, `presenter/mod.rs` |
| 4. Typst renderer | `MinimalWorld`, compile+render pipeline, cache | `dais-document/src/typst_renderer.rs` |
| 5. Inline editor | TextEdit overlay, live re-render on content change | `text_box_canvas.rs` |
| 6. Serialization | Save/load text boxes in `.dais` sidecar | `dais-sidecar/src/types.rs`, `dais_format.rs` |
| 7. Style controls | Font size, color, background in presenter toolbar | `dais-ui/src/presenter/` |

## Design Decisions

- **Normalized coordinates** — same 0..1 system as ink, denormalized at render time. Boxes survive resolution changes.
- **Typst as library, not subprocess** — uses `typst` crate directly with `MinimalWorld`. No PATH dependency, no shelling out.
- **Render cache keyed on content hash** — Typst compilation takes ~50ms; only recompile on content/style change, not every frame.
- **Editing overlay** — transparent `TextEdit` floats over the rendered texture for a live-preview feel without per-keystroke recompile (recompile on commit only).
- **Text box mode is exclusive with ink mode** — same mutual-exclusion pattern as existing modes.

## Implementation Status

All phases complete.

| Phase | Status | Notes |
|-------|--------|-------|
| 1. State + Commands | ✅ Done | All structs, commands, keybinding (`x`), engine handlers |
| 2. Placeholder render | ✅ Done | egui colored rect + label; selected border + 4 corner handles |
| 3. Input + interaction | ✅ Done | Place, select, double-click edit, drag move/resize, delete, Escape |
| 4. Typst renderer | ✅ Done | `MinimalWorld` + `render_text_box` + `TextBoxRenderCache` in `dais-document/src/typst_renderer.rs`; build and all tests pass |
| 5. Inline editor | ✅ Done | `TextEdit` overlay; on commit re-renders via Typst; cache invalidated on each commit |
| 6. Serialization | ✅ Done | `TextBoxMeta` + `slide_text_boxes` field; full EON roundtrip in `.dais` |
| 7. Style controls | ⚠️ Partial | `[X]Text` status indicator; commands `SetTextBoxFontSize/Color/Background` are wired; no toolbar UI |

### Phase 4 — Typst renderer details

Implemented in `dais-document/src/typst_renderer.rs`:

- **`MinimalWorld`** — implements `typst::World`; inline source only; fonts loaded once via `LazyLock<Fonts>` using `FontSearcher::new().search()`; `today()` returns `None`.
- **`render_text_box`** — wraps content in a Typst template (page dimensions, text color, background fill), compiles via `typst::compile::<PagedDocument>`, rasterizes with `typst_render::render` at 1 px/pt, converts premultiplied → straight alpha for egui.
- **`TextBoxRenderCache`** — keyed on `(content_hash, width, height, font_size_bits, color, background)`; `get_or_render` compiles on miss; `invalidate` clears by content hash (called on edit commit).
- **`draw_text_boxes`** — takes `&mut TextBoxRenderCache`; uploads RGBA bitmap via `ctx.load_texture` and draws with `painter.image()`; falls back to plain `child.label()` if typst compilation fails.
- **Three `TextBoxRenderCache` instances** — one each in `PresenterConsole`, `AudienceWindow`, and `HudOverlay`; threaded into `draw_overlays` and `draw_text_boxes`.
