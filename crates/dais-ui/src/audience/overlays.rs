//! Audience window overlays — laser, ink, spotlight.
//!
//! Renders visual aids over the audience slide image.

use dais_core::state::{PointerStyle, PresentationState};
use dais_document::typst_renderer::TextBoxRenderCache;

use crate::widgets::TextBoxTextureCache;

/// Draw all active overlays on the audience window.
pub fn draw_overlays(
    ui: &mut egui::Ui,
    viewport_rect: egui::Rect,
    image_rect: egui::Rect,
    state: &PresentationState,
    tb_cache: &mut TextBoxRenderCache,
    texture_cache: &mut TextBoxTextureCache,
    draw_text_boxes: bool,
) {
    // Ink strokes
    let page_ink = state.current_page_ink();
    if !page_ink.is_empty() {
        crate::widgets::draw_ink_strokes(ui, image_rect, page_ink);
    }

    // Text boxes (non-interactive on audience display)
    let page_text_boxes = state.current_page_text_boxes();
    if draw_text_boxes && !page_text_boxes.is_empty() {
        let _ = crate::widgets::draw_text_boxes(
            ui,
            page_text_boxes,
            None,
            None,
            false,
            image_rect,
            tb_cache,
            texture_cache,
        );
    }

    // Laser pointer
    if state.laser_active
        && let Some((px, py)) = state.pointer_position
    {
        draw_laser_overlay(
            ui,
            image_rect,
            px,
            py,
            state.pointer_color,
            state.pointer_size,
            state.pointer_style,
        );
    }

    // Spotlight
    if state.spotlight_active
        && let Some((sx, sy)) = state.spotlight_position
    {
        draw_spotlight_overlay(
            ui,
            image_rect,
            sx,
            sy,
            state.spotlight_radius,
            state.spotlight_dim_opacity,
        );
    }

    // Blackout
    if state.blacked_out {
        ui.painter().rect_filled(viewport_rect, 0.0, egui::Color32::BLACK);
    }

    // Whiteboard — white canvas with dedicated strokes
    if state.whiteboard_active {
        ui.painter().rect_filled(viewport_rect, 0.0, egui::Color32::WHITE);
        if !state.whiteboard_strokes.is_empty() {
            crate::widgets::draw_ink_strokes(ui, image_rect, &state.whiteboard_strokes);
        }
    }
}

/// Draw the configured pointer overlay at normalized position.
pub fn draw_laser_overlay(
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    nx: f32,
    ny: f32,
    color: [u8; 4],
    size: f32,
    style: PointerStyle,
) {
    let pos = denormalize(image_rect, nx, ny);
    let painter = ui.painter();
    let color = color32_from_rgba(color);
    let size = size.clamp(2.0, 96.0);
    let glow = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 72);

    match style {
        PointerStyle::Dot => {
            painter.circle_filled(pos, (size * 0.95).max(4.0), glow);
            painter.circle_filled(pos, (size * 0.45).max(2.0), color);
        }
        PointerStyle::Crosshair => {
            let arm = (size * 1.2).max(8.0);
            let gap = (size * 0.35).max(3.0);
            let stroke = egui::Stroke::new((size * 0.16).max(1.5), color);
            painter.circle_stroke(pos, (size * 0.45).max(3.0), stroke);
            painter
                .line_segment([pos + egui::vec2(-arm, 0.0), pos + egui::vec2(-gap, 0.0)], stroke);
            painter.line_segment([pos + egui::vec2(gap, 0.0), pos + egui::vec2(arm, 0.0)], stroke);
            painter
                .line_segment([pos + egui::vec2(0.0, -arm), pos + egui::vec2(0.0, -gap)], stroke);
            painter.line_segment([pos + egui::vec2(0.0, gap), pos + egui::vec2(0.0, arm)], stroke);
            painter.circle_filled(pos, (size * 0.18).max(1.5), color);
        }
        PointerStyle::Arrow => {
            // A conventional pointer arrow: tip at the cursor, trailing down-right
            // with a slight downward rotation from the 45-degree diagonal.
            let head_len = (size * 0.70).max(6.0);
            let head_half_width = (size * 0.24).max(2.2);
            let tail_len = (size * 0.31).max(3.0);
            let tail_start = head_len * 0.50;
            let angle = 60.0_f32.to_radians();
            let dir = egui::vec2(angle.cos(), angle.sin());
            let perp = egui::vec2(-dir.y, dir.x);
            let base_center = pos + dir * head_len;
            let left = base_center + perp * head_half_width;
            let right = base_center - perp * head_half_width;
            let tail_join = pos + dir * tail_start;
            let tail_end = pos + dir * (head_len + tail_len);
            let stroke = egui::Stroke::new((size * 0.145).max(1.7), color);
            painter.line_segment([tail_join, tail_end], stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![pos, left, right],
                color,
                egui::Stroke::NONE,
            ));
            painter.circle_filled(pos, (size * 0.16).max(1.5), glow);
        }
    }
}

