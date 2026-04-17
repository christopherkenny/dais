//! Audience window — fullscreen slide display and overlays.
//!
//! Composes the audience display + visual aid overlays.

pub mod display;
pub mod overlays;

use std::sync::{Arc, RwLock};

use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::source::DocumentSource;

use self::display::AudienceDisplay;

/// The audience window.
pub struct AudienceWindow {
    display: AudienceDisplay,
}

impl AudienceWindow {
    pub fn new() -> Self {
        Self { display: AudienceDisplay::new() }
    }

    /// Render the audience window content.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        shared_state: &Arc<RwLock<PresentationState>>,
        doc: &dyn DocumentSource,
        cache: &mut PageCache,
    ) {
        let state = shared_state.read().map_or_else(
            |e| {
                tracing::error!("Failed to read state for audience window: {e}");
                PresentationState::new(0, Vec::new())
            },
            |s| s.clone(),
        );

        let audience_page = state.audience_page();

        // Render page at audience resolution
        let render_size = RenderSize { width: 1920, height: 1080 };

        if cache.get(audience_page, render_size).is_none()
            && let Ok(rendered) = doc.render_page(audience_page, render_size)
        {
            cache.insert(audience_page, render_size, rendered);
        }

        if let Some(page) = cache.get(audience_page, render_size) {
            let page = page.clone();
            self.display.update(ctx, &page, audience_page);
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(
            ctx,
            |ui| {
                let image_rect = self.display.show(ui);
                overlays::draw_overlays(ui, image_rect, &state);
            },
        );
    }
}

impl Default for AudienceWindow {
    fn default() -> Self {
        Self::new()
    }
}
