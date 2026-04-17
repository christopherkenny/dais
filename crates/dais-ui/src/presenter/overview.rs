//! Slide overview grid.
//!
//! Modal overlay showing a grid of slide thumbnails with keyboard navigation.

use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::state::PresentationState;
use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::source::DocumentSource;

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
        Self {
            thumbnails: Vec::new(),
            selected: 0,
            columns: 4,
        }
    }

    /// Show the overview grid as a full-window overlay.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        state: &PresentationState,
        doc: &dyn DocumentSource,
        cache: &mut PageCache,
        sender: &CommandSender,
    ) {
        if !state.overview_visible {
            return;
        }

        self.selected = state.current_logical_slide;

        // Ensure we have enough thumbnails
        while self.thumbnails.len() < state.total_logical_slides {
            self.thumbnails.push(SlideThumbnail::new());
        }

        let available = ui.available_rect_before_wrap();

        // Paint semi-transparent background
        ui.painter().rect_filled(
            available,
            0.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220),
        );

        // Compute grid layout
        self.columns =
            ((available.width() / (THUMB_WIDTH + THUMB_PADDING * 2.0)) as usize).max(1);

        // Handle keyboard navigation
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
                NavigateDir::Right => {
                    if self.selected + 1 < state.total_logical_slides {
                        self.selected += 1;
                    }
                }
                NavigateDir::Left => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                NavigateDir::Down => {
                    let next = self.selected + self.columns;
                    if next < state.total_logical_slides {
                        self.selected = next;
                    }
                }
                NavigateDir::Up => {
                    if self.selected >= self.columns {
                        self.selected -= self.columns;
                    }
                }
                NavigateDir::Select => {
                    let _ = sender.send(Command::GoToSlide(self.selected));
                    let _ = sender.send(Command::ToggleSlideOverview);
                }
                NavigateDir::Close => {
                    let _ = sender.send(Command::ToggleSlideOverview);
                }
            }
        }

        // Render grid
        let render_size =
            RenderSize { width: THUMB_WIDTH as u32, height: THUMB_HEIGHT as u32 };

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing =
                    egui::vec2(THUMB_PADDING, THUMB_PADDING);

                for i in 0..state.total_logical_slides {
                    let first_page = state
                        .slide_groups
                        .get(i)
                        .and_then(|g| g.pages.first().copied())
                        .unwrap_or(i);

                    // Render page if not cached
                    if cache.get(first_page, render_size).is_none() {
                        if let Ok(rendered) = doc.render_page(first_page, render_size) {
                            cache.insert(first_page, render_size, rendered);
                        }
                    }

                    if let Some(page) = cache.get(first_page, render_size) {
                        self.thumbnails[i].update(ctx, page, first_page);
                    }

                    let desired = egui::vec2(THUMB_WIDTH, THUMB_HEIGHT + 20.0);
                    let (rect, response) =
                        ui.allocate_exact_size(desired, egui::Sense::click());

                    // Draw thumbnail
                    let thumb_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(THUMB_WIDTH, THUMB_HEIGHT),
                    );
                    let mut thumb_ui = ui.new_child(
                        egui::UiBuilder::new().max_rect(thumb_rect),
                    );
                    self.thumbnails[i]
                        .show(&mut thumb_ui, egui::vec2(THUMB_WIDTH, THUMB_HEIGHT));

                    // Highlight selected
                    if i == self.selected {
                        ui.painter().rect_stroke(
                            thumb_rect,
                            2.0,
                            egui::Stroke::new(3.0, egui::Color32::LIGHT_BLUE),
                        );
                    }

                    // Slide number label
                    let label_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2(0.0, THUMB_HEIGHT),
                        egui::vec2(THUMB_WIDTH, 20.0),
                    );
                    ui.painter().text(
                        label_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}", i + 1),
                        egui::FontId::proportional(12.0),
                        egui::Color32::LIGHT_GRAY,
                    );

                    // Click to select
                    if response.clicked() {
                        let _ = sender.send(Command::GoToSlide(i));
                        let _ = sender.send(Command::ToggleSlideOverview);
                    }
                }
            });
        });
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
