//! Audience window overlays — laser, ink, spotlight, zoom.
//!
//! Renders visual aids over the audience slide image.

use dais_core::state::PresentationState;

/// Draw all active overlays on the audience window.
pub fn draw_overlays(
    ui: &mut egui::Ui,
    viewport_rect: egui::Rect,
    image_rect: egui::Rect,
    state: &PresentationState,
) {
    // Ink strokes
    if !state.ink_strokes.is_empty() {
        crate::widgets::draw_ink_strokes(ui, image_rect, &state.ink_strokes);
    }

    // Laser pointer
    if state.laser_active
        && let Some((px, py)) = state.pointer_position
    {
        draw_laser(ui, image_rect, px, py);
    }

    // Spotlight
    if state.spotlight_active
        && let Some((sx, sy)) = state.spotlight_position
    {
        draw_spotlight(ui, image_rect, sx, sy);
    }

    // Zoom
    if state.zoom_active
        && let Some(ref region) = state.zoom_region
    {
        draw_zoom_indicator(ui, image_rect, region.center, region.factor);
    }

    // Blackout
    if state.blacked_out {
        ui.painter().rect_filled(viewport_rect, 0.0, egui::Color32::BLACK);
    }
}

/// Draw a red laser dot at normalized position.
fn draw_laser(ui: &mut egui::Ui, image_rect: egui::Rect, nx: f32, ny: f32) {
    let pos = denormalize(image_rect, nx, ny);
    let painter = ui.painter();

    // Outer glow
    painter.circle_filled(pos, 10.0, egui::Color32::from_rgba_unmultiplied(255, 0, 0, 60));
    // Inner dot
    painter.circle_filled(pos, 5.0, egui::Color32::RED);
}

/// Draw a spotlight overlay — dims everything outside a circle.
///
/// Public so it can be shared between audience and presenter windows.
pub fn draw_spotlight_overlay(ui: &mut egui::Ui, image_rect: egui::Rect, nx: f32, ny: f32) {
    let half_size = (image_rect.width().min(image_rect.height()) * 0.075).clamp(30.0, 76.0);
    let center = denormalize(image_rect, nx, ny);
    let painter = ui.painter_at(image_rect);
    let dim_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 150);
    let hole_rect = egui::Rect::from_center_size(center, egui::vec2(half_size * 2.0, half_size * 2.0))
        .intersect(image_rect);

    if hole_rect.top() > image_rect.top() {
        painter.rect_filled(
            egui::Rect::from_min_max(image_rect.left_top(), egui::pos2(image_rect.right(), hole_rect.top())),
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

/// Draw a spotlight overlay — dims everything outside a circle.
fn draw_spotlight(ui: &mut egui::Ui, image_rect: egui::Rect, nx: f32, ny: f32) {
    draw_spotlight_overlay(ui, image_rect, nx, ny);
}

/// Draw a zoom indicator at the given position.
fn draw_zoom_indicator(ui: &mut egui::Ui, image_rect: egui::Rect, center: (f32, f32), factor: f32) {
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
