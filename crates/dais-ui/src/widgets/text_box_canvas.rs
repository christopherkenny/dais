//! Text box canvas widget.
//!
//! Renders text box overlays on a slide and handles placement, selection,
//! move, resize, and inline editing interactions.

use dais_core::commands::Command;
use dais_core::state::TextBox;
use dais_document::render_pipeline::FALLBACK_RENDER_SIZE;
use dais_document::typst_renderer::{TextBoxRenderCache, TextBoxRenderRequest};
use egui::{Color32, ColorImage, Id, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, vec2};
use std::collections::HashMap;

const HANDLE_RADIUS: f32 = 5.0;
const HANDLE_COLOR: Color32 = Color32::WHITE;
const SELECTED_BORDER: Color32 = Color32::from_rgb(100, 160, 255);
const SELECTED_BORDER_WIDTH: f32 = 2.0;
const MIN_PLACE_SIZE: f32 = 0.04;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TextureKey {
    id: u64,
    width: u32,
    height: u32,
    content_hash: u64,
    prelude_hash: u64,
    font_size_bits: u32,
    color: [u8; 4],
    background: Option<[u8; 4]>,
}

#[derive(Default)]
pub struct TextBoxTextureCache {
    textures: HashMap<TextureKey, TextureHandle>,
}

impl TextBoxTextureCache {
    pub fn get_or_load(
        &mut self,
        ui: &Ui,
        tb: &TextBox,
        rendered: &dais_document::typst_renderer::RenderedTextBox,
        width: u32,
        height: u32,
        font_size: f32,
    ) -> &TextureHandle {
        let key = TextureKey {
            id: tb.id,
            width,
            height,
            content_hash: content_hash(&tb.content),
            prelude_hash: content_hash(&tb.typst_prelude),
            font_size_bits: font_size.to_bits(),
            color: tb.color,
            background: tb.background,
        };
        self.textures.entry(key).or_insert_with(|| {
            let image = ColorImage::from_rgba_unmultiplied(
                [rendered.width as usize, rendered.height as usize],
                &rendered.data,
            );
            ui.ctx().load_texture(
                format!("tb_{}_{}", tb.id, key.content_hash),
                image,
                egui::TextureOptions::LINEAR,
            )
        })
    }

    pub fn retain_for_boxes(&mut self, boxes: &[TextBox], slide_rect: Rect) {
        let font_scale = slide_font_scale(slide_rect);
        self.textures.retain(|key, _| {
            boxes.iter().any(|tb| {
                let font_size = scaled_font_size(tb.font_size, font_scale);
                tb.id == key.id
                    && key.content_hash == content_hash(&tb.content)
                    && key.prelude_hash == content_hash(&tb.typst_prelude)
                    && key.color == tb.color
                    && key.background == tb.background
                    && key.font_size_bits == font_size.to_bits()
                    && key.width == texture_dimension(screen_rect(slide_rect, tb.rect).width())
                    && key.height == texture_dimension(screen_rect(slide_rect, tb.rect).height())
            })
        });
    }
}

