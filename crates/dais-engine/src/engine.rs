use std::sync::{Arc, RwLock};
use std::time::Instant;

use dais_core::bus::CommandReceiver;
use dais_core::commands::Command;
use dais_core::config::Config;
use dais_core::slide_group::SlideGroup;
use dais_core::state::{
    ActivePen, InkStroke, PointerAppearance, PointerAppearances, PointerStyle, PresentationState,
    TextBox, TimerState, ZoomRegion,
};
use dais_sidecar::types::{InkStrokeMeta, PresentationMetadata, TextBoxMeta};

/// Fallback color presets appended when the user configures fewer than 4 colors.
const INK_COLOR_FALLBACKS: &[[u8; 4]] = &[
    [220, 30, 30, 255],  // red
    [30, 100, 220, 255], // blue
    [30, 180, 30, 255],  // green
    [220, 200, 0, 255],  // yellow
];

/// Built-in pen width presets cycled by `CycleInkWidth` (logical pixels).
const INK_WIDTH_PRESETS: &[f32] = &[1.0, 2.0, 4.0, 8.0, 16.0];

#[derive(Debug, Clone, Copy)]
enum SidecarKind {
    Dais,
    Pdfpc,
}

/// The presentation engine — processes commands and owns the authoritative state.
///
/// Called once per frame via `tick()`. All state mutations happen here.
/// The UI reads state via a shared `Arc<RwLock<PresentationState>>`.
pub struct PresentationEngine {
    receiver: CommandReceiver,
    state: PresentationState,
    shared_state: Arc<RwLock<PresentationState>>,
    timer_start: Option<Instant>,
    slide_start: Instant,
    /// Pen color presets for cycling (from config, padded to at least 4 with fallbacks).
    ink_color_presets: Vec<[u8; 4]>,
    /// Default text color for newly placed text boxes.
    text_box_default_color: [u8; 4],
    /// Default background fill for newly placed text boxes.
    text_box_default_background: Option<[u8; 4]>,
    /// Default Typst setup for newly placed text boxes.
    text_box_default_typst_prelude: String,
    /// Path to the PDF file (used for sidecar save).
    pdf_path: std::path::PathBuf,
    /// Which sidecar format to write on save.
    sidecar_kind: SidecarKind,
    /// Original metadata loaded for this presentation.
    metadata: PresentationMetadata,
    /// Whether sidecar saves should update per-slide timing data.
    save_slide_timings: bool,
}

impl PresentationEngine {
    /// Create a new engine from document metadata and config.
    ///
    /// Returns the engine and a shared state handle for UI windows.
    pub fn new(
        total_pages: usize,
        metadata: &PresentationMetadata,
        config: &Config,
        receiver: CommandReceiver,
        pdf_path: std::path::PathBuf,
    ) -> (Self, Arc<RwLock<PresentationState>>) {
        let slide_groups = build_slide_groups(total_pages, metadata);
        let mut state = PresentationState::new(total_pages, slide_groups);

        // Apply config to initial state
        let duration = match (config.timer.mode, config.timer.duration_minutes) {
            (dais_core::state::TimerMode::Elapsed, None) => None,
            (_, Some(minutes)) => Some(std::time::Duration::from_secs(u64::from(minutes) * 60)),
            (dais_core::state::TimerMode::Countdown, None) => {
                Some(std::time::Duration::from_mins(20))
            }
        };
        let warning_threshold = match (duration, config.timer.warning_minutes) {
            (Some(_), Some(minutes)) => {
                Some(std::time::Duration::from_secs(u64::from(minutes) * 60))
            }
            _ => None,
        };
        state.timer = TimerState {
            mode: config.timer.mode,
            duration,
            warning_threshold,
            ..TimerState::default()
        };
        state.notes_font_size = config.notes.font_size;
        state.notes_font_size_step = config.notes.font_size_step;
        state.pointer_style = parse_pointer_style(&config.laser.style);
        state.pointer_appearances = pointer_appearances_from_config(config);
        if config.display.mode == "single" {
            state.laser_active = false;
        }
        state.spotlight_radius = config.spotlight.radius.clamp(16.0, 2048.0);
        state.spotlight_dim_opacity = config.spotlight.dim_opacity.clamp(0.0, 1.0);

        // Parse user-configured colors, then pad with fallbacks until we have at least 4.
        let mut ink_color_presets: Vec<[u8; 4]> =
            config.ink.colors.iter().filter_map(|s| parse_hex_color(s)).collect();
        for &fallback in INK_COLOR_FALLBACKS {
            if ink_color_presets.len() >= 4 {
                break;
            }
            ink_color_presets.push(fallback);
        }

        state.active_pen = ActivePen {
            color: ink_color_presets.first().copied().unwrap_or([255, 0, 0, 255]),
            width: config.ink.width,
        };
        let text_box_default_color =
            parse_hex_color(&config.text_boxes.color).unwrap_or([0, 0, 0, 255]);
        let text_box_default_background = parse_optional_hex_color(&config.text_boxes.background);
        let text_box_default_typst_prelude = config.text_boxes.typst_prelude.clone();

        // Hydrate runtime annotation state from metadata before publishing shared state,
        // so the UI never sees a state with annotations missing on the first frame.
        load_annotations_into_state(&mut state, metadata);

        let shared_state = Arc::new(RwLock::new(state.clone()));

        (
            Self {
                receiver,
                state,
                shared_state: Arc::clone(&shared_state),
                timer_start: None,
                slide_start: Instant::now(),
                ink_color_presets,
                text_box_default_color,
                text_box_default_background,
                text_box_default_typst_prelude,
                pdf_path,
                sidecar_kind: if config.normalized_sidecar_format() == "dais" {
                    SidecarKind::Dais
                } else {
                    SidecarKind::Pdfpc
                },
                metadata: metadata.clone(),
                save_slide_timings: config.save_slide_timings,
            },
            shared_state,
        )
    }

    /// Process all pending commands, update timer, and broadcast state.
    ///
    /// Returns `true` if the application should quit.
    pub fn tick(&mut self) -> bool {
        // Update timers. Returns true when timer state is actively changing.
        let timers_ticking = self.update_timers();

        // Drain and process all pending commands
        let commands = self.receiver.drain();
        let mut should_quit = false;
        let content_changed = !commands.is_empty();

        for cmd in &commands {
            if matches!(cmd, Command::Quit) {
                if self.state.presentation_mode {
                    self.state.presentation_mode = false;
                } else if self.state.overview_visible {
                    self.state.overview_visible = false;
                } else if self.state.quit_requested {
                    self.save_sidecar();
                    should_quit = true;
                } else {
                    self.state.quit_requested = true;
                }
            } else {
                self.state.quit_requested = false;
            }
            self.process_command(cmd);
        }

        // Broadcast updated state to UI.
        // When commands were processed, the full state may have changed — clone everything.
        // When only timers ticked, write just the two timer fields to avoid cloning the entire
        // state (which contains Vecs and HashMaps) at 60 fps.
        if content_changed {
            if let Ok(mut shared) = self.shared_state.write() {
                *shared = self.state.clone();
            }
        } else if timers_ticking && let Ok(mut shared) = self.shared_state.write() {
            shared.timer.elapsed = self.state.timer.elapsed;
            shared.slide_elapsed = self.state.slide_elapsed;
        }

        should_quit
    }

    /// Get a reference to the current state (for engine-internal use).
    pub fn state(&self) -> &PresentationState {
        &self.state
    }

    /// Update timer state each frame. Returns `true` when timer fields are actively changing
    /// and the UI needs a (partial) state update.
    fn update_timers(&mut self) -> bool {
        let current = self.state.current_logical_slide;
        let elapsed = self.slide_start.elapsed();
        if let Some(total) = self.state.slide_elapsed_by_logical.get_mut(current) {
            *total = elapsed;
            self.state.slide_elapsed = *total;
        } else {
            self.state.slide_elapsed = elapsed;
        }

        if self.state.timer.running
            && let Some(start) = self.timer_start
        {
            self.state.timer.elapsed = start.elapsed();
        }

        self.state.timer.running || self.state.slide_elapsed > std::time::Duration::ZERO
    }

