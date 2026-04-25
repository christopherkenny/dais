//! Keybinding help overlay.
//!
//! A dismissible popup that lists the active keybindings grouped by category.
//! Toggled with `?` (button or key) and dismissed with `?`, Escape, or the
//! close button.

use dais_core::keybindings::{Action, KeybindingMap};

const OVERLAY_BG: egui::Color32 = egui::Color32::BLACK;
const HEADER_COLOR: egui::Color32 = egui::Color32::from_rgb(124, 178, 255);
const TEXT_COLOR: egui::Color32 = egui::Color32::WHITE;
const WINDOW_WIDTH: f32 = 560.0;
const MAX_HEIGHT_FRACTION: f32 = 0.80;
const SCROLLBAR_GUTTER: f32 = 22.0;
const ROW_GAP: f32 = 12.0;
const KEY_COLUMN_WIDTH: f32 = 200.0;
const HEADER_BUTTON_SIZE: f32 = 28.0;

/// Persistent state for the help overlay.
#[derive(Default)]
pub struct HelpOverlay {
    pub visible: bool,
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle visibility and return `true` when visible after the toggle.
    pub fn toggle(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }

    /// Render the overlay. Call once per frame; it will only draw when visible.
    ///
    /// Returns `true` if the overlay consumed the `?` key this frame (so the
    /// caller can suppress further key handling).
    pub fn show(&mut self, ctx: &egui::Context, keybindings: &KeybindingMap) -> bool {
        if !self.visible {
            return false;
        }

        // Check for dismiss keys before drawing so the overlay can close on
        // the same frame the key is pressed.
        let dismiss = ctx.input(|i| {
            i.events.iter().any(|e| match e {
                egui::Event::Text(t) => t == "?",
                egui::Event::Key { key: egui::Key::Escape, pressed: true, .. } => true,
                _ => false,
            })
        });

        if dismiss {
            self.visible = false;
            return true;
        }

        let screen = ctx.content_rect();
        let max_h = screen.height() * MAX_HEIGHT_FRACTION;

        egui::Area::new(egui::Id::new("help_overlay_bg"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    screen,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                );
            });

        let mut still_open = true;
        egui::Window::new("help_overlay")
            .open(&mut still_open)
            .enabled(true)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(WINDOW_WIDTH)
            .min_width(WINDOW_WIDTH)
            .max_width(WINDOW_WIDTH)
            .max_height(max_h)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(OVERLAY_BG)
                    .corner_radius(10.0)
                    .inner_margin(16.0),
            )
            .show(ctx, |ui| {
                let visuals = &mut ui.style_mut().visuals;
                visuals.override_text_color = Some(TEXT_COLOR);
                visuals.widgets.noninteractive.fg_stroke.color = TEXT_COLOR;
                visuals.widgets.inactive.fg_stroke.color = TEXT_COLOR;
                visuals.widgets.hovered.fg_stroke.color = TEXT_COLOR;
                visuals.widgets.active.fg_stroke.color = TEXT_COLOR;
                visuals.widgets.open.fg_stroke.color = TEXT_COLOR;
                visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;
                visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(20);
                visuals.widgets.active.bg_fill = egui::Color32::from_gray(28);
                visuals.widgets.noninteractive.bg_fill = egui::Color32::BLACK;
                visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::BLACK;
                visuals.widgets.inactive.weak_bg_fill = egui::Color32::BLACK;
                visuals.widgets.hovered.weak_bg_fill = egui::Color32::BLACK;
                visuals.widgets.active.weak_bg_fill = egui::Color32::BLACK;
                visuals.widgets.open.weak_bg_fill = egui::Color32::BLACK;

                Self::render_header(ui, &mut self.visible);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(10.0);

                Self::render_table(ui, keybindings);
            });

        if !still_open {
            self.visible = false;
        }

        true
    }

    fn render_table(ui: &mut egui::Ui, keybindings: &KeybindingMap) {
        let bindings = keybindings.action_bindings();
        let content_width = (ui.available_width() - SCROLLBAR_GUTTER).max(0.0);
        let key_width = KEY_COLUMN_WIDTH.min((content_width * 0.42).max(170.0));
        let action_width = (content_width - key_width - ROW_GAP).max(160.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(content_width);

            let mut idx = 0;
            while idx < bindings.len() {
                let group = bindings[idx].0.group();

                if idx > 0 {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                }

                ui.label(egui::RichText::new(group).size(13.0).color(HEADER_COLOR).strong());
                ui.add_space(4.0);

                egui::Grid::new(("help_overlay_group", group))
                    .num_columns(2)
                    .min_col_width(0.0)
                    .max_col_width(f32::INFINITY)
                    .spacing(egui::vec2(ROW_GAP, 6.0))
                    .show(ui, |ui| {
                        while idx < bindings.len() && bindings[idx].0.group() == group {
                            let (action, keys) = &bindings[idx];
                            Self::render_row(ui, *action, keys, action_width, key_width);
                            ui.end_row();
                            idx += 1;
                        }
                    });
            }
        });
    }

    fn render_header(ui: &mut egui::Ui, visible: &mut bool) {
        let total_width = ui.available_width();
        let center_width = (total_width - HEADER_BUTTON_SIZE * 2.0).max(0.0);

        ui.horizontal(|ui| {
            ui.allocate_space(egui::vec2(HEADER_BUTTON_SIZE, HEADER_BUTTON_SIZE));

            ui.allocate_ui_with_layout(
                egui::vec2(center_width, HEADER_BUTTON_SIZE),
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.label(
                        egui::RichText::new("Keyboard Shortcuts")
                            .size(20.0)
                            .strong()
                            .color(TEXT_COLOR),
                    );
                },
            );

            let close = ui.add_sized(
                egui::vec2(HEADER_BUTTON_SIZE, HEADER_BUTTON_SIZE),
                egui::Button::new(egui::RichText::new("X").size(18.0).color(TEXT_COLOR))
                    .frame(false),
            );
            if close.clicked() {
                *visible = false;
            }
        });
    }

    fn render_row(
        ui: &mut egui::Ui,
        action: Action,
        keys: &[String],
        action_width: f32,
        key_width: f32,
    ) {
        let key_text = if keys.is_empty() { "—".to_string() } else { keys.join("  /  ") };

        ui.allocate_ui_with_layout(
            egui::vec2(action_width, ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(action.description())
                            .size(13.0)
                            .strong()
                            .color(TEXT_COLOR),
                    )
                    .sense(egui::Sense::hover()),
                );
            },
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(key_width, ui.spacing().interact_size.y),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(key_text)
                                .size(12.5)
                                .strong()
                                .color(TEXT_COLOR)
                                .family(egui::FontFamily::Monospace),
                        )
                        .sense(egui::Sense::hover()),
                    );
                },
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips_visibility() {
        let mut overlay = HelpOverlay::new();
        assert!(!overlay.visible);
        assert!(overlay.toggle());
        assert!(overlay.visible);
        assert!(!overlay.toggle());
        assert!(!overlay.visible);
    }

    #[test]
    fn action_bindings_covers_all_actions() {
        let map = KeybindingMap::from_config(&std::collections::HashMap::new());
        let bindings = map.action_bindings();
        assert_eq!(bindings.len(), Action::all().len());
    }

    #[test]
    fn every_action_has_description_and_group() {
        for action in Action::all() {
            assert!(!action.description().is_empty(), "{action:?} missing description");
            assert!(!action.group().is_empty(), "{action:?} missing group");
        }
    }
}