/// Draw text box overlays on a slide image area.
///
/// When `text_box_mode` is true, the canvas also handles:
/// - Click-drag on empty space → [`Command::PlaceTextBox`]
/// - Click on box → [`Command::SelectTextBox`]
/// - Double-click on box → [`Command::BeginTextBoxEdit`]
/// - Drag box body → [`Command::MoveTextBox`]
/// - Drag corner handle → [`Command::ResizeTextBox`]
///
/// Returns a list of commands to dispatch. Non-interactive (audience) renders
/// should pass `text_box_mode: false`, `selected_id: None`, `editing_id: None`.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn draw_text_boxes(
    ui: &mut Ui,
    boxes: &[TextBox],
    selected_id: Option<u64>,
    editing_id: Option<u64>,
    text_box_mode: bool,
    slide_rect: Rect,
    tb_cache: &mut TextBoxRenderCache,
    texture_cache: &mut TextBoxTextureCache,
) -> Vec<Command> {
    let mut commands = Vec::new();
    let font_scale = slide_font_scale(slide_rect);
    texture_cache.retain_for_boxes(boxes, slide_rect);

    // --- Drag-to-place new box ---
    if text_box_mode {
        let place_start_id = Id::new("tb_place_start");
        let slide_resp =
            ui.interact(slide_rect, Id::new("tb_slide_interact"), Sense::click_and_drag());

        let place_start: Option<(f32, f32)> = ui.data(|d| d.get_temp(place_start_id));

        if slide_resp.drag_started() {
            // Only initiate place-drag if cursor was NOT inside any existing box
            let press_pos = ui.ctx().input(|i| i.pointer.press_origin());
            let on_box = press_pos
                .is_some_and(|p| boxes.iter().any(|b| screen_rect(slide_rect, b.rect).contains(p)));
            if !on_box && let Some(pos) = press_pos {
                let norm = norm_pos(pos, slide_rect);
                ui.data_mut(|d| d.insert_temp(place_start_id, norm));
            }
        }

        if let Some(start) = place_start {
            if slide_resp.drag_stopped() {
                let end = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos())
                    .map_or(start, |p| norm_pos(p, slide_rect));
                let x = start.0.min(end.0);
                let y = start.1.min(end.1);
                let w = (start.0 - end.0).abs().max(MIN_PLACE_SIZE);
                let h = (start.1 - end.1).abs().max(MIN_PLACE_SIZE);
                commands.push(Command::PlaceTextBox { x, y, w, h });
                ui.data_mut(|d| d.remove::<(f32, f32)>(place_start_id));
            } else if !slide_resp.dragged() {
                // Stale state (drag was cancelled), clear
                ui.data_mut(|d| d.remove::<(f32, f32)>(place_start_id));
            } else {
                // Draw placement preview
                let cur = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos())
                    .map_or(start, |p| norm_pos(p, slide_rect));
                let px = start.0.min(cur.0);
                let py = start.1.min(cur.1);
                let pw = (start.0 - cur.0).abs().max(0.01);
                let ph = (start.1 - cur.1).abs().max(0.01);
                let preview = screen_rect(slide_rect, (px, py, pw, ph));
                ui.painter_at(slide_rect).rect_stroke(
                    preview,
                    2.0,
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(100, 160, 255, 180)),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Click on empty space with no drag → deselect
        if slide_resp.clicked() && selected_id.is_some() {
            let click_pos = slide_resp.interact_pointer_pos();
            let on_box = click_pos
                .is_some_and(|p| boxes.iter().any(|b| screen_rect(slide_rect, b.rect).contains(p)));
            if !on_box {
                commands.push(Command::DeselectTextBox);
            }
        }
    }

    // --- Render each box ---
    for tb in boxes {
        let box_rect = screen_rect(slide_rect, tb.rect);
        let is_selected = selected_id == Some(tb.id);
        let is_editing = editing_id == Some(tb.id);

        // Background fill
        if let Some(bg) = tb.background {
            ui.painter_at(slide_rect).rect_filled(
                box_rect,
                2.0,
                Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]),
            );
        }

        if is_editing {
            // Inline TextEdit overlay
            let edit_buf_id = Id::new(("tb_edit_buf", tb.id));
            let mut buf: String = ui
                .data(|d| d.get_temp::<String>(edit_buf_id))
                .unwrap_or_else(|| tb.content.clone());

            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(box_rect.shrink(4.0)));
            child.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
            child.visuals_mut().override_text_color = Some(Color32::from_rgba_unmultiplied(
                tb.color[0],
                tb.color[1],
                tb.color[2],
                tb.color[3],
            ));
            let font_size = scaled_font_size(tb.font_size, font_scale);
            child.style_mut().override_font_id = Some(egui::FontId::proportional(font_size));

            let edit_resp = child.add_sized(
                box_rect.shrink(4.0).size(),
                egui::TextEdit::multiline(&mut buf)
                    .desired_width(f32::INFINITY)
                    .hint_text("Type here…"),
            );
            if edit_resp.changed() {
                commands.push(Command::EditTextBoxContent { id: tb.id, content: buf.clone() });
            }
            // Ctrl+Enter commits and exits editing
            let commit = child.ctx().input(|i| {
                i.events.iter().any(|e| {
                    matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. }
                        if modifiers.ctrl || modifiers.command)
                })
            });
            let clicked_away = child.ctx().input(|i| {
                i.pointer.any_pressed()
                    && i.pointer.interact_pos().is_some_and(|pos| !box_rect.contains(pos))
            });
            if commit || (edit_resp.lost_focus() && clicked_away) {
                // Invalidate cache for old content so next render re-compiles
                tb_cache.invalidate(&tb.content);
                commands.push(Command::DeselectTextBox);
            }
            ui.data_mut(|d| d.insert_temp(edit_buf_id, buf));

            // Border around editing box
            ui.painter_at(slide_rect).rect_stroke(
                box_rect,
                2.0,
                Stroke::new(SELECTED_BORDER_WIDTH, Color32::from_rgb(255, 200, 80)),
                egui::StrokeKind::Outside,
            );
        } else {
            // Typst-rendered texture
            let px_w = texture_dimension(box_rect.width());
            let px_h = texture_dimension(box_rect.height());
            let font_size = scaled_font_size(tb.font_size, font_scale);
            if let Some(rendered) = tb_cache.get_or_render(TextBoxRenderRequest {
                content: &tb.content,
                typst_prelude: &tb.typst_prelude,
                px_width: px_w,
                px_height: px_h,
                font_size,
                color: tb.color,
                background: tb.background,
            }) {
                let tex = texture_cache.get_or_load(ui, tb, rendered, px_w, px_h, font_size);
                ui.painter_at(slide_rect).image(
                    tex.id(),
                    box_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                // Fallback: plain label if typst render fails
                let text_color = Color32::from_rgba_unmultiplied(
                    tb.color[0],
                    tb.color[1],
                    tb.color[2],
                    tb.color[3],
                );
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(box_rect.shrink(4.0)));
                child.visuals_mut().override_text_color = Some(text_color);
                child.style_mut().override_font_id = Some(egui::FontId::proportional(font_size));
                child.label(egui::RichText::new(&tb.content).size(font_size).color(text_color));
            }
        }

        // --- Box interaction (only in text_box_mode) ---
        if text_box_mode && !is_editing {
            let box_resp =
                ui.interact(box_rect, Id::new(("tb_box", tb.id)), Sense::click_and_drag());

            if box_resp.double_clicked() {
                commands.push(Command::BeginTextBoxEdit { id: tb.id });
            } else if box_resp.clicked() {
                commands.push(Command::SelectTextBox(tb.id));
            }

            if box_resp.dragged() && is_selected {
                let delta = box_resp.drag_delta();
                let dx = delta.x / slide_rect.width();
                let dy = delta.y / slide_rect.height();
                let (bx, by, _, _) = tb.rect;
                commands.push(Command::MoveTextBox {
                    id: tb.id,
                    x: (bx + dx).max(0.0),
                    y: (by + dy).max(0.0),
                });
            }
        }

        // Selected border + resize handles
        if is_selected && !is_editing {
            ui.painter_at(slide_rect).rect_stroke(
                box_rect,
                2.0,
                Stroke::new(SELECTED_BORDER_WIDTH, SELECTED_BORDER),
                egui::StrokeKind::Outside,
            );

            if text_box_mode {
                // 4 corner handles — drag to resize
                let corners = [
                    (box_rect.left_top(), "nw"),
                    (box_rect.right_top(), "ne"),
                    (box_rect.left_bottom(), "sw"),
                    (box_rect.right_bottom(), "se"),
                ];
                for (corner, tag) in corners {
                    let handle_rect = Rect::from_center_size(
                        corner,
                        vec2(HANDLE_RADIUS * 2.0, HANDLE_RADIUS * 2.0),
                    );
                    let handle_resp =
                        ui.interact(handle_rect, Id::new(("tb_handle", tb.id, tag)), Sense::drag());
                    ui.painter_at(slide_rect).circle_filled(corner, HANDLE_RADIUS, HANDLE_COLOR);
                    ui.painter_at(slide_rect).circle_stroke(
                        corner,
                        HANDLE_RADIUS,
                        Stroke::new(1.0, Color32::from_gray(80)),
                    );

                    if handle_resp.dragged() {
                        let delta = handle_resp.drag_delta();
                        let dx = delta.x / slide_rect.width();
                        let dy = delta.y / slide_rect.height();
                        let (bx, by, bw, bh) = tb.rect;
                        let (new_x, new_y, new_w, new_h) = match tag {
                            "nw" => (bx + dx, by + dy, (bw - dx).max(0.02), (bh - dy).max(0.02)),
                            "ne" => (bx, by + dy, (bw + dx).max(0.02), (bh - dy).max(0.02)),
                            "sw" => (bx + dx, by, (bw - dx).max(0.02), (bh + dy).max(0.02)),
                            _ => (bx, by, (bw + dx).max(0.02), (bh + dy).max(0.02)), // se
                        };
                        // Move if anchor changed
                        if (new_x - bx).abs() > f32::EPSILON || (new_y - by).abs() > f32::EPSILON {
                            commands.push(Command::MoveTextBox {
                                id: tb.id,
                                x: new_x.max(0.0),
                                y: new_y.max(0.0),
                            });
                        }
                        commands.push(Command::ResizeTextBox { id: tb.id, w: new_w, h: new_h });
                    }
                }
            }
        }
    }

    commands
}