    fn process_command(&mut self, cmd: &Command) {
        match cmd {
            Command::NextSlide
            | Command::PreviousSlide
            | Command::NextOverlay
            | Command::PreviousOverlay
            | Command::FirstSlide
            | Command::LastSlide
            | Command::GoToSlide(_) => self.handle_navigation(cmd),

            Command::ToggleFreeze
            | Command::ToggleBlackout
            | Command::ToggleWhiteboard
            | Command::ToggleScreenShareMode
            | Command::TogglePresentationMode => {
                self.handle_display_mode(cmd);
            }

            Command::ToggleLaser
            | Command::CycleLaserStyle
            | Command::SetPointerPosition(..)
            | Command::ToggleInk
            | Command::AddInkPoint(..)
            | Command::FinishInkStroke
            | Command::ClearInk
            | Command::SetInkColor(_)
            | Command::SetInkWidth(_)
            | Command::CycleInkColor
            | Command::CycleInkWidth
            | Command::ToggleSpotlight
            | Command::SetSpotlightPosition(..)
            | Command::ToggleZoom
            | Command::SetZoomRegion { .. } => self.handle_aid(cmd),

            Command::ToggleTextBoxMode
            | Command::PlaceTextBox { .. }
            | Command::EditTextBoxContent { .. }
            | Command::MoveTextBox { .. }
            | Command::ResizeTextBox { .. }
            | Command::DeleteTextBox { .. }
            | Command::SelectTextBox(_)
            | Command::DeselectTextBox
            | Command::BeginTextBoxEdit { .. }
            | Command::SetTextBoxFontSize { .. }
            | Command::SetTextBoxColor { .. }
            | Command::SetTextBoxBackground { .. } => self.handle_text_box(cmd),

            Command::StartTimer
            | Command::PauseTimer
            | Command::ToggleTimer
            | Command::ResetTimer => {
                self.handle_timer(cmd);
            }

            Command::ToggleSlideOverview
            | Command::ToggleNotesPanel
            | Command::ToggleNotesEdit
            | Command::SetCurrentSlideNotes(_)
            | Command::IncrementNotesFontSize
            | Command::DecrementNotesFontSize => self.handle_ui_panel(cmd),

            Command::Quit => {} // handled in tick()
            Command::SaveSidecar => {
                self.save_sidecar();
            }
        }
    }

    fn handle_navigation(&mut self, cmd: &Command) {
        match *cmd {
            Command::NextSlide => self.next_slide(),
            Command::PreviousSlide => self.previous_slide(),
            Command::NextOverlay => self.next_overlay(),
            Command::PreviousOverlay => self.previous_overlay(),
            Command::FirstSlide => self.go_to_group(0),
            Command::LastSlide => {
                let last = self.state.total_logical_slides.saturating_sub(1);
                self.go_to_group(last);
            }
            Command::GoToSlide(index) => self.go_to_group(index),
            _ => {}
        }
    }

    fn handle_display_mode(&mut self, cmd: &Command) {
        match *cmd {
            Command::ToggleFreeze => {
                if self.state.frozen {
                    self.state.frozen = false;
                    self.state.frozen_page = None;
                } else {
                    self.state.frozen = true;
                    self.state.frozen_page = Some(self.state.current_page);
                }
            }
            Command::ToggleBlackout => {
                self.state.blacked_out = !self.state.blacked_out;
                if self.state.blacked_out {
                    self.state.whiteboard_active = false;
                }
            }
            Command::ToggleWhiteboard => {
                self.state.whiteboard_active = !self.state.whiteboard_active;
                if self.state.whiteboard_active {
                    self.state.blacked_out = false;
                    // Auto-activate ink so the user can draw immediately
                    self.state.ink_active = true;
                    self.state.laser_active = false;
                    self.state.text_box_mode = false;
                    self.state.pointer_position = None;
                    self.reset_text_box_selection();
                }
            }
            Command::ToggleScreenShareMode => {
                self.state.screen_share_mode = !self.state.screen_share_mode;
            }
            Command::TogglePresentationMode => {
                self.state.presentation_mode = !self.state.presentation_mode;
            }
            _ => {}
        }
    }

    fn handle_aid(&mut self, cmd: &Command) {
        match *cmd {
            Command::ToggleLaser => {
                self.state.laser_active = !self.state.laser_active;
                if !self.state.laser_active {
                    self.state.pointer_position = None;
                }
                // Laser and ink are mutually exclusive
                if self.state.laser_active {
                    self.state.ink_active = false;
                }
            }
            Command::CycleLaserStyle => {
                self.state.pointer_style = match self.state.pointer_style {
                    PointerStyle::Dot => PointerStyle::Minimal,
                    PointerStyle::Minimal => PointerStyle::Crosshair,
                    PointerStyle::Crosshair => PointerStyle::Arrow,
                    PointerStyle::Arrow => PointerStyle::Ring,
                    PointerStyle::Ring => PointerStyle::Bullseye,
                    PointerStyle::Bullseye => PointerStyle::Highlight,
                    PointerStyle::Highlight => PointerStyle::Dot,
                };
            }
            Command::SetPointerPosition(x, y) => {
                let clamped = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
                if self.state.laser_active || self.state.spotlight_active {
                    self.state.pointer_position = Some(clamped);
                }
                if self.state.spotlight_active {
                    self.state.spotlight_position = Some(clamped);
                }
            }
            Command::ToggleInk
            | Command::AddInkPoint(..)
            | Command::FinishInkStroke
            | Command::ClearInk
            | Command::SetInkColor(_)
            | Command::SetInkWidth(_)
            | Command::CycleInkColor
            | Command::CycleInkWidth => self.handle_ink(cmd),
            Command::ToggleSpotlight => {
                self.state.spotlight_active = !self.state.spotlight_active;
                if !self.state.spotlight_active {
                    self.state.spotlight_position = None;
                }
            }
            Command::SetSpotlightPosition(x, y) if self.state.spotlight_active => {
                self.state.spotlight_position = Some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
            }
            Command::SetSpotlightPosition(..) => {}
            Command::ToggleZoom => {
                self.state.zoom_active = !self.state.zoom_active;
                if !self.state.zoom_active {
                    self.state.zoom_region = None;
                }
            }
            Command::SetZoomRegion { center, factor } if self.state.zoom_active => {
                self.state.zoom_region = Some(ZoomRegion {
                    center: (center.0.clamp(0.0, 1.0), center.1.clamp(0.0, 1.0)),
                    factor: factor.clamp(1.0, 10.0),
                });
            }
            Command::SetZoomRegion { .. } => {}
            _ => {}
        }
    }

    fn handle_ink(&mut self, cmd: &Command) {
        match *cmd {
            Command::ToggleInk => {
                self.state.ink_active = !self.state.ink_active;
                if self.state.ink_active {
                    self.state.laser_active = false;
                    self.state.pointer_position = None;
                }
            }
            Command::AddInkPoint(x, y) if self.state.ink_active => {
                let point = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
                let active_pen = self.state.active_pen;
                let strokes = if self.state.whiteboard_active {
                    &mut self.state.whiteboard_strokes
                } else {
                    self.state.slide_ink_by_page.entry(self.state.current_page).or_default()
                };
                if let Some(stroke) = strokes.last_mut()
                    && !stroke.finished
                {
                    stroke.points.push(point);
                    return;
                }
                // Snapshot active pen into the stroke — later pen changes won't affect this.
                strokes.push(InkStroke {
                    points: vec![point],
                    color: active_pen.color,
                    width: active_pen.width,
                    finished: false,
                });
            }
            Command::AddInkPoint(..) => {}
            Command::FinishInkStroke => {
                let strokes = if self.state.whiteboard_active {
                    &mut self.state.whiteboard_strokes
                } else {
                    self.state.slide_ink_by_page.entry(self.state.current_page).or_default()
                };
                if let Some(stroke) = strokes.last_mut() {
                    stroke.finished = true;
                }
            }
            Command::ClearInk => {
                if self.state.whiteboard_active {
                    self.state.whiteboard_strokes.clear();
                } else {
                    self.state.slide_ink_by_page.remove(&self.state.current_page);
                }
            }
            Command::SetInkColor(color) => {
                self.state.active_pen.color = color;
            }
            Command::SetInkWidth(width) => {
                self.state.active_pen.width = width.max(0.5);
            }
            Command::CycleInkColor if !self.ink_color_presets.is_empty() => {
                let current = self.state.active_pen.color;
                let idx = self.ink_color_presets.iter().position(|c| *c == current).unwrap_or(0);
                self.state.active_pen.color =
                    self.ink_color_presets[(idx + 1) % self.ink_color_presets.len()];
            }
            Command::CycleInkColor => {}
            Command::CycleInkWidth => {
                let current = self.state.active_pen.width;
                // Find the first preset strictly greater than the current width, wrapping to
                // the smallest. This handles non-preset widths (e.g. from SetInkWidth or
                // config) by snapping forward to the next larger preset rather than resetting.
                self.state.active_pen.width = INK_WIDTH_PRESETS
                    .iter()
                    .find(|&&w| w > current)
                    .copied()
                    .unwrap_or(INK_WIDTH_PRESETS[0]);
            }
            _ => {}
        }
    }

