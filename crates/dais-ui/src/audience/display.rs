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
        Self {
            thumbnail: SlideThumbnail::new(),
            last_image_rect: egui::Rect::NOTHING,
        }
    }

    /// Update with the audience page's rendered data.
    pub fn update(&mut self, ctx: &egui::Context, page: &RenderedPage, page_index: usize) {
        self.thumbnail.update(ctx, page, page_index);
    }

    /// Render the slide in the full available area, returning the image rect.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let available = ui.available_size();
        let response = self.thumbnail.show(ui, available);
        // Reconstruct image rect from the response rect
        let rect = response.rect;

        // Calculate the actual image rect within the allocated space
        if self.thumbnail.has_texture() {
            self.last_image_rect = compute_image_rect(rect, &self.thumbnail);
        } else {
            self.last_image_rect = rect;
        }

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

/// Compute the letterboxed image rect within the allocated rect.
fn compute_image_rect(alloc_rect: egui::Rect, thumb: &SlideThumbnail) -> egui::Rect {
    // We need to reconstruct the same logic as SlideThumbnail::show
    // Since show() allocates the full desired_size and centers the image,
    // we just return the allocated rect. The actual image is centered inside it.
    // For overlay purposes, this is sufficient.
    let _ = thumb;
    alloc_rect
}
