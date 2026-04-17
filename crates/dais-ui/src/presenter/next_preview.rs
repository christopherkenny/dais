//! Next slide preview thumbnail.
//!
//! Shows a preview of the next slide/overlay in the presenter's right-top panel.

use crate::widgets::SlideThumbnail;
use dais_document::page::RenderedPage;

/// Manages the next-slide preview.
pub struct NextPreviewPanel {
    thumbnail: SlideThumbnail,
}

impl NextPreviewPanel {
    pub fn new() -> Self {
        Self { thumbnail: SlideThumbnail::new() }
    }

    /// Update with the next page's rendered data.
    pub fn update(&mut self, ctx: &egui::Context, page: &RenderedPage, page_index: usize) {
        self.thumbnail.update(ctx, page, page_index);
    }

    /// Render the preview in the given area.
    pub fn show(&self, ui: &mut egui::Ui, area: egui::Rect) {
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
        );

        // Header
        child_ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(area.min, egui::vec2(area.width(), 20.0))),
            |ui| {
                ui.colored_label(egui::Color32::GRAY, "Next");
            },
        );

        let content_area = egui::Rect::from_min_size(
            area.min + egui::vec2(0.0, 20.0),
            egui::vec2(area.width(), (area.height() - 20.0).max(1.0)),
        );

        let padding = egui::vec2(8.0, 4.0);
        let available = egui::vec2(
            (content_area.width() - padding.x * 2.0).max(1.0),
            (content_area.height() - padding.y * 2.0).max(1.0),
        );

        let mut inner = child_ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_area)
                .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
        );
        self.thumbnail.show(&mut inner, available);
    }

    /// Show a "no next slide" placeholder.
    pub fn show_empty(&self, ui: &mut egui::Ui, area: egui::Rect) {
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
        );
        child_ui.colored_label(egui::Color32::from_gray(100), "End of presentation");
    }
}

impl Default for NextPreviewPanel {
    fn default() -> Self {
        Self::new()
    }
}
