//! Notes panel with Markdown rendering.
//!
//! Renders the current slide's speaker notes using `egui_commonmark`.

use dais_core::bus::CommandSender;
use dais_core::commands::Command;

pub struct NotesPanelView<'a> {
    pub notes: Option<&'a str>,
    pub font_size: f32,
    pub visible: bool,
    pub editing: bool,
}

/// Presenter notes panel.
pub struct NotesPanel {
    cache: egui_commonmark::CommonMarkCache,
}

impl NotesPanel {
    pub fn new() -> Self {
        Self { cache: egui_commonmark::CommonMarkCache::default() }
    }

    /// Render the notes panel in the given area.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        area: egui::Rect,
        view: &NotesPanelView<'_>,
        sender: &CommandSender,
    ) {
        if !view.visible {
            return;
        }

        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(area));

        child_ui.allocate_new_ui(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(area.min, egui::vec2(area.width(), 20.0))),
            |ui| {
                ui.colored_label(egui::Color32::GRAY, "Notes");
            },
        );

        let content_area = egui::Rect::from_min_size(
            area.min + egui::vec2(8.0, 24.0),
            egui::vec2((area.width() - 16.0).max(1.0), (area.height() - 28.0).max(1.0)),
        );

        child_ui.allocate_new_ui(egui::UiBuilder::new().max_rect(content_area), |ui| {
            // Apply font size override
            ui.style_mut().override_font_id = Some(egui::FontId::proportional(view.font_size));

            if view.editing {
                let mut buffer = view.notes.unwrap_or_default().to_string();
                let edit = egui::TextEdit::multiline(&mut buffer)
                    .desired_width(f32::INFINITY)
                    .desired_rows(12)
                    .hint_text("Write speaker notes in Markdown");
                let response = ui.add_sized(content_area.size(), edit);
                if response.changed() {
                    let _ = sender.send(Command::SetCurrentSlideNotes(buffer));
                }
            } else {
                egui::ScrollArea::vertical().max_height(content_area.height()).show(ui, |ui| {
                    if let Some(text) = view.notes {
                        egui_commonmark::CommonMarkViewer::new().show(ui, &mut self.cache, text);
                    } else {
                        ui.colored_label(egui::Color32::from_gray(80), "No notes for this slide");
                    }
                });
            }
        });
    }
}

impl Default for NotesPanel {
    fn default() -> Self {
        Self::new()
    }
}