    fn reset_text_box_selection(&mut self) {
        self.state.selected_text_box = None;
        self.state.text_box_editing = false;
    }

    fn update_current_text_box_mut(&mut self, id: u64, f: impl FnOnce(&mut TextBox)) {
        let page = self.state.current_page;
        if let Some(boxes) = self.state.slide_text_boxes_by_page.get_mut(&page)
            && let Some(tb) = boxes.iter_mut().find(|b| b.id == id)
        {
            f(tb);
        }
    }

    fn create_text_box(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let id = self.state.next_text_box_id;
        self.state.next_text_box_id += 1;
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, 1.0);
        let w = w.clamp(0.01, 1.0 - x);
        let h = h.clamp(0.01, 1.0 - y);
        let text_box = TextBox {
            id,
            rect: (x, y, w, h),
            content: String::new(),
            font_size: 30.0,
            color: self.text_box_default_color,
            background: self.text_box_default_background,
            typst_prelude: self.text_box_default_typst_prelude.clone(),
        };
        self.state
            .slide_text_boxes_by_page
            .entry(self.state.current_page)
            .or_default()
            .push(text_box);
        self.state.selected_text_box = Some(id);
        self.state.text_box_editing = true;
    }

    fn handle_text_box(&mut self, cmd: &Command) {
        match cmd {
            Command::ToggleTextBoxMode => {
                self.state.text_box_mode = !self.state.text_box_mode;
                if self.state.text_box_mode {
                    // Mutually exclusive with ink and laser
                    self.state.ink_active = false;
                    self.state.laser_active = false;
                    self.state.pointer_position = None;
                } else {
                    self.reset_text_box_selection();
                }
            }
            Command::PlaceTextBox { x, y, w, h } => self.create_text_box(*x, *y, *w, *h),
            Command::EditTextBoxContent { id, content } => {
                self.update_current_text_box_mut(*id, |tb| tb.content.clone_from(content));
            }
            Command::MoveTextBox { id, x, y } => {
                self.update_current_text_box_mut(*id, |tb| {
                    let (_, _, w, h) = tb.rect;
                    tb.rect = (x.clamp(0.0, 1.0 - w), y.clamp(0.0, 1.0 - h), w, h);
                });
            }
            Command::ResizeTextBox { id, w, h } => {
                self.update_current_text_box_mut(*id, |tb| {
                    let (x, y, _, _) = tb.rect;
                    let w = w.clamp(0.02, 1.0 - x);
                    let h = h.clamp(0.02, 1.0 - y);
                    tb.rect = (x, y, w, h);
                });
            }
            Command::DeleteTextBox { id } => {
                let page = self.state.current_page;
                if let Some(boxes) = self.state.slide_text_boxes_by_page.get_mut(&page) {
                    boxes.retain(|b| b.id != *id);
                }
                if self.state.selected_text_box == Some(*id) {
                    self.reset_text_box_selection();
                }
            }
            Command::SelectTextBox(id) => {
                self.state.selected_text_box = Some(*id);
                self.state.text_box_editing = false;
            }
            Command::DeselectTextBox => self.reset_text_box_selection(),
            Command::BeginTextBoxEdit { id } => {
                self.state.selected_text_box = Some(*id);
                self.state.text_box_editing = true;
            }
            Command::SetTextBoxFontSize { id, size } => {
                self.update_current_text_box_mut(*id, |tb| tb.font_size = size.clamp(6.0, 144.0));
            }
            Command::SetTextBoxColor { id, color } => {
                self.update_current_text_box_mut(*id, |tb| tb.color = *color);
            }
            Command::SetTextBoxBackground { id, color } => {
                self.update_current_text_box_mut(*id, |tb| tb.background = *color);
            }
            _ => {}
        }
    }

    fn handle_timer(&mut self, cmd: &Command) {
        match *cmd {
            Command::ToggleTimer => {
                if self.state.timer.running {
                    self.state.timer.running = false;
                } else {
                    self.state.timer.running = true;
                    self.timer_start = Some(
                        Instant::now()
                            .checked_sub(self.state.timer.elapsed)
                            .unwrap_or_else(Instant::now),
                    );
                }
            }
            Command::StartTimer if !self.state.timer.running => {
                self.state.timer.running = true;
                self.timer_start = Some(
                    Instant::now()
                        .checked_sub(self.state.timer.elapsed)
                        .unwrap_or_else(Instant::now),
                );
            }
            Command::StartTimer => {}
            Command::PauseTimer => self.state.timer.running = false,
            Command::ResetTimer => {
                self.state.timer.running = false;
                self.state.timer.elapsed = std::time::Duration::ZERO;
                self.timer_start = None;
            }
            _ => {}
        }
    }

    fn handle_ui_panel(&mut self, cmd: &Command) {
        match *cmd {
            Command::ToggleSlideOverview => {
                self.state.overview_visible = !self.state.overview_visible;
            }
            Command::ToggleNotesPanel => {
                self.state.notes_visible = !self.state.notes_visible;
                if !self.state.notes_visible {
                    self.state.notes_editing = false;
                }
            }
            Command::ToggleNotesEdit => {
                self.state.notes_editing = !self.state.notes_editing;
                if self.state.notes_editing {
                    self.state.notes_visible = true;
                }
            }
            Command::SetCurrentSlideNotes(ref text) => {
                let notes = if text.trim().is_empty() { None } else { Some(text.clone()) };
                if let Some(group) =
                    self.state.slide_groups.get_mut(self.state.current_logical_slide)
                {
                    group.notes.clone_from(&notes);
                }
                self.state.current_notes = notes;
            }
            Command::IncrementNotesFontSize => {
                self.state.notes_font_size =
                    (self.state.notes_font_size + self.state.notes_font_size_step).min(72.0);
            }
            Command::DecrementNotesFontSize => {
                self.state.notes_font_size =
                    (self.state.notes_font_size - self.state.notes_font_size_step).max(8.0);
            }
            _ => {}
        }
    }

    // -- Sidecar save --

    fn save_sidecar(&self) {
        use dais_sidecar::format::SidecarFormat;
        use dais_sidecar::types::SlideGroupMeta;

        // Reconstruct editable fields from current engine state while preserving
        // the original metadata fields we loaded from disk.
        let mut notes = std::collections::HashMap::new();
        let mut groups = Vec::new();
        for group in &self.state.slide_groups {
            if let (Some(&start), Some(&end)) = (group.pages.first(), group.pages.last()) {
                groups.push(SlideGroupMeta { start_page: start, end_page: end });
            }
            if let Some(ref text) = group.notes
                && let Some(&page) = group.pages.first()
            {
                notes.insert(page, text.clone());
            }
        }

        let mut metadata = self.metadata.clone();
        metadata.groups = groups;
        metadata.notes = notes;

        if self.save_slide_timings {
            // Persist per-slide timing data (logical slide index -> seconds).
            let mut slide_timings = std::collections::HashMap::new();
            for (i, dur) in self.state.slide_elapsed_by_logical.iter().enumerate() {
                let secs = dur.as_secs_f64();
                if secs > 0.0 {
                    slide_timings.insert(i, secs);
                }
            }
            metadata.slide_timings = slide_timings;
        }

        // Copy runtime annotations into metadata (completed strokes only).
        metadata.slide_annotations = self
            .state
            .slide_ink_by_page
            .iter()
            .filter(|(_, strokes)| !strokes.is_empty())
            .map(|(&page, strokes)| {
                (
                    page,
                    strokes
                        .iter()
                        .filter(|s| s.finished)
                        .map(|s| InkStrokeMeta {
                            points: s.points.clone(),
                            color: s.color,
                            width: s.width,
                        })
                        .collect(),
                )
            })
            .collect();
        metadata.whiteboard_annotations = self
            .state
            .whiteboard_strokes
            .iter()
            .filter(|s| s.finished)
            .map(|s| InkStrokeMeta { points: s.points.clone(), color: s.color, width: s.width })
            .collect();

        // Copy text box overlays into metadata.
        metadata.slide_text_boxes = self
            .state
            .slide_text_boxes_by_page
            .iter()
            .filter(|(_, boxes)| !boxes.is_empty())
            .map(|(&page, boxes)| {
                (
                    page,
                    boxes
                        .iter()
                        .map(|tb| TextBoxMeta {
                            id: tb.id,
                            rect: tb.rect,
                            content: tb.content.clone(),
                            font_size: tb.font_size,
                            color: tb.color,
                            background: tb.background,
                            typst_prelude: tb.typst_prelude.clone(),
                        })
                        .collect(),
                )
            })
            .collect();

        let (format, sidecar_path): (Box<dyn SidecarFormat>, _) = match self.sidecar_kind {
            SidecarKind::Dais => (
                Box::new(dais_sidecar::dais_format::DaisFormat),
                self.pdf_path.with_extension("dais"),
            ),
            SidecarKind::Pdfpc => {
                (Box::new(dais_sidecar::pdfpc::PdfpcFormat), self.pdf_path.with_extension("pdfpc"))
            }
        };

        match format.write(&sidecar_path, &metadata) {
            Ok(()) => tracing::info!("Saved sidecar to {}", sidecar_path.display()),
            Err(e) => tracing::error!("Failed to save sidecar: {e}"),
        }
    }

    // -- Navigation helpers --

    fn next_slide(&mut self) {
        self.advance_step();
    }

    fn previous_slide(&mut self) {
        self.rewind_step();
    }

    fn next_overlay(&mut self) {
        self.advance_step();
    }

    fn previous_overlay(&mut self) {
        self.rewind_step();
    }

    fn advance_step(&mut self) {
        if self.state.blacked_out {
            return;
        }
        if self.state.slide_groups.is_empty() {
            return;
        }
        let group = &self.state.slide_groups[self.state.current_logical_slide];
        let overlay = self.state.current_overlay_within_group;
        if overlay + 1 < group.pages.len() {
            self.state.current_overlay_within_group = overlay + 1;
            self.state.current_page = group.pages[overlay + 1];
        } else {
            let current = self.state.current_logical_slide;
            if current + 1 < self.state.total_logical_slides {
                self.state.blacked_out = false;
                self.go_to_group(current + 1);
            } else {
                self.state.blacked_out = true;
            }
        }
    }

    fn rewind_step(&mut self) {
        if self.state.blacked_out {
            self.state.blacked_out = false;
            return;
        }
        if self.state.slide_groups.is_empty() {
            return;
        }
        let overlay = self.state.current_overlay_within_group;
        if overlay > 0 {
            let group = &self.state.slide_groups[self.state.current_logical_slide];
            self.state.current_overlay_within_group = overlay - 1;
            self.state.current_page = group.pages[overlay - 1];
        } else {
            let current = self.state.current_logical_slide;
            if current > 0 {
                let last_overlay = self.state.slide_groups[current - 1].pages.len() - 1;
                self.go_to_group(current - 1);
                self.state.current_overlay_within_group = last_overlay;
                self.state.current_page = self.state.slide_groups[current - 1].pages[last_overlay];
            }
        }
    }

    fn go_to_group(&mut self, group_index: usize) {
        if self.state.slide_groups.is_empty() || self.state.total_logical_slides == 0 {
            return;
        }
        // Clamp to last valid slide if out of range
        let clamped = group_index.min(self.state.total_logical_slides - 1);
        self.state.blacked_out = false;
        self.state.current_logical_slide = clamped;
        self.state.current_overlay_within_group = 0;
        self.state.current_page = self.state.slide_groups[clamped].pages[0];
        let accumulated = self
            .state
            .slide_elapsed_by_logical
            .get(clamped)
            .copied()
            .unwrap_or(std::time::Duration::ZERO);
        self.slide_start = Instant::now().checked_sub(accumulated).unwrap_or_else(Instant::now);
        self.state.slide_elapsed = accumulated;
        self.update_notes();
    }

    fn update_notes(&mut self) {
        self.state.current_notes = self
            .state
            .slide_groups
            .get(self.state.current_logical_slide)
            .and_then(|g| g.notes.clone());
    }
}

