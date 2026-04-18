//! Lightweight toast notification system.
//!
//! Queue messages with a severity level. The `ToastManager` renders them
//! as a stack of auto-dismissing banners at the top-right of the frame.

use std::time::{Duration, Instant};

const DEFAULT_DURATION: Duration = Duration::from_secs(4);
const TOAST_WIDTH: f32 = 300.0;
const TOAST_ROUNDING: f32 = 6.0;
const TOAST_PADDING: f32 = 10.0;
const TOAST_SPACING: f32 = 6.0;
const MARGIN_TOP: f32 = 8.0;
const MARGIN_RIGHT: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warning,
    Error,
}

impl ToastLevel {
    fn bg_color(self) -> egui::Color32 {
        match self {
            Self::Info => egui::Color32::from_rgba_unmultiplied(30, 100, 200, 220),
            Self::Warning => egui::Color32::from_rgba_unmultiplied(200, 160, 20, 220),
            Self::Error => egui::Color32::from_rgba_unmultiplied(200, 40, 40, 220),
        }
    }

    fn text_color() -> egui::Color32 {
        egui::Color32::WHITE
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created: Instant,
    pub duration: Duration,
}

pub struct ToastManager {
    toasts: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn push(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.push_with_duration(level, message, DEFAULT_DURATION);
    }

    pub fn push_with_duration(
        &mut self,
        level: ToastLevel,
        message: impl Into<String>,
        duration: Duration,
    ) {
        self.toasts.push(Toast {
            message: message.into(),
            level,
            created: Instant::now(),
            duration,
        });
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.toasts.len()
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.created.elapsed() < t.duration);

        if self.toasts.is_empty() {
            return;
        }

        // Request repaints while toasts are visible so they auto-dismiss.
        ctx.request_repaint_after(Duration::from_millis(250));

        let screen = ctx.screen_rect();
        let anchor_x = screen.max.x - MARGIN_RIGHT;
        let mut y = screen.min.y + MARGIN_TOP;

        let mut dismiss: Option<usize> = None;

        for (i, toast) in self.toasts.iter().enumerate() {
            let area_id = egui::Id::new("toast").with(i);

            egui::Area::new(area_id)
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(anchor_x - TOAST_WIDTH, y))
                .interactable(true)
                .show(ctx, |ui| {
                    let frame = egui::Frame::new()
                        .fill(toast.level.bg_color())
                        .corner_radius(TOAST_ROUNDING)
                        .inner_margin(TOAST_PADDING);

                    let response = frame.show(ui, |ui| {
                        ui.set_max_width(TOAST_WIDTH - TOAST_PADDING * 2.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&toast.message)
                                        .color(ToastLevel::text_color()),
                                )
                                .wrap(),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("×")
                                                .color(ToastLevel::text_color())
                                                .strong(),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    dismiss = Some(i);
                                }
                            });
                        });
                    });

                    y += response.response.rect.height() + TOAST_SPACING;
                });
        }

        if let Some(idx) = dismiss {
            self.toasts.remove(idx);
        }
    }
}

impl Default for ToastManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_push_and_count() {
        let mut mgr = ToastManager::new();
        mgr.push(ToastLevel::Info, "hello");
        mgr.push(ToastLevel::Warning, "warn");
        mgr.push(ToastLevel::Error, "err");
        assert_eq!(mgr.len(), 3);
    }

    #[test]
    fn toast_auto_expires() {
        let mut mgr = ToastManager::new();
        mgr.push_with_duration(ToastLevel::Info, "brief", Duration::from_millis(0));
        // Duration is zero so the toast is already expired.
        std::thread::sleep(Duration::from_millis(1));
        mgr.toasts.retain(|t| t.created.elapsed() < t.duration);
        assert_eq!(mgr.len(), 0);
    }
}
