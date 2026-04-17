//! Current slide display in the presenter console.
//!
//! Renders the current slide with correct aspect ratio, letterboxed, in the
//! presenter's left panel.

use crate::widgets::SlideThumbnail;
use dais_document::page::RenderedPage;

/// Manages the current slide display in the presenter console.
pub struct CurrentSlidePanel {
    thumbnail: SlideThumbnail,
    /// Stored image rect from the last render (for mouse coordinate mapping).
    last_image_rect: egui::Rect,
}

impl CurrentSlidePanel {
    pub fn new() -> Self {
        Self { thumbnail: SlideThumbnail::new(), last_image_rect: egui::Rect::NOTHING }
    }

    /// Update the texture data when the page changes.
    pub fn update(&mut self, ctx: &egui::Context, page: &RenderedPage, page_index: usize) {
        self.thumbnail.update(ctx, page, page_index);
    }

    /// Render the current slide in the given area.
    /// Returns the response and the image rect for mouse handling.
    pub fn show(&mut self, ui: &mut egui::Ui, area: egui::Rect) -> (egui::Response, egui::Rect) {
        let padding = egui::vec2(8.0, 8.0);
        let content_rect = area.shrink2(padding);
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        let available = content_rect.size().max(egui::vec2(1.0, 1.0));

        let (response, image_rect) = self.thumbnail.show_interactive(&mut child_ui, available);
        self.last_image_rect = image_rect;
        (response, image_rect)
    }

    /// The image rect from the most recent render.
    pub fn image_rect(&self) -> egui::Rect {
        self.last_image_rect
    }
}

impl Default for CurrentSlidePanel {
    fn default() -> Self {
        Self::new()
    }
}
