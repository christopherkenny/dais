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
use dais_document::page::RenderSize;
use dais_document::render_pipeline::{FALLBACK_RENDER_SIZE, RenderPipeline};
use dais_document::source::DocumentSource;
use dais_engine::engine::PresentationEngine;
use dais_remote::RemoteStatusHandle;

use crate::audience::AudienceWindow;
use crate::display_mode::{self, AudienceReassignmentPrompt, DisplayMode, SingleMonitorView};
use crate::input::InputHandler;
use crate::presenter::PresenterConsole;
use crate::presenter::hud::HudOverlay;
use crate::widgets::ToastManager;

/// The main Dais application, implementing `eframe::App`.
pub struct DaisApp {
    engine: PresentationEngine,
    shared_state: Arc<RwLock<PresentationState>>,
    cache: PageCache,
    pipeline: RenderPipeline,
    presenter: PresenterConsole,
    hud: HudOverlay,
    audience: AudienceWindow,
    sender: CommandSender,
    display_mode: DisplayMode,
    single_monitor_view: SingleMonitorView,
    audience_reassignment: Option<AudienceReassignmentPrompt>,
    toast_manager: ToastManager,
    remote: Option<RemoteUiInfo>,
    presenter_window_size: egui::Vec2,
    applied_presenter_monitor_id: Option<String>,
}

const MAX_ZOOM_RENDER_DIMENSION: u32 = 4320;

#[derive(Clone)]
pub struct RemoteUiInfo {
    pub urls: Vec<String>,
    pub token: String,
    pub requires_token: bool,
    pub status: RemoteStatusHandle,
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
        let keybindings = KeybindingMap::from_full_config(config);
        let input = InputHandler::new(sender.clone(), keybindings);
        let presenter = PresenterConsole::new(input);
        let audience = AudienceWindow::new();
        let cache = PageCache::new(64);
        let pipeline = RenderPipeline::new(doc, 2);
        let presenter_window_size = egui::vec2(1400.0, 850.0);

        Self {
            engine,
            shared_state,
            cache,
            pipeline,
            presenter,
            hud: HudOverlay::new(),
            audience,
            sender,
            display_mode,
            single_monitor_view: SingleMonitorView::from_config(
                &config.display.single_monitor_view,
            ),
            audience_reassignment: None,
            toast_manager: ToastManager::new(),
            remote: None,
            presenter_window_size,
            applied_presenter_monitor_id: None,
        }
    }

    pub fn toast_manager_mut(&mut self) -> &mut ToastManager {
        &mut self.toast_manager
    }

    pub fn set_audience_reassignment(&mut self, prompt: Option<AudienceReassignmentPrompt>) {
        self.audience_reassignment = prompt;
    }

    pub fn set_remote_info(&mut self, remote: Option<RemoteUiInfo>) {
        self.remote = remote;
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
        let effective_display_mode =
            display_mode::effective_display_mode(&self.display_mode, state.displays_swapped);
        self.apply_presenter_viewport(ctx, &effective_display_mode);
        let base_audience_size = display_mode::audience_render_size(&effective_display_mode);
        let audience_size = effective_audience_render_size(&state, base_audience_size);
        self.pipeline.prefetch_neighborhood(
            state.current_page,
            state.total_pages,
            presenter_size,
            &mut self.cache,
        );
        // Audience page (may differ if frozen)
        self.pipeline.ensure_rendered(state.audience_page(), base_audience_size, &mut self.cache);
        self.pipeline.ensure_rendered(state.audience_page(), audience_size, &mut self.cache);

        // When overview is visible, request renders for all logical slide thumbnails
        if state.overview_visible {
            for group in &state.slide_groups {
                if let Some(&first_page) = group.pages.first() {
                    self.pipeline.ensure_rendered(first_page, presenter_size, &mut self.cache);
                }
            }
        }

        // Request periodic repaints while timers are active or renders are pending.
        if state.timer.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            // The per-slide timer updates every second, while the render pipeline
            // still benefits from a modest polling cadence.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        // In Single mode, F5 (presentation_mode) flips between the two views.
        // The configured single_monitor_view is the default; F5 shows the other.
        if matches!(self.display_mode, DisplayMode::Single) {
            let render_size = effective_audience_render_size(&state, FALLBACK_RENDER_SIZE);
            let active_view = if state.presentation_mode {
                match self.single_monitor_view {
                    SingleMonitorView::Hud => SingleMonitorView::Split,
                    SingleMonitorView::Split => SingleMonitorView::Hud,
                }
            } else {
                self.single_monitor_view
            };
            match active_view {
                SingleMonitorView::Hud => {
                    let input = self.presenter.input_mut();
                    self.hud.show(ctx, &state, &mut self.cache, &self.sender, input, render_size);
                }
                SingleMonitorView::Split => {
                    self.presenter.show_single_monitor_split(
                        ctx,
                        &state,
                        &mut self.cache,
                        &self.sender,
                        render_size,
                        self.remote.as_ref(),
                    );
                }
            }
            self.show_audience_reassignment_prompt(ctx);
            self.toast_manager.show(ctx);
            return;
        }

        // Read runtime screen-share toggle
        let is_runtime_screen_share = state.screen_share_mode;

        // Render the presenter console in the main viewport
        self.presenter.show(ctx, &state, &mut self.cache, &self.sender, self.remote.as_ref());

        // Choose audience viewport builder
        let viewport_builder = if is_runtime_screen_share {
            display_mode::with_app_icon(egui::ViewportBuilder::default())
                .with_title("Dais — Audience")
                .with_inner_size(egui::vec2(1280.0, 720.0))
                .with_fullscreen(false)
                .with_resizable(true)
        } else {
            display_mode::audience_viewport_builder(&effective_display_mode)
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

        self.show_audience_reassignment_prompt(ctx);
        self.toast_manager.show(ctx);
    }
}

