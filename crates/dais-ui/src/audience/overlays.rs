//! Audience window overlays — laser, ink, spotlight, zoom.
//!
//! Renders visual aids over the audience slide image.

use dais_core::state::PresentationState;

/// Draw all active overlays on the audience window.
pub fn draw_overlays(ui: &mut egui::Ui, image_rect: egui::Rect, state: &PresentationState) {
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
        ui.painter().rect_filled(image_rect, 0.0, egui::Color32::BLACK);
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
fn draw_spotlight(ui: &mut egui::Ui, image_rect: egui::Rect, nx: f32, ny: f32) {
    let center = denormalize(image_rect, nx, ny);
    let radius = 120.0; // Logical pixels
    let painter = ui.painter_at(image_rect);

    // We approximate the spotlight by drawing 4 dark rectangles around the circle area.
    // For a proper spotlight, we'd need a custom shader. For v1, draw a semi-transparent
    // overlay and then clear a circle. Since egui doesn't support clip paths easily,
    // we'll draw a dark overlay with a "hole" by drawing many small wedges or just
    // accepting the approximation of a slightly dim overlay everywhere and a bright
    // circle at the spotlight position.

    // Simple approach: draw the full dark overlay, then paint the circle with the
    // original content. Since we can't "erase" the overlay, we'll skip the dim
    // overlay inside the circle area and just note that the dim_opacity from config
    // would ideally be used. For now, draw a ring to indicate spotlight.
    let dim_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 150);

    // Draw 4 rects that cover the area outside the spotlight circle (approximation)
    let r = radius;
    // Top band
    if center.y - r > image_rect.min.y {
        painter.rect_filled(
            egui::Rect::from_min_max(image_rect.min, egui::pos2(image_rect.max.x, center.y - r)),
            0.0,
            dim_color,
        );
    }
    // Bottom band
    if center.y + r < image_rect.max.y {
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(image_rect.min.x, center.y + r), image_rect.max),
            0.0,
            dim_color,
        );
    }
    // Left band (between top and bottom bands)
    let band_top = (center.y - r).max(image_rect.min.y);
    let band_bottom = (center.y + r).min(image_rect.max.y);
    if center.x - r > image_rect.min.x {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(image_rect.min.x, band_top),
                egui::pos2(center.x - r, band_bottom),
            ),
            0.0,
            dim_color,
        );
    }
    // Right band
    if center.x + r < image_rect.max.x {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(center.x + r, band_top),
                egui::pos2(image_rect.max.x, band_bottom),
            ),
            0.0,
            dim_color,
        );
    }

    // Bright circle border to indicate spotlight area
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100)),
    );
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