/// Draw a spotlight overlay — dims everything outside a circle.
///
/// Public so it can be shared between audience and presenter windows.
pub fn draw_spotlight_overlay(
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    nx: f32,
    ny: f32,
    radius: f32,
    dim_opacity: f32,
) {
    let half_size = radius.clamp(16.0, image_rect.width().min(image_rect.height()) * 0.45);
    let center = denormalize(image_rect, nx, ny);
    let painter = ui.painter_at(image_rect);
    let dim_color =
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, dim_opacity_to_alpha(dim_opacity));
    let hole_rect =
        egui::Rect::from_center_size(center, egui::vec2(half_size * 2.0, half_size * 2.0))
            .intersect(image_rect);

    if hole_rect.top() > image_rect.top() {
        painter.rect_filled(
            egui::Rect::from_min_max(
                image_rect.left_top(),
                egui::pos2(image_rect.right(), hole_rect.top()),
            ),
            0.0,
            dim_color,
        );
    }
    if hole_rect.bottom() < image_rect.bottom() {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(image_rect.left(), hole_rect.bottom()),
                image_rect.right_bottom(),
            ),
            0.0,
            dim_color,
        );
    }
    if hole_rect.left() > image_rect.left() {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(image_rect.left(), hole_rect.top()),
                egui::pos2(hole_rect.left(), hole_rect.bottom()),
            ),
            0.0,
            dim_color,
        );
    }
    if hole_rect.right() < image_rect.right() {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(hole_rect.right(), hole_rect.top()),
                egui::pos2(image_rect.right(), hole_rect.bottom()),
            ),
            0.0,
            dim_color,
        );
    }

    // Bright border to indicate spotlight edge
    painter.rect_stroke(
        hole_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100)),
        egui::StrokeKind::Outside,
    );
}

fn color32_from_rgba(color: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3])
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn dim_opacity_to_alpha(dim_opacity: f32) -> u8 {
    (dim_opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Draw a zoom indicator at the given position.
///
/// Public so the presenter view can show the target region while the audience
/// sees only the zoomed result.
pub fn draw_zoom_indicator(
    ui: &mut egui::Ui,
    image_rect: egui::Rect,
    center: (f32, f32),
    factor: f32,
) {
    let pos = denormalize(image_rect, center.0, center.1);
    let painter = ui.painter();

    // Draw a rectangle showing the zoom region
    let half_w = image_rect.width() / (factor * 2.0);
    let half_h = image_rect.height() / (factor * 2.0);
    let zoom_rect = egui::Rect::from_center_size(pos, egui::vec2(half_w * 2.0, half_h * 2.0));

    painter.rect_stroke(
        zoom_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::YELLOW),
        egui::StrokeKind::Outside,
    );

    // Label
    painter.text(
        zoom_rect.right_top() + egui::vec2(4.0, 0.0),
        egui::Align2::LEFT_TOP,
        format!("{factor:.1}x"),
        egui::FontId::proportional(12.0),
        egui::Color32::YELLOW,
    );
}

/// Convert normalized (0..1) coordinates to screen-space within the image rect.
fn denormalize(rect: egui::Rect, nx: f32, ny: f32) -> egui::Pos2 {
    egui::pos2(rect.min.x + nx * rect.width(), rect.min.y + ny * rect.height())
}