impl DaisApp {
    fn apply_presenter_viewport(&mut self, ctx: &egui::Context, mode: &DisplayMode) {
        let DisplayMode::Dual { presenter_monitor, .. } = mode else {
            self.applied_presenter_monitor_id = None;
            return;
        };

        if self.applied_presenter_monitor_id.as_deref() == Some(presenter_monitor.id.as_str()) {
            return;
        }

        let Some(placement) = display_mode::presenter_viewport_placement(
            presenter_monitor,
            self.presenter_window_size,
        ) else {
            return;
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(placement.inner_size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(placement.position));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        self.applied_presenter_monitor_id = Some(presenter_monitor.id.clone());
    }

    fn current_presenter_monitor(&self) -> Option<dais_core::monitor::MonitorInfo> {
        match &self.display_mode {
            DisplayMode::Dual { presenter_monitor, .. } => Some(presenter_monitor.clone()),
            DisplayMode::Single | DisplayMode::ScreenShare => None,
        }
    }

    fn presenter_for_audience(
        &self,
        audience_monitor: &dais_core::monitor::MonitorInfo,
        available_monitors: &[dais_core::monitor::MonitorInfo],
    ) -> dais_core::monitor::MonitorInfo {
        self.current_presenter_monitor()
            .filter(|presenter| presenter.id != audience_monitor.id)
            .or_else(|| {
                available_monitors
                    .iter()
                    .find(|candidate| candidate.id != audience_monitor.id)
                    .cloned()
            })
            .unwrap_or_else(|| audience_monitor.clone())
    }

    fn show_audience_reassignment_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.audience_reassignment.clone() else {
            return;
        };

        egui::Window::new("Audience Monitor Changed")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.label(format!(
                    "Configured audience monitor '{}' is not available.",
                    prompt.missing_selector
                ));

                if let Some(fallback) = &prompt.attempted_fallback {
                    ui.label(format!(
                        "Dais is currently using '{}' as a fallback for this session.",
                        fallback.name
                    ));
                } else {
                    ui.label("No audience monitor fallback was available, so Dais switched to single-monitor mode.");
                }

                ui.separator();
                ui.label("Choose the audience output for this session:");

                let mut dismiss = false;
                let fallback_id = prompt.attempted_fallback.as_ref().map(|monitor| monitor.id.as_str());
                let alternative_monitors = prompt
                    .available_monitors
                    .iter()
                    .filter(|monitor| Some(monitor.id.as_str()) != fallback_id)
                    .cloned()
                    .collect::<Vec<_>>();

                if alternative_monitors.is_empty() {
                    ui.label(
                        egui::RichText::new("No alternate audience monitors were detected.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                }

                for monitor in &alternative_monitors {
                    let primary = if monitor.is_primary { ", primary" } else { "" };
                    let label = format!("Use {} ({}{primary})", monitor.name, monitor.id);
                    if ui.button(label).clicked() {
                        let presenter_monitor =
                            self.presenter_for_audience(monitor, &prompt.available_monitors);
                        self.display_mode = DisplayMode::Dual {
                            presenter_monitor,
                            audience_monitor: monitor.clone(),
                        };
                        self.toast_manager.push(
                            crate::widgets::toast::ToastLevel::Info,
                            format!("Audience moved to '{}'", monitor.name),
                        );
                        dismiss = true;
                    }
                }

                ui.horizontal(|ui| {
                    if let Some(fallback) = &prompt.attempted_fallback {
                        if ui.button("Keep current fallback").clicked() {
                            let presenter_monitor =
                                self.presenter_for_audience(fallback, &prompt.available_monitors);
                            self.display_mode = DisplayMode::Dual {
                                presenter_monitor,
                                audience_monitor: fallback.clone(),
                            };
                            self.toast_manager.push(
                                crate::widgets::toast::ToastLevel::Info,
                                format!("Keeping fallback audience monitor '{}'", fallback.name),
                            );
                            dismiss = true;
                        }

                        if ui.button("Use single-monitor mode").clicked() {
                            self.display_mode = DisplayMode::Single;
                            self.toast_manager.push(
                                crate::widgets::toast::ToastLevel::Info,
                                "Audience reassigned to single-monitor mode",
                            );
                            dismiss = true;
                        }
                    } else if ui.button("Stay in single-monitor mode").clicked() {
                        self.display_mode = DisplayMode::Single;
                        self.toast_manager.push(
                            crate::widgets::toast::ToastLevel::Info,
                            "Keeping single-monitor mode",
                        );
                        dismiss = true;
                    }
                });

                ui.label(
                    egui::RichText::new(
                        "This updates the current session only. You can make it permanent in `dais.toml` later.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );

                if dismiss {
                    self.audience_reassignment = None;
                }
            });
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn effective_audience_render_size(state: &PresentationState, base_size: RenderSize) -> RenderSize {
    let Some(region) = state.zoom_region.as_ref().filter(|_| state.zoom_active) else {
        return base_size;
    };

    // Render a denser source image while zoom is active so the cropped audience
    // view has more real pixels to work with. We cap the output size to keep
    // render cost bounded on extreme zoom factors.
    let multiplier = region.factor.clamp(1.0, 3.0);
    let width = ((base_size.width as f32) * multiplier).round() as u32;
    let height = ((base_size.height as f32) * multiplier).round() as u32;

    RenderSize {
        width: width.clamp(base_size.width, MAX_ZOOM_RENDER_DIMENSION),
        height: height.clamp(base_size.height, MAX_ZOOM_RENDER_DIMENSION),
    }
}
