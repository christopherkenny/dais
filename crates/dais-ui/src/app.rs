//! Top-level eframe application and window lifecycle.
//!
//! Manages the presentation engine, document source, page cache, and both
//! presenter and audience windows.

use std::sync::{Arc, RwLock};

use dais_core::bus::CommandSender;
use dais_core::config::Config;
use dais_core::keybindings::KeybindingMap;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::source::DocumentSource;
use dais_engine::engine::PresentationEngine;

use crate::audience::AudienceWindow;
use crate::input::InputHandler;
use crate::presenter::PresenterConsole;

/// The main Dais application, implementing `eframe::App`.
pub struct DaisApp {
    engine: PresentationEngine,
    shared_state: Arc<RwLock<PresentationState>>,
    doc: Box<dyn DocumentSource>,
    cache: PageCache,
    presenter: PresenterConsole,
    audience: AudienceWindow,
    sender: CommandSender,
    screen_share_mode: bool,
}

impl DaisApp {
    /// Create a new Dais application.
    ///
    /// # Arguments
    /// * `engine` - The presentation engine (already created with CommandBus receiver)
    /// * `shared_state` - Shared state handle from the engine
    /// * `doc` - The document source (PDF)
    /// * `sender` - Command sender for dispatching commands
    /// * `config` - Application configuration
    /// * `screen_share_mode` - Whether to start in screen-share mode
    pub fn new(
        engine: PresentationEngine,
        shared_state: Arc<RwLock<PresentationState>>,
        doc: Box<dyn DocumentSource>,
        sender: CommandSender,
        config: &Config,
        screen_share_mode: bool,
    ) -> Self {
        let keybindings = KeybindingMap::from_config(&config.keybindings);
        let input = InputHandler::new(sender.clone(), keybindings);
        let presenter = PresenterConsole::new(input);
        let audience = AudienceWindow::new();
        let cache = PageCache::new(32);

        Self {
            engine,
            shared_state,
            doc,
            cache,
            presenter,
            audience,
            sender,
            screen_share_mode,
        }
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

        // Request continuous repainting (timer, animations)
        ctx.request_repaint();

        // Read current state for screen_share_mode check
        let is_screen_share = self
            .shared_state
            .read()
            .map(|s| s.screen_share_mode)
            .unwrap_or(self.screen_share_mode);

        // Render the presenter console in the main viewport
        self.presenter.show(
            ctx,
            &self.shared_state,
            self.doc.as_ref(),
            &mut self.cache,
            &self.sender,
        );

        // Render the audience window in a secondary viewport
        // In screen-share mode, use a normal window; otherwise fullscreen
        let viewport_builder = if is_screen_share {
            egui::ViewportBuilder::default()
                .with_title("Dais — Audience")
                .with_inner_size(egui::vec2(1280.0, 720.0))
        } else {
            egui::ViewportBuilder::default()
                .with_title("Dais — Audience")
                .with_fullscreen(true)
        };

        let shared = self.shared_state.clone();
        let doc: &dyn DocumentSource = self.doc.as_ref();

        // We need to pass mutable cache, but show_viewport_immediate takes an
        // FnOnce. We'll render the audience page into the cache beforehand.
        {
            let audience_page = self
                .shared_state
                .read()
                .map(|s| s.audience_page())
                .unwrap_or(0);
            let render_size =
                dais_document::page::RenderSize { width: 1920, height: 1080 };
            if self.cache.get(audience_page, render_size).is_none() {
                if let Ok(rendered) = doc.render_page(audience_page, render_size) {
                    self.cache.insert(audience_page, render_size, rendered);
                }
            }
        }

        // The audience window needs its own cache reference. Since
        // show_viewport_immediate runs synchronously, we can pass a mutable ref
        // to audience and cache through the closure.
        let audience = &mut self.audience;
        let cache = &mut self.cache;
        let shared_ref = &shared;
        let doc_ref: &dyn DocumentSource = self.doc.as_ref();

        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("audience"),
            viewport_builder,
            |ctx, _class| {
                audience.show(ctx, shared_ref, doc_ref, cache);
            },
        );
    }
}