/// Build slide groups from metadata, falling back to 1:1 if no grouping info.
///
/// Walks pages in order so that uncovered pages are interleaved at their natural position
/// rather than appended after all declared groups.
fn build_slide_groups(total_pages: usize, metadata: &PresentationMetadata) -> Vec<SlideGroup> {
    if metadata.groups.is_empty() {
        // 1:1 page-to-slide mapping, but still attach notes from metadata
        let mut groups = dais_core::slide_group::default_grouping(total_pages);
        for group in &mut groups {
            if let Some(page) = group.pages.first() {
                group.notes = metadata.notes.get(page).cloned();
            }
        }
        return groups;
    }

    // Index declared groups by their start page. Groups whose start page is out of range
    // are dropped; overlapping groups are resolved by whichever is encountered first in the
    // page walk (earlier group consumes its range and the walk skips past).
    let mut group_by_start: std::collections::HashMap<usize, &dais_sidecar::types::SlideGroupMeta> =
        metadata
            .groups
            .iter()
            .filter(|gm| gm.start_page < total_pages)
            .map(|gm| (gm.start_page, gm))
            .collect();

    let mut groups: Vec<SlideGroup> = Vec::new();
    let mut page = 0usize;
    while page < total_pages {
        let logical_index = groups.len();
        if let Some(gm) = group_by_start.remove(&page) {
            let end = gm.end_page.min(total_pages - 1);
            let pages: Vec<usize> = (page..=end).collect();
            let notes = metadata.notes.get(&page).cloned();
            groups.push(SlideGroup { logical_index, pages, notes });
            page = end + 1;
        } else {
            let notes = metadata.notes.get(&page).cloned();
            groups.push(SlideGroup { logical_index, pages: vec![page], notes });
            page += 1;
        }
    }

    groups
}

/// Parse a hex color string like "#FF0000" or "FF0000" to RGBA.
fn parse_hex_color(color_str: &str) -> Option<[u8; 4]> {
    let hex = color_str.strip_prefix('#').unwrap_or(color_str);
    if hex.len() == 6 {
        let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([red, green, blue, 255])
    } else if hex.len() == 8 {
        let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let alpha = u8::from_str_radix(&hex[6..8], 16).ok()?;
        Some([red, green, blue, alpha])
    } else {
        None
    }
}

fn parse_optional_hex_color(color_str: &str) -> Option<[u8; 4]> {
    if color_str.trim().eq_ignore_ascii_case("transparent") {
        None
    } else {
        parse_hex_color(color_str)
    }
}

fn parse_pointer_style(style: &str) -> PointerStyle {
    match style.trim().to_ascii_lowercase().as_str() {
        "minimal" => PointerStyle::Minimal,
        "crosshair" => PointerStyle::Crosshair,
        "arrow" => PointerStyle::Arrow,
        "ring" => PointerStyle::Ring,
        "bullseye" => PointerStyle::Bullseye,
        "highlight" => PointerStyle::Highlight,
        _ => PointerStyle::Dot,
    }
}

fn pointer_appearances_from_config(config: &Config) -> PointerAppearances {
    PointerAppearances {
        dot: pointer_appearance_from_config(&config.laser.dot),
        minimal: pointer_appearance_from_config(&config.laser.minimal),
        crosshair: pointer_appearance_from_config(&config.laser.crosshair),
        arrow: pointer_appearance_from_config(&config.laser.arrow),
        ring: pointer_appearance_from_config(&config.laser.ring),
        bullseye: pointer_appearance_from_config(&config.laser.bullseye),
        highlight: pointer_appearance_from_config(&config.laser.highlight),
    }
}

fn pointer_appearance_from_config(
    config: &dais_core::config::PointerStyleConfig,
) -> PointerAppearance {
    PointerAppearance {
        color: parse_hex_color(&config.color).unwrap_or([255, 0, 0, 255]),
        size: config.size.clamp(2.0, 96.0),
    }
}

