//! Input handling — mode-aware key/mouse → Command pipeline.
//!
//! Converts egui key/mouse events into [`Command`]s dispatched via the
//! [`CommandBus`].

use std::time::Instant;

use dais_core::bus::CommandSender;
use dais_core::commands::Command;
use dais_core::keybindings::{Action, KeyCombo, KeybindingMap};

/// Which input mode the presenter console is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Default keyboard-driven navigation.
    Normal,
    /// Slide overview grid is showing; Enter selects, Escape closes.
    Overview,
    /// Freehand ink drawing; mouse drives strokes.
    Ink,
    /// Laser pointer active; mouse drives position.
    Laser,
    /// Inline markdown notes editing.
    NotesEdit,
    /// Digit accumulation for jump-to-slide (G → digits → Enter).
    JumpToSlide,
}

/// Which presentation aids are currently active, used to drive mouse handling.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveAids {
    pub ink: bool,
    pub laser: bool,
    pub spotlight: bool,
    pub zoom: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UiModes {
    pub overview_visible: bool,
    pub ink_active: bool,
    pub laser_active: bool,
    pub notes_editing: bool,
}

/// Processes egui events and dispatches [`Command`]s.
pub struct InputHandler {
    sender: CommandSender,
    keybindings: KeybindingMap,
    mode: InputMode,
    jump_buffer: String,
    jump_start: Option<Instant>,
}

/// Timeout for jump-to-slide digit accumulation.
const JUMP_TIMEOUT_SECS: f64 = 3.0;
/// Default zoom factor when zoom is first activated.
const DEFAULT_ZOOM_FACTOR: f32 = 1.5;
/// Supported zoom steps for mouse-wheel control.
const ZOOM_STEPS: &[f32] = &[1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0];

impl InputHandler {
    pub fn new(sender: CommandSender, keybindings: KeybindingMap) -> Self {
        Self {
            sender,
            keybindings,
            mode: InputMode::Normal,
            jump_buffer: String::new(),
            jump_start: None,
        }
    }

    /// Call once per frame from the presenter console.
    ///
    /// UI state drives mode transitions.
    pub fn handle_input(&mut self, ctx: &egui::Context, modes: UiModes) {
        // Sync mode from external state changes
        if self.mode != InputMode::JumpToSlide {
            if modes.overview_visible {
                self.mode = InputMode::Overview;
            } else if modes.notes_editing {
                self.mode = InputMode::NotesEdit;
            } else if modes.ink_active {
                self.mode = InputMode::Ink;
            } else if modes.laser_active {
                self.mode = InputMode::Laser;
            } else {
                self.mode = InputMode::Normal;
            }
        }

        // Check jump-to-slide timeout
        if self.mode == InputMode::JumpToSlide
            && let Some(start) = self.jump_start
            && start.elapsed().as_secs_f64() > JUMP_TIMEOUT_SECS
        {
            self.cancel_jump();
        }

        if self.mode == InputMode::NotesEdit {
            self.process_notes_editor_keys(ctx);
            return;
        }

        self.process_keys(ctx);
    }

    fn process_notes_editor_keys(&mut self, ctx: &egui::Context) {
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());

        for event in &events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                if *key == egui::Key::Escape {
                    let _ = self.sender.send(Command::ToggleNotesEdit);
                    continue;
                }

                let combo = egui_to_key_combo(*key, *modifiers);
                if let Some(action) = self.keybindings.lookup(&combo) {
                    match action {
                        Action::SaveSidecar | Action::ToggleNotesEdit => {
                            self.dispatch_action(action);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn process_keys(&mut self, ctx: &egui::Context) {
        // Collect key events this frame
        let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());

        for event in &events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                self.handle_key(*key, *modifiers);
            }
        }
    }

    fn handle_key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        // In jump-to-slide mode, handle digits/Enter/Escape specially
        if self.mode == InputMode::JumpToSlide {
            if let Some(digit) = key_to_digit(key) {
                self.jump_buffer.push(digit);
                return;
            }
            match key {
                egui::Key::Enter => {
                    if let Ok(page_num) = self.jump_buffer.parse::<usize>() {
                        let index = page_num.saturating_sub(1);
                        let _ = self.sender.send(Command::GoToSlide(index));
                    }
                    self.cancel_jump();
                    return;
                }
                egui::Key::Escape => {
                    self.cancel_jump();
                    return;
                }
                _ => {
                    self.cancel_jump();
                }
            }
        }

