//! Presenter console layout and panels.
//!
//! Composes the current slide, next preview, notes, timer, and overview
//! into the presenter console window.

pub mod current_slide;
pub mod layout;
pub mod next_preview;
pub mod notes_panel;
pub mod overview;
pub mod timer;

use std::sync::{Arc, RwLock};

use dais_core::bus::CommandSender;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::source::DocumentSource;

use self::current_slide::CurrentSlidePanel;
use self::layout::PresenterLayout;
use self::next_preview::NextPreviewPanel;
use self::notes_panel::NotesPanel;
use self::overview::OverviewGrid;

use crate::input::InputHandler;

/// The presenter console window — composes all sub-panels.
pub struct PresenterConsole {
    current_slide: CurrentSlidePanel,
    next_preview: NextPreviewPanel,
    notes: NotesPanel,
    overview: OverviewGrid,
    input: InputHandler,
}

impl PresenterConsole {
    pub fn new(input: InputHandler) -> Self {
        Self {
            current_slide: CurrentSlidePanel::new(),
            next_preview: NextPreviewPanel::new(),
            notes: NotesPanel::new(),
            overview: OverviewGrid::new(),
            input,
        }
    }

    /// Render the presenter console in the given egui context.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        shared_state: &Arc<RwLock<PresentationState>>,
        doc: &dyn DocumentSource,
        cache: &mut PageCache,
        sender: &CommandSender,
    ) {
        let state = shared_state.read().map(|s| s.clone()).unwrap_or_else(|e| {
            tracing::error!("Failed to read presentation state: {e}");
            PresentationState::new(0, Vec::new())
        });

        // Process input
        self.input.handle_input(
            ctx,
            state.overview_visible,
            state.ink_active,
            state.laser_active,
        );

        // Render current page texture
        let current_page = state.current_page;
        self.render_page_texture(ctx, doc, cache, current_page, &state, true);

        // Render next page texture
        let next_page = if current_page + 1 < state.total_pages {
            Some(current_page + 1)
        } else {
            None
        };
        if let Some(np) = next_page {
            self.render_page_texture(ctx, doc, cache, np, &state, false);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(30)))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();
                let layout = PresenterLayout::compute(available);

                // Current slide
                let (response, image_rect) =
                    self.current_slide.show(ui, layout.current_slide);

                // Handle mouse on current slide
                self.input.handle_slide_mouse(
                    &response,
                    image_rect,
                    state.ink_active,
                    state.laser_active,
                    state.spotlight_active,
                );

                // Draw ink strokes on presenter view
                if !state.ink_strokes.is_empty() {
                    crate::widgets::draw_ink_strokes(
                        ui,
                        image_rect,
                        &state.ink_strokes,
                    );
                }

                // Draw laser dot on presenter view
                if state.laser_active {
                    if let Some((px, py)) = state.pointer_position {
                        let pos = egui::pos2(
                            image_rect.min.x + px * image_rect.width(),
                            image_rect.min.y + py * image_rect.height(),
                        );
                        ui.painter().circle_filled(
                            pos,
                            6.0,
                            egui::Color32::RED,
                        );
                    }
                }

                // Next preview
                if let Some(np) = next_page {
                    let _ = np;
                    self.next_preview.show(ui, layout.next_preview);
                } else {
                    self.next_preview.show_empty(ui, layout.next_preview);
                }

                // Notes panel
                self.notes.show(
                    ui,
                    layout.notes_panel,
                    state.current_notes.as_deref(),
                    state.notes_font_size,
                    state.notes_visible,
                );

                // Status bar
                self.show_status_bar(ui, layout.status_bar, &state);

                // Slide overview (modal overlay)
                if state.overview_visible {
                    self.overview.show(ctx, ui, &state, doc, cache, sender);
                }
            });
    }

    fn render_page_texture(
        &mut self,
        ctx: &egui::Context,
        doc: &dyn DocumentSource,
        cache: &mut PageCache,
        page_index: usize,
        _state: &PresentationState,
        is_current: bool,
    ) {
        // Use a reasonable render size for the presenter
        let render_size = RenderSize { width: 1280, height: 960 };

        if cache.get(page_index, render_size).is_none() {
            match doc.render_page(page_index, render_size) {
                Ok(rendered) => {
                    cache.insert(page_index, render_size, rendered);
                }
                Err(e) => {
                    tracing::warn!("Failed to render page {page_index}: {e}");
                    return;
                }
            }
        }

        if let Some(page) = cache.get(page_index, render_size) {
            let page = page.clone();
            if is_current {
                self.current_slide.update(ctx, &page, page_index);
            } else {
                self.next_preview.update(ctx, &page, page_index);
            }
        }
    }

    fn show_status_bar(
        &self,
        ui: &mut egui::Ui,
        area: egui::Rect,
        state: &PresentationState,
    ) {
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));

        // Background
        child_ui
            .painter()
            .rect_filled(area, 0.0, egui::Color32::from_gray(20));

        child_ui.allocate_ui_at_rect(
            area.shrink(4.0),
            |ui| {
                ui.horizontal(|ui| {
                    // Slide position
                    let slide_text = format!(
                        "Slide {}/{}",
                        state.current_logical_slide + 1,
                        state.total_logical_slides,
                    );

                    let group = state.slide_groups.get(state.current_logical_slide);
                    let overlay_text = if let Some(g) = group {
                        if g.pages.len() > 1 {
                            format!(
                                " (step {}/{})",
                                state.current_overlay_within_group + 1,
                                g.pages.len()
                            )
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    ui.label(
                        egui::RichText::new(format!("{slide_text}{overlay_text}"))
                            .size(14.0)
                            .color(egui::Color32::WHITE),
                    );

                    ui.separator();

                    // Timer
                    timer::show_timer(ui, &state.timer);

                    ui.separator();

                    // Mode indicators
                    let mut indicators = Vec::new();
                    if state.frozen {
                        indicators.push(("[F]rozen", egui::Color32::LIGHT_BLUE));
                    }
                    if state.blacked_out {
                        indicators.push(("[B]lack", egui::Color32::YELLOW));
                    }
                    if state.screen_share_mode {
                        indicators.push(("[S]creen-share", egui::Color32::LIGHT_GREEN));
                    }
                    if state.laser_active {
                        indicators.push(("[L]aser", egui::Color32::RED));
                    }
                    if state.ink_active {
                        indicators.push(("[D]raw", egui::Color32::from_rgb(255, 165, 0)));
                    }
                    if state.spotlight_active {
                        indicators.push(("Spotlight", egui::Color32::LIGHT_YELLOW));
                    }
                    if state.zoom_active {
                        indicators.push(("[Z]oom", egui::Color32::LIGHT_GREEN));
                    }

                    for (text, color) in indicators {
                        ui.colored_label(color, egui::RichText::new(text).size(12.0));
                    }

                    // Jump mode indicator
                    if self.input.mode() == crate::input::InputMode::JumpToSlide {
                        let buf = self.input.jump_buffer();
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            egui::RichText::new(format!("Go to: {buf}_")).size(14.0),
                        );
                    }
                });
            },
        );
    }
}
