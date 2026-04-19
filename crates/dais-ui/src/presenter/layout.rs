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
    /// Vertical splitter between the current slide and right column.
    pub vertical_splitter: egui::Rect,
    /// Horizontal splitter between next preview and notes.
    pub horizontal_splitter: egui::Rect,
}

/// Height of the status bar in logical pixels.
const STATUS_BAR_HEIGHT: f32 = 40.0;
/// Thickness of draggable splitters.
const SPLITTER_SIZE: f32 = 8.0;

impl PresenterLayout {
    /// Compute layout rects from the available content area.
    pub fn compute(available: egui::Rect, left_fraction: f32, top_fraction: f32) -> Self {
        let total_w = available.width();
        let total_h = available.height();

        let status_bar = egui::Rect::from_min_size(
            egui::pos2(available.min.x, available.max.y - STATUS_BAR_HEIGHT),
            egui::vec2(total_w, STATUS_BAR_HEIGHT),
        );

        let content_h = (total_h - STATUS_BAR_HEIGHT).max(0.0);
        let splitter_half = SPLITTER_SIZE * 0.5;
        let left_w = (total_w * left_fraction - splitter_half).max(0.0);
        let right_w = (total_w - left_w - SPLITTER_SIZE).max(0.0);

        let current_slide = egui::Rect::from_min_size(available.min, egui::vec2(left_w, content_h));
        let vertical_splitter = egui::Rect::from_min_max(
            egui::pos2(current_slide.max.x, available.min.y),
            egui::pos2(current_slide.max.x + SPLITTER_SIZE, available.min.y + content_h),
        );

        let right_top = egui::pos2(vertical_splitter.max.x, available.min.y);
        let top_h = (content_h * top_fraction - splitter_half).max(0.0);
        let bottom_h = (content_h - top_h - SPLITTER_SIZE).max(0.0);

        let next_preview = egui::Rect::from_min_size(right_top, egui::vec2(right_w, top_h));
        let horizontal_splitter = egui::Rect::from_min_max(
            egui::pos2(right_top.x, next_preview.max.y),
            egui::pos2(right_top.x + right_w, next_preview.max.y + SPLITTER_SIZE),
        );

        let notes_panel = egui::Rect::from_min_size(
            egui::pos2(right_top.x, horizontal_splitter.max.y),
            egui::vec2(right_w, bottom_h),
        );

        Self {
            current_slide,
            next_preview,
            notes_panel,
            status_bar,
            vertical_splitter,
            horizontal_splitter,
        }
    }
}
