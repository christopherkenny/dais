//! Audience slide rendering with letterboxing.
//!
//! Renders the audience-facing slide (centered, letterboxed/pillarboxed).

use crate::widgets::SlideThumbnail;
use dais_document::page::RenderedPage;

/// The audience display — renders a single slide centered with letterboxing.
pub struct AudienceDisplay {
    thumbnail: SlideThumbnail,
    /// Stored image rect for overlay coordinate mapping.
    last_image_rect: egui::Rect,
}

impl AudienceDisplay {
    pub fn new() -> Self {
        Self { thumbnail: SlideThumbnail::new(), last_image_rect: egui::Rect::NOTHING }
    }

    /// Update with the audience page's rendered data.
    pub fn update(&mut self, ctx: &egui::Context, page: &RenderedPage, page_index: usize) {
        self.thumbnail.update(ctx, page, page_index);
    }

    /// Render the slide in the full available area, returning the image rect.
    /// If `zoom_region` is provided, render only that sub-region magnified.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        zoom_region: Option<((f32, f32), f32)>,
    ) -> egui::Rect {
        let available = ui.available_size();
        let response = if let Some((center, factor)) = zoom_region {
            self.thumbnail.show_zoomed(ui, available, center, factor)
        } else {
            self.thumbnail.show(ui, available)
        };
        self.last_image_rect = response.rect;

        self.last_image_rect
    }

    pub fn image_rect(&self) -> egui::Rect {
        self.last_image_rect
    }
}

impl Default for AudienceDisplay {
    fn default() -> Self {
        Self::new()
    }
}