        let combo = egui_to_key_combo(key, modifiers);
        if let Some(action) = self.keybindings.lookup(&combo) {
            self.dispatch_action(action);
        }
    }

    fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::GoToSlide => {
                self.mode = InputMode::JumpToSlide;
                self.jump_buffer.clear();
                self.jump_start = Some(Instant::now());
            }
            Action::StartPauseTimer => {
                let _ = self.sender.send(Command::ToggleTimer);
            }
            _ => {
                if let Some(cmd) = action_to_command(action) {
                    let _ = self.sender.send(cmd);
                }
            }
        }
    }

    fn cancel_jump(&mut self) {
        self.mode = InputMode::Normal;
        self.jump_buffer.clear();
        self.jump_start = None;
    }

    /// Handle mouse interaction on the current slide image area.
    ///
    /// Call this with the egui `Response` and image `Rect` from the current
    /// slide widget.
    pub fn handle_slide_mouse(
        &self,
        response: &egui::Response,
        image_rect: egui::Rect,
        aids: ActiveAids,
        current_zoom_factor: Option<f32>,
    ) {
        if let Some(pos) = response.hover_pos() {
            let norm = normalize_to_rect(pos, image_rect);
            if (0.0..=1.0).contains(&norm.0) && (0.0..=1.0).contains(&norm.1) {
                if aids.laser || aids.spotlight {
                    let _ = self.sender.send(Command::SetPointerPosition(norm.0, norm.1));
                    if aids.spotlight {
                        let _ = self.sender.send(Command::SetSpotlightPosition(norm.0, norm.1));
                    }
                }

                if aids.zoom {
                    let scroll_delta = response.ctx.input(|i| i.raw_scroll_delta.y);
                    let current_factor = current_zoom_factor.unwrap_or(DEFAULT_ZOOM_FACTOR);
                    let factor = if response.hovered() && scroll_delta.abs() > f32::EPSILON {
                        step_zoom_factor(current_factor, scroll_delta)
                    } else {
                        current_factor
                    };
                    let _ = self
                        .sender
                        .send(Command::SetZoomRegion { center: (norm.0, norm.1), factor });
                }
            }
        }

        let pointer_down = response.ctx.input(|i| i.pointer.primary_down());
        if aids.ink
            && pointer_down
            && response.contains_pointer()
            && let Some(pos) = response
                .interact_pointer_pos()
                .or_else(|| response.ctx.input(|i| i.pointer.latest_pos()))
        {
            let norm = normalize_to_rect(pos, image_rect);
            if (0.0..=1.0).contains(&norm.0) && (0.0..=1.0).contains(&norm.1) {
                let _ = self.sender.send(Command::AddInkPoint(norm.0, norm.1));
            }
        }

        if aids.ink && response.drag_stopped() {
            let _ = self.sender.send(Command::FinishInkStroke);
        }
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn jump_buffer(&self) -> &str {
        &self.jump_buffer
    }
}

fn step_zoom_factor(current_factor: f32, scroll_delta: f32) -> f32 {
    let current_index = ZOOM_STEPS
        .iter()
        .position(|step| (*step - current_factor).abs() < f32::EPSILON)
        .unwrap_or_else(|| nearest_zoom_step_index(current_factor));

    let next_index = if scroll_delta > 0.0 {
        current_index.saturating_add(1).min(ZOOM_STEPS.len() - 1)
    } else {
        current_index.saturating_sub(1)
    };

    ZOOM_STEPS[next_index]
}