/// Hydrate runtime annotation state from sidecar metadata.
fn load_annotations_into_state(state: &mut PresentationState, metadata: &PresentationMetadata) {
    for (page, strokes) in &metadata.slide_annotations {
        let runtime_strokes: Vec<InkStroke> = strokes
            .iter()
            .map(|s| InkStroke {
                points: s.points.clone(),
                color: s.color,
                width: s.width,
                finished: true,
            })
            .collect();
        if !runtime_strokes.is_empty() {
            state.slide_ink_by_page.insert(*page, runtime_strokes);
        }
    }
    state.whiteboard_strokes = metadata
        .whiteboard_annotations
        .iter()
        .map(|s| InkStroke {
            points: s.points.clone(),
            color: s.color,
            width: s.width,
            finished: true,
        })
        .collect();

    // Load text boxes from metadata
    let mut max_id: u64 = 0;
    for (page, boxes) in &metadata.slide_text_boxes {
        let runtime_boxes: Vec<TextBox> = boxes
            .iter()
            .map(|tb| {
                if tb.id > max_id {
                    max_id = tb.id;
                }
                TextBox {
                    id: tb.id,
                    rect: tb.rect,
                    content: tb.content.clone(),
                    font_size: tb.font_size,
                    color: tb.color,
                    background: tb.background,
                    typst_prelude: tb.typst_prelude.clone(),
                }
            })
            .collect();
        if !runtime_boxes.is_empty() {
            state.slide_text_boxes_by_page.insert(*page, runtime_boxes);
        }
    }
    // Ensure new IDs don't collide with loaded ones
    state.next_text_box_id = max_id + 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use dais_core::bus::CommandBus;
    use dais_sidecar::types::SlideGroupMeta;
    use std::collections::HashMap;

    fn make_engine(
        total_pages: usize,
    ) -> (PresentationEngine, Arc<RwLock<PresentationState>>, dais_core::bus::CommandSender) {
        make_engine_with_metadata(total_pages, &test_metadata())
    }

    fn test_metadata() -> PresentationMetadata {
        PresentationMetadata {
            slide_timings: HashMap::from([(0, 9.7e-6)]),
            ..PresentationMetadata::default()
        }
    }

    fn make_engine_with_metadata(
        total_pages: usize,
        metadata: &PresentationMetadata,
    ) -> (PresentationEngine, Arc<RwLock<PresentationState>>, dais_core::bus::CommandSender) {
        make_engine_with_config(total_pages, metadata, &Config::default())
    }

    fn make_engine_with_config(
        total_pages: usize,
        metadata: &PresentationMetadata,
        config: &Config,
    ) -> (PresentationEngine, Arc<RwLock<PresentationState>>, dais_core::bus::CommandSender) {
        let mut config = config.clone();
        config.save_slide_timings = false;
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let (engine, shared) = PresentationEngine::new(
            total_pages,
            metadata,
            &config,
            receiver,
            std::path::PathBuf::from("test.pdf"),
        );
        (engine, shared, sender)
    }

    // ---- parse_hex_color ----

    #[test]
    fn parse_hex_color_6_digit() {
        assert_eq!(parse_hex_color("#FF0000"), Some([255, 0, 0, 255]));
        assert_eq!(parse_hex_color("00FF00"), Some([0, 255, 0, 255]));
        assert_eq!(parse_hex_color("#0000ff"), Some([0, 0, 255, 255]));
    }

    #[test]
    fn parse_hex_color_8_digit() {
        assert_eq!(parse_hex_color("#FF000080"), Some([255, 0, 0, 128]));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#FFF"), None);
        assert_eq!(parse_hex_color("ZZZZZZ"), None);
    }

    #[test]
    fn parse_optional_hex_color_transparent() {
        assert_eq!(parse_optional_hex_color("transparent"), None);
        assert_eq!(parse_optional_hex_color(" TRANSPARENT "), None);
        assert_eq!(parse_optional_hex_color("#01020380"), Some([1, 2, 3, 128]));
    }

    #[test]
    fn parse_pointer_style_variants() {
        assert_eq!(parse_pointer_style("dot"), PointerStyle::Dot);
        assert_eq!(parse_pointer_style("minimal"), PointerStyle::Minimal);
        assert_eq!(parse_pointer_style("crosshair"), PointerStyle::Crosshair);
        assert_eq!(parse_pointer_style("arrow"), PointerStyle::Arrow);
        assert_eq!(parse_pointer_style("ring"), PointerStyle::Ring);
        assert_eq!(parse_pointer_style("bullseye"), PointerStyle::Bullseye);
        assert_eq!(parse_pointer_style("highlight"), PointerStyle::Highlight);
        assert_eq!(parse_pointer_style("unknown"), PointerStyle::Dot);
    }

    #[test]
    fn presentation_aid_config_populates_state() {
        let mut config = Config::default();
        config.laser.style = "crosshair".to_string();
        config.laser.crosshair.color = "#33CC66AA".to_string();
        config.laser.crosshair.size = 24.0;
        config.spotlight.radius = 220.0;
        config.spotlight.dim_opacity = 0.35;

        let (engine, _, _) = make_engine_with_config(3, &PresentationMetadata::default(), &config);

        let pointer = engine.state().current_pointer_appearance();
        assert_eq!(pointer.color, [0x33, 0xCC, 0x66, 0xAA]);
        assert!((pointer.size - 24.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_style, PointerStyle::Crosshair);
        assert!((engine.state().spotlight_radius - 220.0).abs() < f32::EPSILON);
        assert!((engine.state().spotlight_dim_opacity - 0.35).abs() < f32::EPSILON);
    }

    #[test]
    fn per_style_pointer_config_populates_state() {
        let mut config = Config::default();
        config.laser.style = "crosshair".to_string();
        config.laser.dot.color = "#FFFFFF".to_string();
        config.laser.dot.size = 10.0;
        config.laser.minimal.color = "#FF00AA".to_string();
        config.laser.minimal.size = 8.0;
        config.laser.crosshair.color = "#00FF00".to_string();
        config.laser.crosshair.size = 28.0;
        config.laser.arrow.color = "#3355FF80".to_string();
        config.laser.arrow.size = 18.0;
        config.laser.ring.color = "#FFAA00".to_string();
        config.laser.ring.size = 30.0;
        config.laser.bullseye.color = "#AA00FF".to_string();
        config.laser.bullseye.size = 26.0;
        config.laser.highlight.color = "#FFFF0080".to_string();
        config.laser.highlight.size = 36.0;

        let (engine, _, _) = make_engine_with_config(3, &PresentationMetadata::default(), &config);

        assert_eq!(engine.state().pointer_appearances.dot.color, [255, 255, 255, 255]);
        assert!((engine.state().pointer_appearances.dot.size - 10.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.minimal.color, [0xFF, 0, 0xAA, 255]);
        assert!((engine.state().pointer_appearances.minimal.size - 8.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.crosshair.color, [0, 255, 0, 255]);
        assert!((engine.state().pointer_appearances.crosshair.size - 28.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.arrow.color, [0x33, 0x55, 0xFF, 0x80]);
        assert!((engine.state().pointer_appearances.arrow.size - 18.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.ring.color, [0xFF, 0xAA, 0, 255]);
        assert!((engine.state().pointer_appearances.ring.size - 30.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.bullseye.color, [0xAA, 0, 0xFF, 255]);
        assert!((engine.state().pointer_appearances.bullseye.size - 26.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().pointer_appearances.highlight.color, [0xFF, 0xFF, 0, 0x80]);
        assert!((engine.state().pointer_appearances.highlight.size - 36.0).abs() < f32::EPSILON);
        assert_eq!(engine.state().current_pointer_appearance().color, [0, 255, 0, 255]);
    }

    // ---- build_slide_groups ----

    #[test]
    fn build_groups_no_metadata_gives_one_to_one() {
        let groups = build_slide_groups(5, &PresentationMetadata::default());
        assert_eq!(groups.len(), 5);
        for (i, group) in groups.iter().enumerate() {
            assert_eq!(group.logical_index, i);
            assert_eq!(group.pages, vec![i]);
        }
    }

    #[test]
    fn build_groups_from_metadata() {
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            ..Default::default()
        };
        let groups = build_slide_groups(5, &meta);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].pages, vec![0, 1, 2]);
        assert_eq!(groups[1].pages, vec![3, 4]);
    }

    #[test]
    fn build_groups_with_notes() {
        let mut notes = HashMap::new();
        notes.insert(0, "Slide one notes".to_string());
        notes.insert(3, "Slide two notes".to_string());
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            notes,
            ..Default::default()
        };
        let groups = build_slide_groups(5, &meta);
        assert_eq!(groups[0].notes.as_deref(), Some("Slide one notes"));
        assert_eq!(groups[1].notes.as_deref(), Some("Slide two notes"));
    }

    #[test]
    fn build_groups_uncovered_pages_become_individual() {
        let meta = PresentationMetadata {
            groups: vec![SlideGroupMeta { start_page: 0, end_page: 1 }],
            ..Default::default()
        };
        let groups = build_slide_groups(5, &meta);
        assert_eq!(groups.len(), 4); // 1 group + 3 individual
        assert_eq!(groups[0].pages, vec![0, 1]);
        assert_eq!(groups[1].pages, vec![2]);
        assert_eq!(groups[2].pages, vec![3]);
        assert_eq!(groups[3].pages, vec![4]);
    }

    // ---- Navigation ----

    #[test]
    fn initial_state_at_first_slide() {
        let (engine, _, _) = make_engine(10);
        let state = engine.state();
        assert_eq!(state.current_page, 0);
        assert_eq!(state.current_logical_slide, 0);
        assert_eq!(state.current_overlay_within_group, 0);
        assert_eq!(state.total_pages, 10);
        assert_eq!(state.total_logical_slides, 10);
    }

    #[test]
    fn next_slide_advances() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 1);
        assert_eq!(engine.state().current_page, 1);
    }

    #[test]
    fn next_slide_advances_within_group_before_next_group() {
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            ..Default::default()
        };
        let (mut engine, _, sender) = make_engine_with_metadata(5, &meta);

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 1);
        assert_eq!(engine.state().current_page, 1);

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 2);
        assert_eq!(engine.state().current_page, 2);

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 1);
        assert_eq!(engine.state().current_overlay_within_group, 0);
        assert_eq!(engine.state().current_page, 3);
    }

    #[test]
    fn next_slide_stops_at_end() {
        let (mut engine, _, sender) = make_engine(3);
        for _ in 0..10 {
            sender.send(Command::NextSlide).unwrap();
        }
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 2);
        assert!(engine.state().blacked_out);
    }

    #[test]
    fn previous_slide_stops_at_start() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
    }

    #[test]
    fn previous_slide_rewinds_within_group_before_previous_group() {
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            ..Default::default()
        };
        let (mut engine, _, sender) = make_engine_with_metadata(5, &meta);

        sender.send(Command::LastSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page, 3);

        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 2);
        assert_eq!(engine.state().current_page, 2);

        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 1);
        assert_eq!(engine.state().current_page, 1);
    }

    #[test]
    fn previous_slide_clears_end_blackout() {
        let (mut engine, _, sender) = make_engine(1);
        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert!(engine.state().blacked_out);

        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert!(!engine.state().blacked_out);
        assert_eq!(engine.state().current_logical_slide, 0);
    }

    #[test]
    fn first_and_last_slide() {
        let (mut engine, _, sender) = make_engine(10);
        sender.send(Command::LastSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 9);

        sender.send(Command::FirstSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
    }

    #[test]
    fn go_to_slide() {
        let (mut engine, _, sender) = make_engine(10);
        sender.send(Command::GoToSlide(5)).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 5);
    }

    #[test]
    fn go_to_slide_out_of_range_ignored() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::GoToSlide(100)).unwrap();
        engine.tick();
        // Out-of-range index is clamped to the last valid slide
        assert_eq!(engine.state().current_logical_slide, 4);
    }

    // ---- Overlay navigation ----

    #[test]
    fn overlay_navigation_within_group() {
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            ..Default::default()
        };
        let (mut engine, _, sender) = make_engine_with_metadata(5, &meta);
        assert_eq!(engine.state().current_logical_slide, 0);

        sender.send(Command::NextOverlay).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 1);
        assert_eq!(engine.state().current_page, 1);

        sender.send(Command::NextOverlay).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_overlay_within_group, 2);
        assert_eq!(engine.state().current_page, 2);

        // Overflow to next slide
        sender.send(Command::NextOverlay).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 1);
        assert_eq!(engine.state().current_overlay_within_group, 0);
        assert_eq!(engine.state().current_page, 3);
    }

    #[test]
    fn previous_overlay_goes_to_last_overlay_of_prev_group() {
        let meta = PresentationMetadata {
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 4 },
            ],
            ..Default::default()
        };
        let (mut engine, _, sender) = make_engine_with_metadata(5, &meta);
        sender.send(Command::LastSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 1);

        sender.send(Command::PreviousOverlay).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert_eq!(engine.state().current_overlay_within_group, 2);
        assert_eq!(engine.state().current_page, 2);
    }

    // ---- Display modes ----

    #[test]
    fn toggle_freeze_captures_page() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::NextSlide).unwrap();
        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page, 2);

        sender.send(Command::ToggleFreeze).unwrap();
        engine.tick();
        assert!(engine.state().frozen);
        assert_eq!(engine.state().frozen_page, Some(2));

        sender.send(Command::ToggleFreeze).unwrap();
        engine.tick();
        assert!(!engine.state().frozen);
        assert_eq!(engine.state().frozen_page, None);
    }

    #[test]
    fn toggle_blackout() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().blacked_out);
        sender.send(Command::ToggleBlackout).unwrap();
        engine.tick();
        assert!(engine.state().blacked_out);
        sender.send(Command::ToggleBlackout).unwrap();
        engine.tick();
        assert!(!engine.state().blacked_out);
    }

    #[test]
    fn toggle_screen_share() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().screen_share_mode);
        sender.send(Command::ToggleScreenShareMode).unwrap();
        engine.tick();
        assert!(engine.state().screen_share_mode);
    }

    #[test]
    fn toggle_presentation_mode() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().presentation_mode);
        sender.send(Command::TogglePresentationMode).unwrap();
        engine.tick();
        assert!(engine.state().presentation_mode);
        sender.send(Command::TogglePresentationMode).unwrap();
        engine.tick();
        assert!(!engine.state().presentation_mode);
    }

    #[test]
    fn quit_exits_presentation_mode_first() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::TogglePresentationMode).unwrap();
        engine.tick();
        assert!(engine.state().presentation_mode);

        // Quit should exit presentation mode, not quit
        sender.send(Command::Quit).unwrap();
        let should_quit = engine.tick();
        assert!(!should_quit);
        assert!(!engine.state().presentation_mode);

        // Second Quit triggers confirmation prompt
        sender.send(Command::Quit).unwrap();
        let should_quit = engine.tick();
        assert!(!should_quit);
        assert!(engine.state().quit_requested);

        // Third Quit actually quits
        sender.send(Command::Quit).unwrap();
        let should_quit = engine.tick();
        assert!(should_quit);
    }

    // ---- Presentation aids ----

    #[test]
    fn laser_and_ink_mutually_exclusive() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(engine.state().laser_active);

        sender.send(Command::ToggleLaser).unwrap();
        engine.tick();
        assert!(!engine.state().laser_active);

        sender.send(Command::ToggleInk).unwrap();
        engine.tick();
        assert!(engine.state().ink_active);
        assert!(!engine.state().laser_active);

        sender.send(Command::ToggleLaser).unwrap();
        engine.tick();
        assert!(engine.state().laser_active);
        assert!(!engine.state().ink_active);
    }

    #[test]
    fn cycle_laser_style_rotates_styles() {
        let (mut engine, _, sender) = make_engine(5);
        assert_eq!(engine.state().pointer_style, PointerStyle::Dot);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Minimal);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Crosshair);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Arrow);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Ring);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Bullseye);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Highlight);

        sender.send(Command::CycleLaserStyle).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_style, PointerStyle::Dot);
    }

    #[test]
    fn pointer_position_when_laser_active() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::SetPointerPosition(0.5, 0.5)).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_position, Some((0.5, 0.5)));
    }

    #[test]
    fn ink_stroke_lifecycle() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::AddInkPoint(0.3, 0.4)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        let strokes = engine.state().current_page_ink();
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].points.len(), 2);
        assert!(strokes[0].finished);

        sender.send(Command::AddInkPoint(0.5, 0.6)).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 2);
    }

    #[test]
    fn ink_points_ignored_when_ink_inactive() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        engine.tick();
        assert!(engine.state().current_page_ink().is_empty());
    }

    #[test]
    fn clear_ink() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);

        sender.send(Command::ClearInk).unwrap();
        engine.tick();
        assert!(engine.state().current_page_ink().is_empty());
    }

    #[test]
    fn ink_persists_across_navigation() {
        let (mut engine, _, sender) = make_engine(5);

        // Draw on slide 0
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);

        // Navigate to slide 1 — slide 0's ink should not appear
        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert!(engine.state().current_page_ink().is_empty());
        assert_eq!(engine.state().current_page, 1);

        // Return to slide 0 — ink should be restored
        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);
        assert_eq!(engine.state().current_page_ink()[0].points.len(), 1);
    }

    #[test]
    fn clear_ink_only_affects_current_page() {
        let (mut engine, _, sender) = make_engine(5);

        // Draw on slide 0
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        // Draw on slide 1
        sender.send(Command::NextSlide).unwrap();
        sender.send(Command::AddInkPoint(0.5, 0.5)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);

        // Clear ink on slide 1
        sender.send(Command::ClearInk).unwrap();
        engine.tick();
        assert!(engine.state().current_page_ink().is_empty());

        // Return to slide 0 — its ink should still exist
        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);
    }

    #[test]
    fn whiteboard_strokes_survive_navigation() {
        let (mut engine, _, sender) = make_engine(5);

        // Draw on whiteboard
        sender.send(Command::ToggleWhiteboard).unwrap();
        sender.send(Command::AddInkPoint(0.3, 0.3)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().whiteboard_strokes.len(), 1);

        // Leave whiteboard, navigate, re-enter
        sender.send(Command::ToggleWhiteboard).unwrap();
        sender.send(Command::NextSlide).unwrap();
        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();
        assert_eq!(engine.state().whiteboard_strokes.len(), 1);
    }

    #[test]
    fn annotations_loaded_from_metadata() {
        use dais_sidecar::types::InkStrokeMeta;

        let mut meta = PresentationMetadata::default();
        meta.slide_annotations.insert(
            0,
            vec![InkStrokeMeta {
                points: vec![(0.1, 0.2), (0.3, 0.4)],
                color: [255, 0, 0, 255],
                width: 3.0,
            }],
        );
        meta.whiteboard_annotations =
            vec![InkStrokeMeta { points: vec![(0.5, 0.5)], color: [0, 0, 255, 255], width: 2.0 }];

        let (engine, _, _) = make_engine_with_metadata(5, &meta);

        // Slide 0 annotations loaded
        assert_eq!(engine.state().current_page_ink().len(), 1);
        assert_eq!(engine.state().current_page_ink()[0].points.len(), 2);
        assert!(engine.state().current_page_ink()[0].finished);

        // Whiteboard annotations loaded
        assert_eq!(engine.state().whiteboard_strokes.len(), 1);
        assert!(engine.state().whiteboard_strokes[0].finished);
    }

    #[test]
    fn spotlight_toggle_and_position() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleSpotlight).unwrap();
        sender.send(Command::SetSpotlightPosition(0.3, 0.7)).unwrap();
        engine.tick();
        assert!(engine.state().spotlight_active);
        assert_eq!(engine.state().spotlight_position, Some((0.3, 0.7)));

        sender.send(Command::ToggleSpotlight).unwrap();
        engine.tick();
        assert!(!engine.state().spotlight_active);
        assert_eq!(engine.state().spotlight_position, None);
    }

    #[test]
    fn spotlight_position_ignored_when_inactive() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::SetSpotlightPosition(0.5, 0.5)).unwrap();
        engine.tick();
        assert_eq!(engine.state().spotlight_position, None);
    }

    #[test]
    fn zoom_toggle_and_region() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleZoom).unwrap();
        sender.send(Command::SetZoomRegion { center: (0.5, 0.5), factor: 2.0 }).unwrap();
        engine.tick();
        assert!(engine.state().zoom_active);
        let region = engine.state().zoom_region.as_ref().unwrap();
        assert_eq!(region.center, (0.5, 0.5));
        assert!((region.factor - 2.0).abs() < f32::EPSILON);

        sender.send(Command::ToggleZoom).unwrap();
        engine.tick();
        assert!(!engine.state().zoom_active);
        assert!(engine.state().zoom_region.is_none());
    }

    #[test]
    fn position_clamping() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::SetPointerPosition(-1.0, 2.0)).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_position, Some((0.0, 1.0)));
    }

    // ---- Timer ----

    #[test]
    fn timer_start_pause_reset() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().timer.running);

        sender.send(Command::StartTimer).unwrap();
        engine.tick();
        assert!(engine.state().timer.running);

        sender.send(Command::PauseTimer).unwrap();
        engine.tick();
        assert!(!engine.state().timer.running);

        sender.send(Command::ResetTimer).unwrap();
        engine.tick();
        assert!(!engine.state().timer.running);
        assert_eq!(engine.state().timer.elapsed, std::time::Duration::ZERO);
    }

    #[test]
    fn toggle_timer_starts_and_pauses() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().timer.running);

        // First toggle: starts the timer
        sender.send(Command::ToggleTimer).unwrap();
        engine.tick();
        assert!(engine.state().timer.running);

        // Second toggle: pauses the timer
        sender.send(Command::ToggleTimer).unwrap();
        engine.tick();
        assert!(!engine.state().timer.running);

        // Third toggle: starts again
        sender.send(Command::ToggleTimer).unwrap();
        engine.tick();
        assert!(engine.state().timer.running);
    }

    #[test]
    fn toggle_timer_does_not_cancel_itself_in_single_tick() {
        let (mut engine, _, sender) = make_engine(5);

        // Send two ToggleTimer commands in the same tick — should net to "no change"
        sender.send(Command::ToggleTimer).unwrap();
        sender.send(Command::ToggleTimer).unwrap();
        engine.tick();
        assert!(!engine.state().timer.running, "two toggles should cancel out");
    }

    #[test]
    fn slide_timer_accumulates_when_returning_to_slide() {
        let (mut engine, _, sender) = make_engine(5);

        std::thread::sleep(std::time::Duration::from_millis(15));
        engine.tick();
        let slide_0_elapsed = engine.state().slide_elapsed;
        assert!(slide_0_elapsed > std::time::Duration::ZERO);

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 1);
        assert!(
            engine.state().slide_elapsed <= std::time::Duration::from_millis(5),
            "new slide should start near zero"
        );

        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 0);
        assert!(
            engine.state().slide_elapsed >= slide_0_elapsed,
            "returning to a slide should restore accumulated time"
        );
    }

    // ---- UI panels ----

    #[test]
    fn toggle_overview() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().overview_visible);
        sender.send(Command::ToggleSlideOverview).unwrap();
        engine.tick();
        assert!(engine.state().overview_visible);
        sender.send(Command::ToggleSlideOverview).unwrap();
        engine.tick();
        assert!(!engine.state().overview_visible);
    }

    #[test]
    fn notes_font_size_bounds() {
        let (mut engine, _, sender) = make_engine(5);
        let initial = engine.state().notes_font_size;

        sender.send(Command::IncrementNotesFontSize).unwrap();
        engine.tick();
        assert!(engine.state().notes_font_size > initial);

        for _ in 0..100 {
            sender.send(Command::DecrementNotesFontSize).unwrap();
        }
        engine.tick();
        assert!((engine.state().notes_font_size - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn notes_font_size_upper_bound() {
        let (mut engine, _, sender) = make_engine(5);
        for _ in 0..100 {
            sender.send(Command::IncrementNotesFontSize).unwrap();
        }
        engine.tick();
        assert!((engine.state().notes_font_size - 72.0).abs() < f32::EPSILON);
    }

    // ---- Quit ----

    #[test]
    fn quit_command_requires_confirmation() {
        let (mut engine, _, sender) = make_engine(5);
        // First quit shows confirmation
        sender.send(Command::Quit).unwrap();
        assert!(!engine.tick());
        assert!(engine.state().quit_requested);

        // Second quit confirms
        sender.send(Command::Quit).unwrap();
        assert!(engine.tick());
    }

    #[test]
    fn quit_cancelled_by_other_command() {
        let (mut engine, _, sender) = make_engine(5);
        // First quit shows confirmation
        sender.send(Command::Quit).unwrap();
        engine.tick();
        assert!(engine.state().quit_requested);

        // Any other command cancels quit
        sender.send(Command::NextSlide).unwrap();
        assert!(!engine.tick());
        assert!(!engine.state().quit_requested);
    }

    #[test]
    fn no_quit_returns_false() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::NextSlide).unwrap();
        assert!(!engine.tick());
    }

    // ---- State broadcast ----

    #[test]
    fn state_broadcast_to_shared() {
        let (mut engine, shared, sender) = make_engine(5);
        sender.send(Command::NextSlide).unwrap();
        engine.tick();

        let shared_state = shared.read().unwrap();
        assert_eq!(shared_state.current_logical_slide, 1);
    }

    // ---- Notes update on navigation ----

    #[test]
    fn notes_update_on_navigation() {
        let mut notes = HashMap::new();
        notes.insert(0, "Notes for slide 0".to_string());
        notes.insert(1, "Notes for slide 1".to_string());
        let meta = PresentationMetadata { notes, ..Default::default() };
        let (mut engine, _, sender) = make_engine_with_metadata(3, &meta);

        assert_eq!(engine.state().current_notes.as_deref(), Some("Notes for slide 0"));

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_notes.as_deref(), Some("Notes for slide 1"));

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_notes, None);
    }

    #[test]
    fn notes_can_be_edited_inline() {
        let (mut engine, _, sender) = make_engine(3);

        sender.send(Command::ToggleNotesEdit).unwrap();
        sender.send(Command::SetCurrentSlideNotes("Hello **markdown**".to_string())).unwrap();
        engine.tick();

        assert!(engine.state().notes_editing);
        assert_eq!(engine.state().current_notes.as_deref(), Some("Hello **markdown**"));
        assert_eq!(engine.state().slide_groups[0].notes.as_deref(), Some("Hello **markdown**"));

        sender.send(Command::SetCurrentSlideNotes(String::new())).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_notes, None);
        assert_eq!(engine.state().slide_groups[0].notes, None);
    }

    #[test]
    fn new_text_boxes_use_configured_typst_prelude() {
        let mut config = Config::default();
        config.text_boxes.typst_prelude = "#set align(horizon)".to_string();
        let (mut engine, _, sender) =
            make_engine_with_config(1, &PresentationMetadata::default(), &config);

        sender.send(Command::PlaceTextBox { x: 0.1, y: 0.1, w: 0.3, h: 0.2 }).unwrap();
        engine.tick();

        let boxes = engine.state().current_page_text_boxes();
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].typst_prelude, "#set align(horizon)");
    }

    // ---- Whiteboard ----

    #[test]
    fn toggle_whiteboard() {
        let (mut engine, _, sender) = make_engine(5);
        assert!(!engine.state().whiteboard_active);

        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();
        assert!(engine.state().whiteboard_active);
        assert!(engine.state().ink_active); // auto-activates ink
        assert!(!engine.state().laser_active);

        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();
        assert!(!engine.state().whiteboard_active);
    }

    #[test]
    fn whiteboard_exits_text_box_context() {
        let (mut engine, _, sender) = make_engine(1);

        sender.send(Command::ToggleTextBoxMode).unwrap();
        sender.send(Command::PlaceTextBox { x: 0.1, y: 0.1, w: 0.2, h: 0.2 }).unwrap();
        engine.tick();
        assert!(engine.state().text_box_mode);
        assert!(engine.state().selected_text_box.is_some());

        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();

        assert!(engine.state().whiteboard_active);
        assert!(!engine.state().text_box_mode);
        assert!(engine.state().selected_text_box.is_none());
        assert!(!engine.state().text_box_editing);
    }

    #[test]
    fn whiteboard_and_blackout_mutually_exclusive() {
        let (mut engine, _, sender) = make_engine(5);

        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();
        assert!(engine.state().whiteboard_active);

        sender.send(Command::ToggleBlackout).unwrap();
        engine.tick();
        assert!(engine.state().blacked_out);
        assert!(!engine.state().whiteboard_active);

        sender.send(Command::ToggleWhiteboard).unwrap();
        engine.tick();
        assert!(engine.state().whiteboard_active);
        assert!(!engine.state().blacked_out);
    }

    #[test]
    fn whiteboard_ink_strokes_separate_from_slide() {
        let (mut engine, _, sender) = make_engine(5);

        // Draw on slide
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.1)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().current_page_ink().len(), 1);
        assert!(engine.state().whiteboard_strokes.is_empty());

        // Enter whiteboard and draw
        sender.send(Command::ToggleWhiteboard).unwrap();
        sender.send(Command::AddInkPoint(0.5, 0.5)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().whiteboard_strokes.len(), 1);
        // Slide strokes unchanged
        assert_eq!(engine.state().current_page_ink().len(), 1);
    }

    #[test]
    fn clear_ink_targets_whiteboard_when_active() {
        let (mut engine, _, sender) = make_engine(5);

        // Draw on whiteboard
        sender.send(Command::ToggleWhiteboard).unwrap();
        sender.send(Command::AddInkPoint(0.5, 0.5)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().whiteboard_strokes.len(), 1);

        sender.send(Command::ClearInk).unwrap();
        engine.tick();
        assert!(engine.state().whiteboard_strokes.is_empty());
    }

    // ---- ActivePen / pen settings ----

    #[test]
    fn active_pen_initializes_from_config() {
        let mut config = Config::default();
        config.ink.colors = vec!["#FF000080".to_string()]; // red, 50% alpha
        config.ink.width = 7.5;
        let (engine, _, _) = make_engine_with_config(3, &PresentationMetadata::default(), &config);
        assert_eq!(engine.state().active_pen.color, [255, 0, 0, 128]);
        assert!((engine.state().active_pen.width - 7.5).abs() < f32::EPSILON);
    }

    #[test]
    fn set_ink_color_updates_active_pen_only() {
        let (mut engine, _, sender) = make_engine(3);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.1)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        // Change color — must not touch the already-finished stroke.
        sender.send(Command::SetInkColor([0, 0, 255, 200])).unwrap();
        engine.tick();

        let strokes = engine.state().current_page_ink();
        assert_eq!(strokes.len(), 1);
        assert_eq!(engine.state().active_pen.color, [0, 0, 255, 200]);
        // First stroke unchanged.
        assert_ne!(strokes[0].color, [0, 0, 255, 200]);
    }

    #[test]
    fn set_ink_width_updates_active_pen_only() {
        let (mut engine, _, sender) = make_engine(3);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.2, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        let original_width = engine.state().current_page_ink()[0].width;

        sender.send(Command::SetInkWidth(12.0)).unwrap();
        engine.tick();

        assert!((engine.state().active_pen.width - 12.0).abs() < f32::EPSILON);
        // First stroke unchanged.
        assert!((engine.state().current_page_ink()[0].width - original_width).abs() < f32::EPSILON);
    }

    #[test]
    fn pen_change_does_not_mutate_prior_strokes() {
        let (mut engine, _, sender) = make_engine(3);
        sender.send(Command::ToggleInk).unwrap();

        // Stroke A with red, width 3.
        sender.send(Command::SetInkColor([255, 0, 0, 255])).unwrap();
        sender.send(Command::SetInkWidth(3.0)).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.1)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        // Change pen to blue, width 10, alpha 128.
        sender.send(Command::SetInkColor([0, 0, 255, 128])).unwrap();
        sender.send(Command::SetInkWidth(10.0)).unwrap();
        sender.send(Command::AddInkPoint(0.5, 0.5)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        let strokes = engine.state().current_page_ink();
        assert_eq!(strokes.len(), 2);
        // First stroke keeps its original style.
        assert_eq!(strokes[0].color, [255, 0, 0, 255]);
        assert!((strokes[0].width - 3.0).abs() < f32::EPSILON);
        // Second stroke uses the new pen.
        assert_eq!(strokes[1].color, [0, 0, 255, 128]);
        assert!((strokes[1].width - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn new_stroke_snapshots_active_pen_at_creation() {
        let mut config = Config::default();
        config.ink.colors = vec!["#00FF0080".to_string()]; // green, 50% alpha
        config.ink.width = 5.0;
        let (mut engine, _, sender) =
            make_engine_with_config(3, &PresentationMetadata::default(), &config);

        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.3, 0.3)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();

        let strokes = engine.state().current_page_ink();
        assert_eq!(strokes[0].color, [0, 255, 0, 128]);
        assert!((strokes[0].width - 5.0).abs() < f32::EPSILON);
    }
}
