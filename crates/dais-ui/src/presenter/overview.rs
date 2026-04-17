//! Slide overview grid.
//!
//! Modal overlay showing a grid of slide thumbnails with keyboard navigation.

use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::render_pipeline::CANONICAL_RENDER_SIZE;

use crate::widgets::SlideThumbnail;

/// Slide overview grid overlay.
pub struct OverviewGrid {
    thumbnails: Vec<SlideThumbnail>,
    selected: usize,
    columns: usize,
}

/// Target thumbnail size for overview grid items.
const THUMB_WIDTH: f32 = 200.0;
const THUMB_HEIGHT: f32 = 150.0;
const THUMB_PADDING: f32 = 8.0;

impl OverviewGrid {
    pub fn new() -> Self {
        Self { thumbnails: Vec::new(), selected: 0, columns: 4 }
    }

    /// Show the overview grid as a full-window overlay.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
    ) {
        if !state.overview_visible {
            return;
        }

        self.selected = state.current_logical_slide;

        while self.thumbnails.len() < state.total_logical_slides {
            self.thumbnails.push(SlideThumbnail::new());
        }

        let available = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            available,
            0.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220),
        );

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        {
            self.columns =
                ((available.width() / (THUMB_WIDTH + THUMB_PADDING * 2.0)) as usize).max(1);
        }

        self.handle_navigation(ctx, state, sender);
        self.render_grid(ctx, ui, state, cache, sender);
    }

    fn handle_navigation(
        &mut self,
        ctx: &egui::Context,
        state: &PresentationState,
        sender: &CommandSender,
    ) {
        let navigate_cmd = ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowRight) {
                Some(NavigateDir::Right)
            } else if i.key_pressed(egui::Key::ArrowLeft) {
                Some(NavigateDir::Left)
            } else if i.key_pressed(egui::Key::ArrowDown) {
                Some(NavigateDir::Down)
            } else if i.key_pressed(egui::Key::ArrowUp) {
                Some(NavigateDir::Up)
            } else if i.key_pressed(egui::Key::Enter) {
                Some(NavigateDir::Select)
            } else if i.key_pressed(egui::Key::Escape) {
                Some(NavigateDir::Close)
            } else {
                None
            }
        });

        if let Some(dir) = navigate_cmd {
            match dir {
                NavigateDir::Right if self.selected + 1 < state.total_logical_slides => {
                    self.selected += 1;
                }
                NavigateDir::Left if self.selected > 0 => {
                    self.selected -= 1;
                }
                NavigateDir::Down if self.selected + self.columns < state.total_logical_slides => {
                    self.selected += self.columns;
                }
                NavigateDir::Up if self.selected >= self.columns => {
                    self.selected -= self.columns;
                }
                NavigateDir::Select => {
                    let _ = sender.send(Command::GoToSlide(self.selected));
                    let _ = sender.send(Command::ToggleSlideOverview);
                }
                NavigateDir::Close => {
                    let _ = sender.send(Command::ToggleSlideOverview);
                }
                _ => {}
            }
        }
    }

    fn render_grid(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
    ) {
        let render_size = CANONICAL_RENDER_SIZE;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(THUMB_PADDING, THUMB_PADDING);

                for i in 0..state.total_logical_slides {
                    self.render_thumbnail(ctx, ui, i, state, cache, sender, render_size);
                }
            });
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_thumbnail(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        index: usize,
        state: &PresentationState,
        cache: &mut PageCache,
        sender: &CommandSender,
        render_size: dais_document::page::RenderSize,
    ) {
        let first_page =
            state.slide_groups.get(index).and_then(|g| g.pages.first().copied()).unwrap_or(index);

        // Just read from cache — the pipeline will populate it
        if let Some(page) = cache.get(first_page, render_size) {
            self.thumbnails[index].update(ctx, page, first_page);
        }

        let desired = egui::vec2(THUMB_WIDTH, THUMB_HEIGHT + 20.0);
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

        let thumb_rect = egui::Rect::from_min_size(rect.min, egui::vec2(THUMB_WIDTH, THUMB_HEIGHT));
        let mut thumb_ui = ui.new_child(egui::UiBuilder::new().max_rect(thumb_rect));
        self.thumbnails[index].show(&mut thumb_ui, egui::vec2(THUMB_WIDTH, THUMB_HEIGHT));

        if index == self.selected {
            ui.painter().rect_stroke(
                thumb_rect,
                2.0,
                egui::Stroke::new(3.0, egui::Color32::LIGHT_BLUE),
                egui::StrokeKind::Outside,
            );
        }

        let label_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(0.0, THUMB_HEIGHT),
            egui::vec2(THUMB_WIDTH, 20.0),
        );
        ui.painter().text(
            label_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}", index + 1),
            egui::FontId::proportional(12.0),
            egui::Color32::LIGHT_GRAY,
        );

        if response.clicked() {
            let _ = sender.send(Command::GoToSlide(index));
            let _ = sender.send(Command::ToggleSlideOverview);
        }
    }
}

impl Default for OverviewGrid {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
enum NavigateDir {
    Left,
    Right,
    Up,
    Down,
    Select,
    Close,
}