fn nearest_zoom_step_index(current_factor: f32) -> usize {
    ZOOM_STEPS
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (current_factor - **left)
                .abs()
                .partial_cmp(&(current_factor - **right).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map_or(0, |(index, _)| index)
}

/// Convert a screen-space position to normalized (0..1) coordinates within a rect.
pub fn normalize_to_rect(pos: egui::Pos2, rect: egui::Rect) -> (f32, f32) {
    let x = (pos.x - rect.min.x) / rect.width();
    let y = (pos.y - rect.min.y) / rect.height();
    (x, y)
}

/// Convert an egui key + modifiers to our `KeyCombo` string format for lookup.
fn egui_to_key_combo(key: egui::Key, modifiers: egui::Modifiers) -> KeyCombo {
    let key_name = egui_key_name(key);
    KeyCombo {
        key: key_name,
        shift: modifiers.shift,
        ctrl: modifiers.ctrl || modifiers.command,
        alt: modifiers.alt,
    }
}

/// Map an egui key to the string name used in our keybinding config.
fn egui_key_name(key: egui::Key) -> String {
    egui_key_name_public(key)
}

/// Map an egui key to the string name used in our keybinding config (public for test-input mode).
pub fn egui_key_name_public(key: egui::Key) -> String {
    match key {
        egui::Key::ArrowRight => "Right".into(),
        egui::Key::ArrowLeft => "Left".into(),
        egui::Key::ArrowUp => "Up".into(),
        egui::Key::ArrowDown => "Down".into(),
        egui::Key::Space => "Space".into(),
        egui::Key::Enter => "Enter".into(),
        egui::Key::Escape => "Escape".into(),
        egui::Key::Home => "Home".into(),
        egui::Key::End => "End".into(),
        egui::Key::PageUp => "PageUp".into(),
        egui::Key::PageDown => "PageDown".into(),
        egui::Key::Tab => "Tab".into(),
        egui::Key::Backspace => "Backspace".into(),
        egui::Key::Delete => "Delete".into(),
        egui::Key::F1 => "F1".into(),
        egui::Key::F2 => "F2".into(),
        egui::Key::F3 => "F3".into(),
        egui::Key::F4 => "F4".into(),
        egui::Key::F5 => "F5".into(),
        egui::Key::F6 => "F6".into(),
        egui::Key::F7 => "F7".into(),
        egui::Key::F8 => "F8".into(),
        egui::Key::F9 => "F9".into(),
        egui::Key::F10 => "F10".into(),
        egui::Key::F11 => "F11".into(),
        egui::Key::F12 => "F12".into(),
        egui::Key::Minus => "-".into(),
        egui::Key::Plus => "+".into(),
        egui::Key::Equals => "=".into(),
        egui::Key::Period => ".".into(),
        other => {
            // For letter keys (A-Z) and digit keys, egui::Key debug names work
            let debug = format!("{other:?}");
            debug.to_lowercase()
        }
    }
}

/// Try to extract a digit character from a key press.
fn key_to_digit(key: egui::Key) -> Option<char> {
    match key {
        egui::Key::Num0 => Some('0'),
        egui::Key::Num1 => Some('1'),
        egui::Key::Num2 => Some('2'),
        egui::Key::Num3 => Some('3'),
        egui::Key::Num4 => Some('4'),
        egui::Key::Num5 => Some('5'),
        egui::Key::Num6 => Some('6'),
        egui::Key::Num7 => Some('7'),
        egui::Key::Num8 => Some('8'),
        egui::Key::Num9 => Some('9'),
        _ => None,
    }
}

/// Map an `Action` to a `Command` for simple 1:1 mappings.
/// Returns `None` for actions that need special handling (`GoToSlide`, `StartPauseTimer`).
fn action_to_command(action: Action) -> Option<Command> {
    match action {
        Action::NextSlide => Some(Command::NextSlide),
        Action::PreviousSlide => Some(Command::PreviousSlide),
        Action::NextOverlay => Some(Command::NextOverlay),
        Action::PreviousOverlay => Some(Command::PreviousOverlay),
        Action::FirstSlide => Some(Command::FirstSlide),
        Action::LastSlide => Some(Command::LastSlide),
        Action::ToggleFreeze => Some(Command::ToggleFreeze),
        Action::ToggleBlackout => Some(Command::ToggleBlackout),
        Action::ToggleLaser => Some(Command::ToggleLaser),
        Action::ToggleInk => Some(Command::ToggleInk),
        Action::ClearInk => Some(Command::ClearInk),
        Action::ToggleSpotlight => Some(Command::ToggleSpotlight),
        Action::ToggleZoom => Some(Command::ToggleZoom),
        Action::ToggleOverview => Some(Command::ToggleSlideOverview),
        Action::ToggleNotes => Some(Command::ToggleNotesPanel),
        Action::ToggleNotesEdit => Some(Command::ToggleNotesEdit),
        Action::ResetTimer => Some(Command::ResetTimer),
        Action::IncrementNotesFont => Some(Command::IncrementNotesFontSize),
        Action::DecrementNotesFont => Some(Command::DecrementNotesFontSize),
        Action::ToggleScreenShare => Some(Command::ToggleScreenShareMode),
        Action::TogglePresentationMode => Some(Command::TogglePresentationMode),
        Action::Quit => Some(Command::Quit),
        Action::SaveSidecar => Some(Command::SaveSidecar),
        Action::GoToSlide | Action::StartPauseTimer => None,
    }
}
