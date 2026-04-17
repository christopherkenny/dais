//! Timer display widget.
//!
//! Shows elapsed/countdown time with color coding based on `TimerPhase`.
//! Clickable to toggle start/pause.

use dais_core::state::{TimerPhase, TimerState};

/// Render the timer in a status-bar area. Returns true if clicked (toggle).
pub fn show_timer(ui: &mut egui::Ui, timer: &TimerState) -> bool {
    let display = timer.display_time();
    let secs = display.as_secs();
    let mins = secs / 60;
    let remaining_secs = secs % 60;
    let time_str = format!("{mins:02}:{remaining_secs:02}");

    let phase = timer.phase();
    let color = match phase {
        TimerPhase::Normal => egui::Color32::WHITE,
        TimerPhase::Warning => egui::Color32::YELLOW,
        TimerPhase::Overrun => egui::Color32::from_rgb(255, 80, 80),
    };

    let total_secs = timer.duration.as_secs();
    let total_mins = total_secs / 60;
    let total_remaining = total_secs % 60;
    let total_str = format!("{total_mins:02}:{total_remaining:02}");

    let running_icon = if timer.running { "▶" } else { "⏸" };

    let label = format!("{running_icon} {time_str} / {total_str}");

    let response = ui.add(
        egui::Label::new(egui::RichText::new(label).size(16.0).color(color))
            .sense(egui::Sense::click()),
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response.clicked()
}
