//! Test-input diagnostic mode.
//!
//! A minimal `eframe::App` that displays key events and their mapped actions,
//! useful for debugging clicker/remote hardware setups.

use std::collections::VecDeque;

use dais_core::keybindings::{KeyCombo, KeybindingMap};

/// Maximum number of key events to display.
const MAX_EVENTS: usize = 20;

/// A recorded key event with its resolved action (if any).
struct KeyEvent {
    key_name: String,
    modifiers: String,
    action: String,
}

/// Standalone eframe app for the `--test-input` diagnostic mode.
pub struct TestInputApp {
    keybindings: KeybindingMap,
    events: VecDeque<KeyEvent>,
}

impl TestInputApp {
    pub fn new(keybindings: KeybindingMap) -> Self {
        Self { keybindings, events: VecDeque::with_capacity(MAX_EVENTS + 1) }
    }
}

impl eframe::App for TestInputApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Collect key events this frame
        let raw_events: Vec<egui::Event> = ctx.input(|i| i.events.clone());

        for event in &raw_events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                let key_name = crate::input::egui_key_name_public(*key);
                let combo = KeyCombo {
                    key: key_name.clone(),
                    shift: modifiers.shift,
                    ctrl: modifiers.ctrl || modifiers.command,
                    alt: modifiers.alt,
                };

                let mod_str = format_modifiers(*modifiers);
                let action = match self.keybindings.lookup(&combo) {
                    Some(a) => a.config_name().to_string(),
                    None => "(none)".to_string(),
                };

                tracing::info!(
                    key = %key_name,
                    modifiers = %mod_str,
                    action = %action,
                    "Key event captured"
                );

                self.events.push_front(KeyEvent { key_name, modifiers: mod_str, action });
                if self.events.len() > MAX_EVENTS {
                    self.events.pop_back();
                }
            }
        }

        // Check for Escape to exit
        let wants_exit = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Test Input Mode — Press keys to see events");
            ui.separator();

            if self.events.is_empty() {
                ui.label("No key events yet. Press any key on your clicker or keyboard.");
            } else {
                egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).show(
                    ui,
                    |ui| {
                        egui::Grid::new("key_events")
                            .num_columns(3)
                            .striped(true)
                            .spacing([20.0, 4.0])
                            .show(ui, |ui| {
                                ui.strong("Key");
                                ui.strong("Modifiers");
                                ui.strong("Action");
                                ui.end_row();

                                for ev in &self.events {
                                    ui.monospace(&ev.key_name);
                                    ui.monospace(&ev.modifiers);
                                    ui.monospace(&ev.action);
                                    ui.end_row();
                                }
                            });
                    },
                );
            }

            ui.separator();
            if ui.button("Exit").clicked() || wants_exit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

/// Format modifier flags into a human-readable string.
fn format_modifiers(m: egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if m.ctrl || m.command {
        parts.push("Ctrl");
    }
    if m.shift {
        parts.push("Shift");
    }
    if m.alt {
        parts.push("Alt");
    }
    if parts.is_empty() { "(none)".to_string() } else { parts.join("+") }
}
