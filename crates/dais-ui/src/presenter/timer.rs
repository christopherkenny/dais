//! Timer display widget.
//!
//! Shows elapsed/countdown time with color coding based on `TimerPhase`.
//! Clickable to toggle start/pause.

use dais_core::state::{TimerPhase, TimerState};

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let mins = secs / 60;
    let remaining_secs = secs % 60;
    format!("{mins:02}:{remaining_secs:02}")
}

/// Render the timer in a status-bar area. Returns true if clicked (toggle).
pub fn show_timer(ui: &mut egui::Ui, timer: &TimerState) -> bool {
    let time_str = format_duration(timer.display_time());

    let phase = timer.phase();
    let color = match phase {
        TimerPhase::Normal => egui::Color32::WHITE,
        TimerPhase::Warning => egui::Color32::YELLOW,
        TimerPhase::Overrun => egui::Color32::from_rgb(255, 80, 80),
    };

    let running_icon = if timer.running { "⏸" } else { "▶" };
    let label = if let Some(duration) = timer.duration {
        format!("{running_icon} {time_str} / {}", format_duration(duration))
    } else {
        format!("{running_icon} {time_str}")
    };

    let response = ui.add(
        egui::Label::new(egui::RichText::new(label).size(16.0).color(color))
            .sense(egui::Sense::click()),
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response.clicked()
}

/// Render the per-slide elapsed timer.
pub fn show_slide_timer(
    ui: &mut egui::Ui,
    elapsed: std::time::Duration,
    target: Option<std::time::Duration>,
) {
    let over_target = target.is_some_and(|duration| elapsed > duration);
    let color =
        if over_target { egui::Color32::from_rgb(255, 80, 80) } else { egui::Color32::LIGHT_GRAY };
    let label = if let Some(target) = target {
        format!("Slide {} / {}", format_duration(elapsed), format_duration(target))
    } else {
        format!("Slide {}", format_duration(elapsed))
    };

    ui.label(egui::RichText::new(label).size(14.0).color(color));
}
