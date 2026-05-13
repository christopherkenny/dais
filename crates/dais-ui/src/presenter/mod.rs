//! Presenter console layout and panels.
//!
//! Composes the current slide, next preview, notes, timer, and overview
//! into the presenter console window.

pub mod current_slide;
pub mod hud;
pub mod layout;
pub mod next_preview;
pub mod notes_panel;
pub mod overview;
pub mod timer;

use dais_core::bus::CommandSender;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::render_pipeline::FALLBACK_RENDER_SIZE;
use dais_document::typst_renderer::TextBoxRenderCache;

use self::current_slide::CurrentSlidePanel;
use self::layout::PresenterLayout;
use self::next_preview::NextPreviewPanel;
use self::notes_panel::{NotesPanel, NotesPanelView};
use self::overview::OverviewGrid;
use crate::audience::display::AudienceDisplay;

use crate::input::{InputHandler, UiModes};
use crate::widgets::{HelpOverlay, TextBoxTextureCache};

const MIN_LEFT_FRACTION: f32 = 0.35;
const MAX_LEFT_FRACTION: f32 = 0.8;
const MIN_TOP_FRACTION: f32 = 0.2;
const MAX_TOP_FRACTION: f32 = 0.8;
const SPLITTER_COLOR: egui::Color32 = egui::Color32::from_gray(70);
const SPLITTER_HOVER: egui::Color32 = egui::Color32::from_rgb(124, 178, 255);

/// The presenter console window — composes all sub-panels.
pub struct PresenterConsole {
    audience_display: AudienceDisplay,
    current_slide: CurrentSlidePanel,
    next_preview: NextPreviewPanel,
    notes: NotesPanel,
    overview: OverviewGrid,
    input: InputHandler,
    help: HelpOverlay,
    tb_cache: TextBoxRenderCache,
    tb_texture_cache: TextBoxTextureCache,
    left_fraction: f32,
    top_fraction: f32,
    single_left_fraction: f32,
    single_top_fraction: f32,
}

impl PresenterConsole {
    pub fn new(input: InputHandler) -> Self {
        Self {
            audience_display: AudienceDisplay::new(),
            current_slide: CurrentSlidePanel::new(),
            next_preview: NextPreviewPanel::new(),
            notes: NotesPanel::new(),
            overview: OverviewGrid::new(),
            input,
            help: HelpOverlay::new(),
            tb_cache: TextBoxRenderCache::new(),
            tb_texture_cache: TextBoxTextureCache::default(),
            left_fraction: 0.60,
            top_fraction: 0.50,
            single_left_fraction: 0.72,
            single_top_fraction: 0.40,
        }
    }

    /// Access the input handler (e.g. to share it with HUD mode).
    pub fn input_mut(&mut self) -> &mut InputHandler {
        &mut self.input
    }