#[allow(clippy::cast_precision_loss)]
fn slide_font_scale(slide_rect: Rect) -> f32 {
    let width_scale = slide_rect.width() / FALLBACK_RENDER_SIZE.width as f32;
    let height_scale = slide_rect.height() / FALLBACK_RENDER_SIZE.height as f32;
    width_scale.min(height_scale).max(0.05)
}

fn scaled_font_size(font_size: f32, scale: f32) -> f32 {
    (font_size.clamp(8.0, 72.0) * scale).max(1.0)
}

/// Convert a normalized (x, y, w, h) rect to screen-space using the slide rect.
fn screen_rect(slide_rect: Rect, (nx, ny, nw, nh): (f32, f32, f32, f32)) -> Rect {
    Rect::from_min_size(
        Pos2::new(
            slide_rect.min.x + nx * slide_rect.width(),
            slide_rect.min.y + ny * slide_rect.height(),
        ),
        vec2(nw * slide_rect.width(), nh * slide_rect.height()),
    )
}

/// Convert a screen-space position to normalized 0..1 coordinates within the slide rect.
fn norm_pos(pos: Pos2, slide_rect: Rect) -> (f32, f32) {
    (
        ((pos.x - slide_rect.min.x) / slide_rect.width()).clamp(0.0, 1.0),
        ((pos.y - slide_rect.min.y) / slide_rect.height()).clamp(0.0, 1.0),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn texture_dimension(size: f32) -> u32 {
    size.max(1.0).ceil() as u32
}

fn content_hash(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{scaled_font_size, slide_font_scale};

    #[test]
    fn text_box_font_scales_with_slide_rect() {
        let full_slide =
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1920.0, 1080.0));
        let small_slide = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(960.0, 540.0));

        assert!((slide_font_scale(full_slide) - 1.0).abs() < f32::EPSILON);
        assert!((slide_font_scale(small_slide) - 0.5).abs() < f32::EPSILON);
        assert!(
            (scaled_font_size(20.0, slide_font_scale(small_slide)) - 10.0).abs() < f32::EPSILON
        );
    }
}
