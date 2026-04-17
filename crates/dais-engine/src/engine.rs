use std::sync::{Arc, RwLock};
use std::time::Instant;

use dais_core::bus::CommandReceiver;
use dais_core::commands::Command;
use dais_core::config::Config;
use dais_core::slide_group::SlideGroup;
use dais_core::state::{InkStroke, PresentationState, TimerState, ZoomRegion};
use dais_sidecar::types::PresentationMetadata;

/// The presentation engine — processes commands and owns the authoritative state.
///
/// Called once per frame via `tick()`. All state mutations happen here.
/// The UI reads state via a shared `Arc<RwLock<PresentationState>>`.
pub struct PresentationEngine {
    receiver: CommandReceiver,
    state: PresentationState,
    shared_state: Arc<RwLock<PresentationState>>,
    timer_start: Option<Instant>,
    /// Ink color from config (RGBA).
    ink_color: [u8; 4],
    /// Ink width from config.
    ink_width: f32,
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
    ) -> (Self, Arc<RwLock<PresentationState>>) {
        let slide_groups = build_slide_groups(total_pages, metadata);
        let mut state = PresentationState::new(total_pages, slide_groups);

        // Apply config to initial state
        state.timer = TimerState {
            mode: config.timer.mode,
            duration: std::time::Duration::from_secs(u64::from(config.timer.duration_minutes) * 60),
            warning_threshold: std::time::Duration::from_secs(
                u64::from(config.timer.warning_minutes) * 60,
            ),
            ..TimerState::default()
        };
        state.notes_font_size = config.notes.font_size;
        state.notes_font_size_step = config.notes.font_size_step;

        let shared_state = Arc::new(RwLock::new(state.clone()));

        let ink_color = parse_hex_color(&config.ink.color).unwrap_or([255, 0, 0, 255]);
        let ink_width = config.ink.width;

        (
            Self {
                receiver,
                state,
                shared_state: Arc::clone(&shared_state),
                timer_start: None,
                ink_color,
                ink_width,
            },
            shared_state,
        )
    }

    /// Process all pending commands, update timer, and broadcast state.
    ///
    /// Returns `true` if the application should quit.
    pub fn tick(&mut self) -> bool {
        // Update timer
        self.update_timer();

        // Drain and process all pending commands
        let commands = self.receiver.drain();
        let mut should_quit = false;
        let mut state_changed = !commands.is_empty();

        for cmd in &commands {
            if matches!(cmd, Command::Quit) {
                should_quit = true;
            }
            self.process_command(cmd);
        }

        // Timer always changes state when running
        if self.state.timer.running {
            state_changed = true;
        }

        // Broadcast updated state to UI
        if state_changed && let Ok(mut shared) = self.shared_state.write() {
            *shared = self.state.clone();
        }

        should_quit
    }

    /// Get a reference to the current state (for engine-internal use).
    pub fn state(&self) -> &PresentationState {
        &self.state
    }

    fn update_timer(&mut self) {
        if self.state.timer.running
            && let Some(start) = self.timer_start
        {
            self.state.timer.elapsed = start.elapsed();
        }
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

            Command::ToggleFreeze | Command::ToggleBlackout | Command::ToggleScreenShareMode => {
                self.handle_display_mode(cmd);
            }

            Command::ToggleLaser
            | Command::SetPointerPosition(..)
            | Command::ToggleInk
            | Command::AddInkPoint(..)
            | Command::FinishInkStroke
            | Command::ClearInk
            | Command::ToggleSpotlight
            | Command::SetSpotlightPosition(..)
            | Command::ToggleZoom
            | Command::SetZoomRegion { .. } => self.handle_aid(cmd),

            Command::StartTimer | Command::PauseTimer | Command::ToggleTimer
            | Command::ResetTimer => {
                self.handle_timer(cmd);
            }

            Command::ToggleSlideOverview
            | Command::ToggleNotesPanel
            | Command::IncrementNotesFontSize
            | Command::DecrementNotesFontSize => self.handle_ui_panel(cmd),

            Command::Quit => {} // handled in tick()
            Command::ReloadConfig => {
                tracing::info!("ReloadConfig received — not yet implemented");
            }
            Command::SaveSidecar => {
                tracing::info!("SaveSidecar received — not yet implemented");
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
            Command::ToggleBlackout => self.state.blacked_out = !self.state.blacked_out,
            Command::ToggleScreenShareMode => {
                self.state.screen_share_mode = !self.state.screen_share_mode;
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
            Command::SetPointerPosition(x, y) => {
                let clamped = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
                if self.state.laser_active || self.state.spotlight_active {
                    self.state.pointer_position = Some(clamped);
                }
                if self.state.spotlight_active {
                    self.state.spotlight_position = Some(clamped);
                }
            }
            Command::ToggleInk => {
                self.state.ink_active = !self.state.ink_active;
                if self.state.ink_active {
                    self.state.laser_active = false;
                    self.state.pointer_position = None;
                }
            }
            Command::AddInkPoint(x, y) if self.state.ink_active => {
                let point = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
                if let Some(stroke) = self.state.ink_strokes.last_mut()
                    && !stroke.finished
                {
                    stroke.points.push(point);
                    return;
                }
                self.state.ink_strokes.push(InkStroke {
                    points: vec![point],
                    color: self.ink_color,
                    width: self.ink_width,
                    finished: false,
                });
            }
            Command::AddInkPoint(..) => {}
            Command::FinishInkStroke => {
                if let Some(stroke) = self.state.ink_strokes.last_mut() {
                    stroke.finished = true;
                }
            }
            Command::ClearInk => self.state.ink_strokes.clear(),
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
            Command::ToggleNotesPanel => self.state.notes_visible = !self.state.notes_visible,
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

    // -- Navigation helpers --

    fn next_slide(&mut self) {
        let current = self.state.current_logical_slide;
        if current + 1 < self.state.total_logical_slides {
            self.go_to_group(current + 1);
        }
    }

    fn previous_slide(&mut self) {
        let current = self.state.current_logical_slide;
        if current > 0 {
            self.go_to_group(current - 1);
        }
    }

    fn next_overlay(&mut self) {
        if self.state.slide_groups.is_empty() {
            return;
        }
        let group = &self.state.slide_groups[self.state.current_logical_slide];
        let overlay = self.state.current_overlay_within_group;
        if overlay + 1 < group.pages.len() {
            // Advance within the same group
            self.state.current_overlay_within_group = overlay + 1;
            self.state.current_page = group.pages[overlay + 1];
        } else {
            // Overflow to next slide
            self.next_slide();
        }
    }

    fn previous_overlay(&mut self) {
        if self.state.slide_groups.is_empty() {
            return;
        }
        let overlay = self.state.current_overlay_within_group;
        if overlay > 0 {
            let group = &self.state.slide_groups[self.state.current_logical_slide];
            self.state.current_overlay_within_group = overlay - 1;
            self.state.current_page = group.pages[overlay - 1];
        } else {
            // Go to last overlay of previous slide
            let current = self.state.current_logical_slide;
            if current > 0 {
                let prev_group = &self.state.slide_groups[current - 1];
                let last_overlay = prev_group.pages.len() - 1;
                self.state.current_logical_slide = current - 1;
                self.state.current_overlay_within_group = last_overlay;
                self.state.current_page = prev_group.pages[last_overlay];
                self.update_notes();
            }
        }
    }

    fn go_to_group(&mut self, group_index: usize) {
        if group_index >= self.state.total_logical_slides || self.state.slide_groups.is_empty() {
            return;
        }
        self.state.current_logical_slide = group_index;
        self.state.current_overlay_within_group = 0;
        self.state.current_page = self.state.slide_groups[group_index].pages[0];
        self.update_notes();
        // Clear ink on navigation
        self.state.ink_strokes.clear();
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

    let mut groups = Vec::new();
    for (i, gm) in metadata.groups.iter().enumerate() {
        let pages: Vec<usize> = (gm.start_page..=gm.end_page.min(total_pages - 1)).collect();
        let notes = metadata.notes.get(&gm.start_page).cloned();
        if !pages.is_empty() {
            groups.push(SlideGroup { logical_index: i, pages, notes });
        }
    }

    // If groups don't cover all pages, add remaining as individual slides
    let covered: std::collections::HashSet<usize> =
        groups.iter().flat_map(|g| g.pages.iter().copied()).collect();
    let base_index = groups.len();
    for page in 0..total_pages {
        if !covered.contains(&page) {
            let notes = metadata.notes.get(&page).cloned();
            groups.push(SlideGroup { logical_index: base_index + page, pages: vec![page], notes });
        }
    }

    // Re-index logical indices
    for (i, group) in groups.iter_mut().enumerate() {
        group.logical_index = i;
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

#[cfg(test)]
mod tests {
    use super::*;
    use dais_core::bus::CommandBus;
    use dais_sidecar::types::SlideGroupMeta;
    use std::collections::HashMap;

    fn make_engine(
        total_pages: usize,
    ) -> (PresentationEngine, Arc<RwLock<PresentationState>>, dais_core::bus::CommandSender) {
        make_engine_with_metadata(total_pages, &PresentationMetadata::default())
    }

    fn make_engine_with_metadata(
        total_pages: usize,
        metadata: &PresentationMetadata,
    ) -> (PresentationEngine, Arc<RwLock<PresentationState>>, dais_core::bus::CommandSender) {
        let bus = CommandBus::new();
        let sender = bus.sender();
        let receiver = bus.into_receiver();
        let config = Config::default();
        let (engine, shared) = PresentationEngine::new(total_pages, metadata, &config, receiver);
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
    fn next_slide_stops_at_end() {
        let (mut engine, _, sender) = make_engine(3);
        for _ in 0..10 {
            sender.send(Command::NextSlide).unwrap();
        }
        engine.tick();
        assert_eq!(engine.state().current_logical_slide, 2);
    }

    #[test]
    fn previous_slide_stops_at_start() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::PreviousSlide).unwrap();
        engine.tick();
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
        assert_eq!(engine.state().current_logical_slide, 0);
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
        sender.send(Command::NextSlide).unwrap();
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

    // ---- Presentation aids ----

    #[test]
    fn laser_and_ink_mutually_exclusive() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleLaser).unwrap();
        engine.tick();
        assert!(engine.state().laser_active);

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
    fn pointer_position_when_laser_active() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::SetPointerPosition(0.5, 0.5)).unwrap();
        engine.tick();
        assert_eq!(engine.state().pointer_position, None);

        sender.send(Command::ToggleLaser).unwrap();
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

        let strokes = &engine.state().ink_strokes;
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].points.len(), 2);
        assert!(strokes[0].finished);

        sender.send(Command::AddInkPoint(0.5, 0.6)).unwrap();
        engine.tick();
        assert_eq!(engine.state().ink_strokes.len(), 2);
    }

    #[test]
    fn ink_points_ignored_when_ink_inactive() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        engine.tick();
        assert!(engine.state().ink_strokes.is_empty());
    }

    #[test]
    fn clear_ink() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().ink_strokes.len(), 1);

        sender.send(Command::ClearInk).unwrap();
        engine.tick();
        assert!(engine.state().ink_strokes.is_empty());
    }

    #[test]
    fn ink_cleared_on_navigation() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::ToggleInk).unwrap();
        sender.send(Command::AddInkPoint(0.1, 0.2)).unwrap();
        sender.send(Command::FinishInkStroke).unwrap();
        engine.tick();
        assert_eq!(engine.state().ink_strokes.len(), 1);

        sender.send(Command::NextSlide).unwrap();
        engine.tick();
        assert!(engine.state().ink_strokes.is_empty());
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
        sender.send(Command::ToggleLaser).unwrap();
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
    fn quit_command_returns_true() {
        let (mut engine, _, sender) = make_engine(5);
        sender.send(Command::Quit).unwrap();
        assert!(engine.tick());
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
}
