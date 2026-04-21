//! Ink canvas drawing widget.
//!
//! Renders ink strokes as overlays on a slide image rect.

use dais_core::state::InkStroke;
use egui::{Color32, Rect, Stroke, Ui};

/// Render ink strokes over a slide image area.
///
/// `image_rect` is the screen-space rect of the slide image.
/// Stroke points are in normalized (0..1) coordinates.
pub fn draw_ink_strokes(ui: &mut Ui, image_rect: Rect, strokes: &[InkStroke]) {
    let painter = ui.painter_at(image_rect);

    for stroke in strokes {
        if stroke.points.len() < 2 {
            if let Some(&(x, y)) = stroke.points.first() {
                let pos = denormalize(image_rect, x, y);
                let color = rgba_to_color32(stroke.color);
                painter.circle_filled(pos, stroke.width * 0.5, color);
            }
            continue;
        }

        let color = rgba_to_color32(stroke.color);
        let egui_stroke = Stroke::new(stroke.width, color);
        let radius = stroke.width * 0.5;

        let points: Vec<egui::Pos2> =
            stroke.points.iter().map(|&(x, y)| denormalize(image_rect, x, y)).collect();

        for &pt in &points {
            painter.circle_filled(pt, radius, color);
        }
        for window in points.windows(2) {
            painter.line_segment([window[0], window[1]], egui_stroke);
        }
    }
}

/// Convert normalized (0..1) coordinates to screen-space position within the image rect.
fn denormalize(rect: Rect, nx: f32, ny: f32) -> egui::Pos2 {
    egui::pos2(rect.min.x + nx * rect.width(), rect.min.y + ny * rect.height())
}

/// Convert an RGBA byte array to egui `Color32`.
fn rgba_to_color32(rgba: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}