    /// Render the presenter console in the given egui context.
    ///
    /// All page textures come from the cache (populated by the background
    /// render pipeline).  If a page isn't cached yet we simply skip it —
    /// the pipeline will deliver it on a subsequent frame.
    #[allow(clippy::too_many_lines)]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
    ) {
        // Help overlay intercepts input when visible.
        let help_consumed = self.help.show(ctx, self.input.keybindings());

        // Toggle help on `?` — but not while notes are being edited (? should type normally).
        let notes_editing = state.notes_editing;
        if !help_consumed && !notes_editing && Self::question_mark_pressed(ctx) {
            self.help.toggle();
        }

        // Skip normal input while help is showing.
        if !self.help.visible {
            self.input.handle_input(
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

        // Update textures from cache (single canonical size)
        let size = FALLBACK_RENDER_SIZE;
        let current_page = state.current_page;

        if let Some(page) = cache.get(current_page, size) {
            let page = page.clone();
            self.current_slide.update(ctx, &page, current_page);
        }

        let next_page =
            if current_page + 1 < state.total_pages { Some(current_page + 1) } else { None };
        if let Some(np) = next_page
            && let Some(page) = cache.get(np, size)
        {
            let page = page.clone();
            self.next_preview.update(ctx, &page, np);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(30)))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();
                let layout =
                    PresenterLayout::compute(available, self.left_fraction, self.top_fraction);

                self.handle_splitters(ui, available, &layout);

                // Current slide
                let slide_sense = if state.text_box_mode {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click_and_drag()
                };
                let (response, image_rect) =
                    self.current_slide.show_with_sense(ui, layout.current_slide, slide_sense);

                // Handle mouse on current slide
                self.input.handle_slide_mouse(
                    &response,
                    image_rect,
                    crate::input::ActiveAids {
                        ink: state.ink_active,
                        laser: state.laser_active,
                        spotlight: state.spotlight_active,
                        zoom: state.zoom_active,
                    },
                    state.zoom_region.as_ref().map(|region| region.factor),
                );

                // Draw ink strokes on presenter view
                if state.whiteboard_active {
                    // White canvas over the current slide
                    ui.painter().rect_filled(image_rect, 0.0, egui::Color32::WHITE);
                    if !state.whiteboard_strokes.is_empty() {
                        crate::widgets::draw_ink_strokes(ui, image_rect, &state.whiteboard_strokes);
                    }
                } else {
                    let page_ink = state.current_page_ink();
                    if !page_ink.is_empty() {
                        crate::widgets::draw_ink_strokes(ui, image_rect, page_ink);
                    }
                }

                if !state.whiteboard_active {
                    // Draw text boxes on presenter view (interactive in text box mode)
                    let editing_id =
                        if state.text_box_editing { state.selected_text_box } else { None };
                    let tb_cmds = crate::widgets::draw_text_boxes(
                        ui,
                        state.current_page_text_boxes(),
                        state.selected_text_box,
                        editing_id,
                        state.text_box_mode,
                        image_rect,
                        &mut self.tb_cache,
                        &mut self.tb_texture_cache,
                    );
                    for cmd in tb_cmds {
                        let _ = sender.send(cmd);
                    }
                }

                // Draw laser dot on presenter view
                if state.laser_active
                    && let Some((px, py)) = state.pointer_position
                {
                    let appearance = state.current_pointer_appearance();
                    crate::audience::overlays::draw_laser_overlay(
                        ui,
                        image_rect,
                        px,
                        py,
                        appearance.color,
                        appearance.size,
                        state.pointer_style,
                    );
                }

                // Draw spotlight on presenter view
                if state.spotlight_active
                    && let Some((sx, sy)) = state.spotlight_position
                {
                    crate::audience::overlays::draw_spotlight_overlay(
                        ui,
                        image_rect,
                        sx,
                        sy,
                        state.spotlight_radius,
                        state.spotlight_dim_opacity,
                    );
                }

                // Draw zoom target on presenter view so the operator can steer
                // the audience zoom region. This is also the natural place to
                // add future mouse-wheel zoom-factor control.
                if state.zoom_active
                    && let Some(ref region) = state.zoom_region
                {
                    crate::audience::overlays::draw_zoom_indicator(
                        ui,
                        image_rect,
                        region.center,
                        region.factor,
                    );
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
                    &NotesPanelView {
                        notes: state.current_notes.as_deref(),
                        font_size: state.notes_font_size,
                        visible: state.notes_visible,
                        editing: state.notes_editing,
                    },
                    sender,
                );

                // Status bar
                if self.show_status_bar(ui, layout.status_bar, state, sender) {
                    self.help.toggle();
                }

                // Slide overview (modal overlay)
                if state.overview_visible {
                    self.overview.show(ctx, ui, state, cache, sender);
                }

                Self::show_quit_dialog(ui, state);
            });
    }

    /// Render a single-monitor split view with the audience slide on the left
    /// and the full presenter console on the right.
    #[allow(clippy::too_many_lines)]
    pub fn show_single_monitor_split(
        &mut self,
        ctx: &egui::Context,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
        audience_render_size: dais_document::page::RenderSize,
    ) {
        // Help overlay intercepts input when visible.
        let help_consumed = self.help.show(ctx, self.input.keybindings());

        if !help_consumed && !state.notes_editing && Self::question_mark_pressed(ctx) {
            self.help.toggle();
        }

        if !self.help.visible {
            self.input.handle_input(
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

        let presenter_size = FALLBACK_RENDER_SIZE;
        let current_page = state.current_page;

        if let Some(page) = cache.get(current_page, presenter_size) {
            let page = page.clone();
            self.current_slide.update(ctx, &page, current_page);
        }

        let next_page =
            if current_page + 1 < state.total_pages { Some(current_page + 1) } else { None };
        if let Some(next_index) = next_page
            && let Some(page) = cache.get(next_index, presenter_size)
        {
            let page = page.clone();
            self.next_preview.update(ctx, &page, next_index);
        }

        let audience_page = state.audience_page();
        if let Some(page) = cache.get(audience_page, audience_render_size) {
            let page = page.clone();
            self.audience_display.update(ctx, &page, audience_page);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(30)))
            .show(ctx, |ui| {
                const VSPLIT: f32 = 8.0;
                const STATUS_H: f32 = 40.0;
                const HSPLIT: f32 = 8.0;
                let available = ui.available_rect_before_wrap();

                // Left/right split
                let left_w = (available.width() * self.single_left_fraction - VSPLIT * 0.5)
                    .clamp(200.0, available.width() - 200.0 - VSPLIT);
                let left_rect = egui::Rect::from_min_max(
                    available.min,
                    egui::pos2(available.min.x + left_w, available.max.y),
                );
                let vsplit_rect = egui::Rect::from_min_max(
                    egui::pos2(left_rect.max.x, available.min.y),
                    egui::pos2(left_rect.max.x + VSPLIT, available.max.y),
                );
                let right_rect = egui::Rect::from_min_max(
                    egui::pos2(vsplit_rect.max.x, available.min.y),
                    available.max,
                );

                // Vertical splitter (audience | presenter strip)
                let vsplit_id = ui.make_persistent_id("sm_vsplit");
                let vsplit_resp =
                    ui.interact(vsplit_rect, vsplit_id, egui::Sense::click_and_drag());
                if vsplit_resp.dragged()
                    && let Some(ptr) = vsplit_resp.interact_pointer_pos()
                {
                    self.single_left_fraction =
                        ((ptr.x - available.left()) / available.width()).clamp(0.3, 0.85);
                    ui.ctx().request_repaint();
                }
                if vsplit_resp.hovered() || vsplit_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                ui.painter().rect_filled(
                    vsplit_rect,
                    2.0,
                    if vsplit_resp.hovered() || vsplit_resp.dragged() {
                        SPLITTER_HOVER
                    } else {
                        SPLITTER_COLOR
                    },
                );

                // Left: audience slide (black, letterboxed)
                ui.painter().rect_filled(left_rect, 0.0, egui::Color32::BLACK);
                let zoom_region = if state.zoom_active {
                    state.zoom_region.as_ref().map(|r| (r.center, r.factor))
                } else {
                    None
                };
                let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
                let aud_rect = self.audience_display.show(&mut left_ui, zoom_region);
                let aud_response = left_ui.interact(
                    aud_rect,
                    egui::Id::new("sm_split_audience"),
                    egui::Sense::click_and_drag(),
                );
                self.input.handle_slide_mouse(
                    &aud_response,
                    aud_rect,
                    crate::input::ActiveAids {
                        ink: state.ink_active,
                        laser: state.laser_active,
                        spotlight: state.spotlight_active,
                        zoom: state.zoom_active,
                    },
                    state.zoom_region.as_ref().map(|r| r.factor),
                );
                if !state.whiteboard_active {
                    let editing_id =
                        if state.text_box_editing { state.selected_text_box } else { None };
                    let text_box_rect = zoom_region.map_or(aud_rect, |(center, factor)| {
                        crate::audience::overlays::zoomed_overlay_rect(aud_rect, center, factor)
                    });
                    let mut text_box_ui =
                        left_ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
                    text_box_ui.set_clip_rect(aud_rect);
                    let tb_cmds = crate::widgets::draw_text_boxes(
                        &mut text_box_ui,
                        state.current_page_text_boxes(),
                        state.selected_text_box,
                        editing_id,
                        state.text_box_mode,
                        text_box_rect,
                        &mut self.tb_cache,
                        &mut self.tb_texture_cache,
                    );
                    for cmd in tb_cmds {
                        let _ = sender.send(cmd);
                    }
                }
                crate::audience::overlays::draw_overlays(
                    &mut left_ui,
                    crate::audience::overlays::OverlayGeometry {
                        viewport_rect: left_rect,
                        image_rect: aud_rect,
                        zoom_region,
                    },
                    state,
                    &mut self.tb_cache,
                    &mut self.tb_texture_cache,
                    false,
                );

                // Right: next preview | [hsplit] | notes | status bar
                let content_h = (right_rect.height() - STATUS_H).max(0.0);
                let next_h = (content_h * self.single_top_fraction - HSPLIT * 0.5).max(60.0);
                let notes_h = (content_h - next_h - HSPLIT).max(40.0);

                let next_rect = egui::Rect::from_min_size(
                    right_rect.min,
                    egui::vec2(right_rect.width(), next_h),
                );
                let hsplit_rect = egui::Rect::from_min_size(
                    egui::pos2(right_rect.min.x, next_rect.max.y),
                    egui::vec2(right_rect.width(), HSPLIT),
                );
                let notes_rect = egui::Rect::from_min_size(
                    egui::pos2(right_rect.min.x, hsplit_rect.max.y),
                    egui::vec2(right_rect.width(), notes_h),
                );
                let status_rect = egui::Rect::from_min_size(
                    egui::pos2(right_rect.min.x, right_rect.max.y - STATUS_H),
                    egui::vec2(right_rect.width(), STATUS_H),
                );

                // Horizontal splitter (next preview | notes)
                let hsplit_id = ui.make_persistent_id("sm_hsplit");
                let hsplit_resp =
                    ui.interact(hsplit_rect, hsplit_id, egui::Sense::click_and_drag());
                if hsplit_resp.dragged()
                    && let Some(ptr) = hsplit_resp.interact_pointer_pos()
                {
                    self.single_top_fraction =
                        ((ptr.y - right_rect.top()) / content_h.max(1.0)).clamp(0.15, 0.75);
                    ui.ctx().request_repaint();
                }
                if hsplit_resp.hovered() || hsplit_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                ui.painter().rect_filled(
                    hsplit_rect,
                    2.0,
                    if hsplit_resp.hovered() || hsplit_resp.dragged() {
                        SPLITTER_HOVER
                    } else {
                        SPLITTER_COLOR
                    },
                );

                if let Some(_np) = next_page {
                    self.next_preview.show(ui, next_rect);
                } else {
                    self.next_preview.show_empty(ui, next_rect);
                }

                self.notes.show(
                    ui,
                    notes_rect,
                    &NotesPanelView {
                        notes: state.current_notes.as_deref(),
                        font_size: state.notes_font_size,
                        visible: state.notes_visible,
                        editing: state.notes_editing,
                    },
                    sender,
                );

                if self.show_status_bar(ui, status_rect, state, sender) {
                    self.help.toggle();
                }

                if state.overview_visible {
                    self.overview.show(ctx, ui, state, cache, sender);
                }

                Self::show_quit_dialog(ui, state);
            });
    }

    fn show_quit_dialog(ui: &mut egui::Ui, state: &PresentationState) {
        if !state.quit_requested {
            return;
        }

        let screen = ui.max_rect();
        ui.painter().rect_filled(screen, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));

        let dialog_size = egui::vec2(320.0, 120.0);
        let dialog_rect = egui::Rect::from_center_size(screen.center(), dialog_size);
        ui.painter().rect_filled(dialog_rect, 8.0, egui::Color32::from_gray(50));
        ui.painter().rect_stroke(
            dialog_rect,
            8.0,
            egui::Stroke::new(1.0, egui::Color32::GRAY),
            egui::StrokeKind::Outside,
        );

        ui.painter().text(
            dialog_rect.center_top() + egui::vec2(0.0, 25.0),
            egui::Align2::CENTER_CENTER,
            "Quit presentation?",
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
        ui.painter().text(
            dialog_rect.center_top() + egui::vec2(0.0, 50.0),
            egui::Align2::CENTER_CENTER,
            "Press q or Escape to confirm",
            egui::FontId::proportional(13.0),
            egui::Color32::LIGHT_GRAY,
        );
        ui.painter().text(
            dialog_rect.center_top() + egui::vec2(0.0, 75.0),
            egui::Align2::CENTER_CENTER,
            "Press any other key to cancel",
            egui::FontId::proportional(12.0),
            egui::Color32::from_gray(160),
        );
    }

    fn handle_splitters(
        &mut self,
        ui: &mut egui::Ui,
        available: egui::Rect,
        layout: &PresenterLayout,
    ) {
        let vertical_id = ui.make_persistent_id("presenter_vertical_splitter");
        let vertical_response =
            ui.interact(layout.vertical_splitter, vertical_id, egui::Sense::click_and_drag());
        if vertical_response.dragged()
            && let Some(pointer) = vertical_response.interact_pointer_pos()
        {
            self.left_fraction = ((pointer.x - available.left()) / available.width())
                .clamp(MIN_LEFT_FRACTION, MAX_LEFT_FRACTION);
            ui.ctx().request_repaint();
        }
        if vertical_response.hovered() || vertical_response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        ui.painter().rect_filled(
            layout.vertical_splitter,
            2.0,
            if vertical_response.hovered() || vertical_response.dragged() {
                SPLITTER_HOVER
            } else {
                SPLITTER_COLOR
            },
        );

        let horizontal_id = ui.make_persistent_id("presenter_horizontal_splitter");
        let horizontal_response =
            ui.interact(layout.horizontal_splitter, horizontal_id, egui::Sense::click_and_drag());
        if horizontal_response.dragged()
            && let Some(pointer) = horizontal_response.interact_pointer_pos()
        {
            self.top_fraction = ((pointer.y - available.top())
                / (available.height() - 40.0).max(1.0))
            .clamp(MIN_TOP_FRACTION, MAX_TOP_FRACTION);
            ui.ctx().request_repaint();
        }
        if horizontal_response.hovered() || horizontal_response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        ui.painter().rect_filled(
            layout.horizontal_splitter,
            2.0,
            if horizontal_response.hovered() || horizontal_response.dragged() {
                SPLITTER_HOVER
            } else {
                SPLITTER_COLOR
            },
        );
    }

    #[allow(clippy::too_many_lines)]
    fn show_status_bar(
        &self,
        ui: &mut egui::Ui,
        area: egui::Rect,
        state: &PresentationState,
        sender: &CommandSender,
    ) -> bool {
        let mut help_clicked = false;
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));

        // Background
        child_ui.painter().rect_filled(area, 0.0, egui::Color32::from_gray(20));

        child_ui.scope_builder(egui::UiBuilder::new().max_rect(area.shrink(4.0)), |ui| {
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

                // Timer (clickable)
                if timer::show_timer(ui, &state.timer) {
                    let _ = sender.send(dais_core::commands::Command::ToggleTimer);
                }

                ui.separator();

                timer::show_slide_timer(ui, state.slide_elapsed);

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
                if state.text_box_mode {
                    indicators.push(("[X]Text", egui::Color32::from_rgb(180, 130, 255)));
                }

                for (text, color) in indicators {
                    ui.colored_label(color, egui::RichText::new(text).size(12.0));
                }
                if state.ink_active {
                    let p = state.active_pen.color;
                    let swatch = egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]);
                    let gray = egui::Color32::from_gray(180);
                    ui.colored_label(swatch, egui::RichText::new("■").size(12.0));
                    ui.colored_label(
                        gray,
                        egui::RichText::new(format!("{}px", state.active_pen.width)).size(11.0),
                    );
                }

                // Jump mode indicator
                if self.input.mode() == crate::input::InputMode::JumpToSlide {
                    let buf = self.input.jump_buffer();
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        egui::RichText::new(format!("Go to: {buf}_")).size(14.0),
                    );
                }

                // Help button — right-aligned
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("?")
                                    .size(15.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_gray(50))
                            .corner_radius(4.0)
                            .min_size(egui::vec2(26.0, 26.0)),
                        )
                        .on_hover_text("Keyboard shortcuts")
                        .clicked()
                    {
                        help_clicked = true;
                    }
                });
            });
        });

        help_clicked
    }

    /// Check whether a `?` text event was produced this frame.
    fn question_mark_pressed(ctx: &egui::Context) -> bool {
        ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "?")))
    }
}
