//! Top-level eframe application and window lifecycle.
//!
//! Manages the presentation engine, document source, page cache, and both
//! presenter and audience windows.  Rendering is offloaded to a background
//! pipeline so the UI thread never blocks on hayro.

use std::sync::{Arc, RwLock};

use dais_core::bus::CommandSender;
use dais_core::config::Config;
use dais_core::keybindings::KeybindingMap;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::render_pipeline::{FALLBACK_RENDER_SIZE, RenderPipeline};
use dais_document::source::DocumentSource;
use dais_engine::engine::PresentationEngine;

use crate::audience::AudienceWindow;
use crate::display_mode::{self, DisplayMode};
use crate::input::InputHandler;
use crate::presenter::PresenterConsole;

/// The main Dais application, implementing `eframe::App`.
pub struct DaisApp {
    engine: PresentationEngine,
    shared_state: Arc<RwLock<PresentationState>>,
    cache: PageCache,
    pipeline: RenderPipeline,
    presenter: PresenterConsole,
    audience: AudienceWindow,
    sender: CommandSender,
    display_mode: DisplayMode,
}

impl DaisApp {
    /// Create a new Dais application.
    pub fn new(
        engine: PresentationEngine,
        shared_state: Arc<RwLock<PresentationState>>,
        doc: Arc<dyn DocumentSource>,
        sender: CommandSender,
        config: &Config,
        display_mode: DisplayMode,
    ) -> Self {
        let keybindings = KeybindingMap::from_config(&config.keybindings);
        let input = InputHandler::new(sender.clone(), keybindings);
        let presenter = PresenterConsole::new(input);
        let audience = AudienceWindow::new();
        let cache = PageCache::new(64);
        let pipeline = RenderPipeline::new(doc, 2);

        Self { engine, shared_state, cache, pipeline, presenter, audience, sender, display_mode }
    }
}

impl eframe::App for DaisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Tick the engine — processes commands, updates timer, broadcasts state
        let should_quit = self.engine.tick();
        if should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Collect completed background renders into the cache
        self.pipeline.poll_results(&mut self.cache);

        // Read state snapshot for this frame
        let state = self.shared_state.read().map_or_else(
            |e| {
                tracing::error!("Failed to read state: {e}");
                PresentationState::new(0, Vec::new())
            },
            |s| s.clone(),
        );

        // Submit render requests for pages we need
        let presenter_size = FALLBACK_RENDER_SIZE;
        let audience_size = display_mode::audience_render_size(&self.display_mode);
        self.pipeline.prefetch_neighborhood(
            state.current_page,
            state.total_pages,
            presenter_size,
            &mut self.cache,
        );
        // Audience page (may differ if frozen)
        self.pipeline.ensure_rendered(state.audience_page(), audience_size, &mut self.cache);

        // Request periodic repaints while timers are active or renders are pending.
        if state.timer.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            // The per-slide timer updates every second, while the render pipeline
            // still benefits from a modest polling cadence.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // In Single mode, only show the presenter — no audience viewport
        if matches!(self.display_mode, DisplayMode::Single) {
            self.presenter.show(ctx, &state, &mut self.cache, &self.sender);
            return;
        }

        // Read runtime screen-share toggle
        let is_runtime_screen_share = state.screen_share_mode;

        // Render the presenter console in the main viewport
        self.presenter.show(ctx, &state, &mut self.cache, &self.sender);

        // Choose audience viewport builder
        let viewport_builder = if is_runtime_screen_share {
            display_mode::with_app_icon(egui::ViewportBuilder::default())
                .with_title("Dais — Audience")
                .with_inner_size(egui::vec2(1280.0, 720.0))
        } else {
            display_mode::audience_viewport_builder(&self.display_mode)
        };

        let shared = self.shared_state.clone();
        let audience = &mut self.audience;
        let cache = &mut self.cache;
        let shared_ref = &shared;

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("audience"),
            viewport_builder,
            |ctx, _class| {
                audience.show(ctx, shared_ref, cache, audience_size);
            },
        );
    }
}
