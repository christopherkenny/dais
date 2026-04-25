//! Audience window — fullscreen slide display and overlays.
//!
//! Composes the audience display + visual aid overlays.

pub mod display;
pub mod overlays;

use std::sync::{Arc, RwLock};

use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::typst_renderer::TextBoxRenderCache;

use self::display::AudienceDisplay;
use crate::widgets::TextBoxTextureCache;

/// The audience window.
pub struct AudienceWindow {
    display: AudienceDisplay,
    tb_cache: TextBoxRenderCache,
    tb_texture_cache: TextBoxTextureCache,
}

impl AudienceWindow {
    pub fn new() -> Self {
        Self {
            display: AudienceDisplay::new(),
            tb_cache: TextBoxRenderCache::new(),
            tb_texture_cache: TextBoxTextureCache::default(),
        }
    }

    /// Render the audience window content.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        shared_state: &Arc<RwLock<PresentationState>>,
        cache: &mut PageCache,
        render_size: RenderSize,
    ) {
        let state = shared_state.read().map_or_else(
            |e| {
                tracing::error!("Failed to read state for audience window: {e}");
                PresentationState::new(0, Vec::new())
            },
            |s| s.clone(),
        );

        let audience_page = state.audience_page();

        if let Some(page) = cache.get(audience_page, render_size) {
            let page = page.clone();
            self.display.update(ctx, &page, audience_page);
        }

        // Extract zoom region if active
        let zoom_region = if state.zoom_active {
            state.zoom_region.as_ref().map(|r| (r.center, r.factor))
        } else {
            None
        };

        egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(
            ctx,
            |ui| {
                let viewport_rect = ui.max_rect();
                let image_rect = self.display.show(ui, zoom_region);
                overlays::draw_overlays(
                    ui,
                    viewport_rect,
                    image_rect,
                    &state,
                    &mut self.tb_cache,
                    &mut self.tb_texture_cache,
                    true,
                );
            },
        );
    }
}

impl Default for AudienceWindow {
    fn default() -> Self {
        Self::new()
    }
}
