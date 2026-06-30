//! Single-monitor HUD overlay.
//!
//! When presentation mode is active in single-monitor mode, the main window
//! shows the audience slide fullscreen with a semi-transparent bottom bar
//! containing timer, slide count, and mode indicators. Notes are accessible
//! via a hover panel near the bottom edge.

use dais_core::bus::CommandSender;
use dais_core::state::{DrawTool, PresentationState};
use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::typst_renderer::TextBoxRenderCache;

use crate::audience::display::AudienceDisplay;
use crate::audience::overlays;
use crate::input::{InputHandler, UiModes};
use crate::widgets::{HelpOverlay, TextBoxTextureCache};

const HUD_BAR_HEIGHT: f32 = 48.0;
const HUD_BAR_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(20, 20, 20, 180);
const NOTES_PANEL_HEIGHT: f32 = 128.0;
const NOTES_HOVER_ZONE: f32 = 80.0;
const HUD_BAR_HOVER_ZONE: f32 = 64.0;

/// The presentation HUD — fullscreen slide with a hoverable info bar.
pub struct HudOverlay {
    display: AudienceDisplay,
    notes_visible: bool,
    bar_visible: bool,
    help: HelpOverlay,
    tb_cache: TextBoxRenderCache,
    tb_texture_cache: TextBoxTextureCache,
}

impl HudOverlay {
    pub fn new() -> Self {
        Self {
            display: AudienceDisplay::default(),
            notes_visible: false,
            bar_visible: false,
            help: HelpOverlay::new(),
            tb_cache: TextBoxRenderCache::new(),
            tb_texture_cache: TextBoxTextureCache::default(),
        }
    }
}

