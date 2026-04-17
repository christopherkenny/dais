//! Presenter console layout manager.
//!
//! Splits the presenter window into:
//! - Left panel (60%): current slide
//! - Right top (40% width, 50% height): next preview
//! - Right bottom (40% width, 50% height): notes panel
//! - Bottom bar (fixed ~40px): status/timer

/// Computed layout rectangles for the presenter console.
pub struct PresenterLayout {
    /// Current slide panel (left 60%).
    pub current_slide: egui::Rect,
    /// Next preview area (right top).
    pub next_preview: egui::Rect,
    /// Notes panel (right bottom).
    pub notes_panel: egui::Rect,
    /// Status bar at the bottom.
    pub status_bar: egui::Rect,
}

/// Height of the status bar in logical pixels.
const STATUS_BAR_HEIGHT: f32 = 40.0;
/// Fraction of window width allocated to the left (current slide) panel.
const LEFT_FRACTION: f32 = 0.60;

impl PresenterLayout {
    /// Compute layout rects from the available content area.
    pub fn compute(available: egui::Rect) -> Self {
        let total_w = available.width();
        let total_h = available.height();

        let status_bar = egui::Rect::from_min_size(
            egui::pos2(available.min.x, available.max.y - STATUS_BAR_HEIGHT),
            egui::vec2(total_w, STATUS_BAR_HEIGHT),
        );

        let content_h = (total_h - STATUS_BAR_HEIGHT).max(0.0);
        let left_w = total_w * LEFT_FRACTION;
        let right_w = total_w - left_w;

        let current_slide = egui::Rect::from_min_size(
            available.min,
            egui::vec2(left_w, content_h),
        );

        let right_top = available.min + egui::vec2(left_w, 0.0);
        let right_h_half = content_h * 0.5;

        let next_preview = egui::Rect::from_min_size(
            right_top,
            egui::vec2(right_w, right_h_half),
        );

        let notes_panel = egui::Rect::from_min_size(
            right_top + egui::vec2(0.0, right_h_half),
            egui::vec2(right_w, content_h - right_h_half),
        );

        Self {
            current_slide,
            next_preview,
            notes_panel,
            status_bar,
        }
    }
}
