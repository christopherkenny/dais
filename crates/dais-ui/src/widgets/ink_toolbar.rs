//! Ink drawing toolbar: tool type selector, colour swatches, and width buttons.

use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::state::{DrawTool, PresentationState};

/// Width presets for the pen tool (logical pixels).
const PEN_WIDTHS: &[(f32, &str)] = &[(2.0, "Thin"), (4.0, "Med"), (8.0, "Thick")];

/// Width presets for the highlighter tool (logical pixels).
const HIGHLIGHTER_WIDTHS: &[(f32, &str)] = &[(6.0, "Thin"), (12.0, "Med"), (20.0, "Thick")];

/// Radius presets for the eraser tool (normalized slide coordinates).
const ERASER_WIDTHS: &[(f32, &str)] = &[(0.015, "Thin"), (0.03, "Med"), (0.06, "Thick")];

/// Draw the ink toolbar inline inside an [`egui::Ui`].
///
/// Shows a [Pen | Highlighter | Eraser] tool selector, then colour swatches and
/// width buttons appropriate for the active tool.  Returns without rendering
/// anything when ink is not active.
pub fn show_ink_toolbar(ui: &mut egui::Ui, state: &PresentationState, sender: &CommandSender) {
    if !state.ink_active {
        return;
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

        // --- Tool type selector ---
        for (tool, label) in
            [(DrawTool::Pen, "Pen"), (DrawTool::Highlighter, "Hi"), (DrawTool::Eraser, "Erase")]
        {
            let is_active = state.draw_tool == tool;
            let btn =
                egui::Button::new(egui::RichText::new(label).size(11.0).color(if is_active {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::WHITE
                }))
                .fill(if is_active {
                    egui::Color32::from_gray(200)
                } else {
                    egui::Color32::from_gray(45)
                })
                .min_size(egui::vec2(38.0, 20.0));
            if ui.add(btn).clicked() && !is_active {
                let _ = sender.send(Command::SetDrawTool(tool));
            }
        }

        ui.separator();

        if state.draw_tool == DrawTool::Eraser {
            // Eraser: width buttons only, no colour swatches.
            show_width_buttons(ui, ERASER_WIDTHS, state.eraser_radius, sender);
        } else {
            // Pen / Highlighter: colour swatches then width buttons.
            let (presets, active_color_bytes, active_width) = match state.draw_tool {
                DrawTool::Highlighter => (
                    state.highlighter_color_presets.as_slice(),
                    state.active_highlighter.color,
                    state.active_highlighter.width,
                ),
                _ => (
                    state.ink_color_presets.as_slice(),
                    state.active_pen.color,
                    state.active_pen.width,
                ),
            };

            let active_color = egui::Color32::from_rgba_unmultiplied(
                active_color_bytes[0],
                active_color_bytes[1],
                active_color_bytes[2],
                active_color_bytes[3],
            );

            for &preset in presets {
                let color = egui::Color32::from_rgba_unmultiplied(
                    preset[0], preset[1], preset[2], preset[3],
                );
                let is_active = preset == active_color_bytes;
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                if response.clicked() {
                    let _ = sender.send(Command::SetInkColor(preset));
                }
                let painter = ui.painter();
                painter.circle_filled(rect.center(), 8.0, color);
                if is_active {
                    painter.circle_stroke(
                        rect.center(),
                        9.0,
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                    );
                }
                response.on_hover_text(color_label(preset));
            }

            ui.separator();

            let widths = match state.draw_tool {
                DrawTool::Highlighter => HIGHLIGHTER_WIDTHS,
                _ => PEN_WIDTHS,
            };
            show_width_buttons_colored(ui, widths, active_width, active_color, sender);
        }
    });
}

fn show_width_buttons(
    ui: &mut egui::Ui,
    widths: &[(f32, &str)],
    active_width: f32,
    sender: &CommandSender,
) {
    show_width_buttons_colored(ui, widths, active_width, egui::Color32::from_gray(200), sender);
}

fn show_width_buttons_colored(
    ui: &mut egui::Ui,
    widths: &[(f32, &str)],
    active_width: f32,
    active_color: egui::Color32,
    sender: &CommandSender,
) {
    for &(width, label) in widths {
        let is_active = (active_width - width).abs() < width * 0.01 + 0.0001;
        let btn = egui::Button::new(egui::RichText::new(label).size(11.0).color(if is_active {
            egui::Color32::BLACK
        } else {
            egui::Color32::WHITE
        }))
        .fill(if is_active { active_color } else { egui::Color32::from_gray(45) })
        .min_size(egui::vec2(34.0, 20.0));
        if ui.add(btn).clicked() {
            let _ = sender.send(Command::SetInkWidth(width));
        }
    }
}

fn color_label(rgba: [u8; 4]) -> &'static str {
    match rgba {
        [220, 30, 30, 255] | [255, 0, 0, 255] => "Red",
        [30, 100, 220, 255] | [0, 110, 255, 255] => "Blue",
        [30, 180, 30, 255] | [0, 190, 60, 255] => "Green",
        [220, 200, 0, 255] | [255, 210 | 220, 0, _] => "Yellow",
        [60, 220, 60, _] => "Green",
        [0, 200, 255, _] => "Cyan",
        [255, 80, 180, _] => "Pink",
        [255, 255, 255, 255] => "White",
        [0, 0, 0, 255] => "Black",
        _ => "Color",
    }
}