impl Default for HudOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HudOverlay {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
        input: &mut InputHandler,
        render_size: RenderSize,
    ) {
        // Help overlay intercepts input when visible.
        let help_consumed = self.help.show(ctx, input.keybindings());

        if !help_consumed && !state.notes_editing && Self::question_mark_pressed(ctx) {
            self.help.toggle();
        }

        if !self.help.visible {
            input.handle_input(
                ctx,
                UiModes {
                    overview_visible: state.overview_visible,
                    ink_active: state.ink_active,
                    laser_active: state.laser_active,
                    notes_editing: state.notes_editing,
                    text_box_mode: state.text_box_mode,
                    text_box_editing: state.text_box_editing,
                    selected_text_box: state.selected_text_box,
                },
            );
        }

        let audience_page = state.audience_page();
        if let Some(page) = cache.get(audience_page, render_size) {
            let page = page.clone();
            self.display.update(ctx, &page, audience_page);
        }

        egui::CentralPanel::default().frame(egui::Frame::new().fill(egui::Color32::BLACK)).show(
            ctx,
            |ui| {
                let viewport_rect = ui.max_rect();

                // Fullscreen slide (with zoom if active)
                let zoom_region = if state.zoom_active {
                    state.zoom_region.as_ref().map(|r| (r.center, r.factor))
                } else {
                    None
                };
                let image_rect = self.display.show(ui, zoom_region);

                // Mouse interaction on the slide
                let response = ui.interact(
                    image_rect,
                    egui::Id::new("hud_slide_interact"),
                    egui::Sense::click_and_drag(),
                );
                input.handle_slide_mouse(
                    &response,
                    image_rect,
                    crate::input::ActiveAids {
                        ink: state.ink_active,
                        eraser: state.draw_tool == DrawTool::Eraser,
                        eraser_radius: state.eraser_radius,
                        laser: state.laser_active,
                        spotlight: state.spotlight_active,
                        zoom: state.zoom_active,
                    },
                    state.zoom_region.as_ref().map(|region| region.factor),
                );

                // Audience overlays (ink, laser, spotlight, zoom, blackout)
                overlays::draw_overlays(
                    ui,
                    overlays::OverlayGeometry { viewport_rect, image_rect, zoom_region },
                    state,
                    &mut self.tb_cache,
                    &mut self.tb_texture_cache,
                    true,
                );

                // Notes hover detection
                if let Some(pointer) = ctx.pointer_hover_pos() {
                    let near_bottom = pointer.y > viewport_rect.max.y - NOTES_HOVER_ZONE;
                    self.bar_visible = pointer.y > viewport_rect.max.y - HUD_BAR_HOVER_ZONE;
                    self.notes_visible = near_bottom && state.current_notes.is_some();
                } else {
                    self.bar_visible = false;
                    self.notes_visible = false;
                }

                // Notes pop-up panel
                if self.notes_visible
                    && let Some(notes) = state.current_notes.as_deref()
                {
                    Self::show_notes_panel(ui, viewport_rect, notes);
                }

                // HUD bar
                if (self.bar_visible || self.notes_visible)
                    && Self::show_hud_bar(ui, viewport_rect, state, sender)
                {
                    self.help.toggle();
                }
            },
        );
    }

    fn show_hud_bar(
        ui: &mut egui::Ui,
        viewport: egui::Rect,
        state: &PresentationState,
        sender: &CommandSender,
    ) -> bool {
        let mut help_clicked = false;
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(viewport.min.x, viewport.max.y - HUD_BAR_HEIGHT),
            viewport.max,
        );

        let painter = ui.painter();
        painter.rect_filled(bar_rect, 0.0, HUD_BAR_BG);

        let inner = bar_rect.shrink2(egui::vec2(12.0, 6.0));
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));

        child_ui.horizontal_centered(|ui| {
            // Slide position
            let slide_text =
                format!("{} / {}", state.current_logical_slide + 1, state.total_logical_slides);
            ui.label(egui::RichText::new(slide_text).size(16.0).color(egui::Color32::WHITE));

            // Overlay step
            if let Some(g) = state.slide_groups.get(state.current_logical_slide)
                && g.pages.len() > 1
            {
                ui.label(
                    egui::RichText::new(format!(
                        "step {}/{}",
                        state.current_overlay_within_group + 1,
                        g.pages.len()
                    ))
                    .size(13.0)
                    .color(egui::Color32::GRAY),
                );
            }

            ui.separator();

            // Timer (clickable)
            if super::timer::show_timer(ui, &state.timer) {
                let _ = sender.send(dais_core::commands::Command::ToggleTimer);
            }

            ui.separator();

            let target =
                state.slide_target_durations.get(state.current_logical_slide).copied().flatten();
            super::timer::show_slide_timer(ui, state.slide_elapsed, target);

            // Mode indicators + help button (right-aligned)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Help button (rightmost)
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("?")
                                .size(14.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(egui::Color32::from_gray(50))
                        .corner_radius(4.0)
                        .min_size(egui::vec2(24.0, 24.0)),
                    )
                    .on_hover_text("Keyboard shortcuts")
                    .clicked()
                {
                    help_clicked = true;
                }

                if state.frozen {
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        egui::RichText::new("FROZEN").size(12.0),
                    );
                }
                if state.ink_active {
                    crate::widgets::show_ink_toolbar(ui, state, sender);
                }
                if state.laser_active {
                    ui.colored_label(egui::Color32::RED, egui::RichText::new("LASER").size(12.0));
                }
                if state.spotlight_active {
                    ui.colored_label(
                        egui::Color32::LIGHT_YELLOW,
                        egui::RichText::new("SPOT").size(12.0),
                    );
                }
                if state.zoom_active {
                    ui.colored_label(
                        egui::Color32::LIGHT_GREEN,
                        egui::RichText::new("ZOOM").size(12.0),
                    );
                }
            });
        });

        help_clicked
    }

    fn show_notes_panel(ui: &mut egui::Ui, viewport: egui::Rect, notes: &str) {
        let panel_rect = egui::Rect::from_min_max(
            egui::pos2(viewport.min.x, viewport.max.y - HUD_BAR_HEIGHT - NOTES_PANEL_HEIGHT),
            egui::pos2(viewport.max.x, viewport.max.y - HUD_BAR_HEIGHT),
        );

        let bg = egui::Color32::from_rgba_premultiplied(15, 15, 15, 210);
        ui.painter().rect_filled(panel_rect, 4.0, bg);

        let inner = panel_rect.shrink(10.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            egui::ScrollArea::vertical().max_height(inner.height()).show(ui, |ui| {
                ui.label(
                    egui::RichText::new(notes).size(14.0).color(egui::Color32::from_gray(220)),
                );
            });
        });
    }

    fn question_mark_pressed(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "?")))
    }
}
