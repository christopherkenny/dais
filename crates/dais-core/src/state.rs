use std::collections::HashMap;
use std::time::Duration;

use crate::slide_group::SlideGroup;

/// A positioned text overlay on a slide.
#[derive(Debug, Clone)]
pub struct TextBox {
    /// Unique identifier for this text box.
    pub id: u64,
    /// Normalized position and size: (x, y, w, h) in 0..1 coordinates.
    pub rect: (f32, f32, f32, f32),
    /// Typst markup content.
    pub content: String,
    /// Font size in points.
    pub font_size: f32,
    /// Text color as RGBA.
    pub color: [u8; 4],
    /// Optional background fill as RGBA.
    pub background: Option<[u8; 4]>,
}

/// The single authoritative state of the presentation.
///
/// The engine owns and mutates this. The UI reads it (via watch channel) and renders it.
/// The UI holds no authoritative state of its own — all mutations go through
/// the [`CommandBus`](crate::bus::CommandBus).
#[derive(Debug, Clone)]
pub struct PresentationState {
    // -- Document info --
    /// Total number of raw PDF pages.
    pub total_pages: usize,
    /// Logical slide groups (may be 1:1 with pages if no grouping).
    pub slide_groups: Vec<SlideGroup>,
    /// Total number of logical slides.
    pub total_logical_slides: usize,

    // -- Navigation --
    /// Current raw PDF page index (0-based).
    pub current_page: usize,
    /// Current logical slide index (0-based).
    pub current_logical_slide: usize,
    /// Current overlay step within the current group (0-based).
    pub current_overlay_within_group: usize,

    // -- Display modes --
    /// Whether the audience display is frozen.
    pub frozen: bool,
    /// The page shown on the audience display when frozen (None = not frozen).
    pub frozen_page: Option<usize>,
    /// Whether the audience display is blacked out.
    pub blacked_out: bool,
    /// Whether the whiteboard is active (blank white canvas on audience).
    pub whiteboard_active: bool,
    /// Ink strokes drawn on the whiteboard (persist across navigation).
    pub whiteboard_strokes: Vec<InkStroke>,
    /// Whether screen-share mode is active.
    pub screen_share_mode: bool,
    /// Whether presentation mode is active (single-monitor fullscreen HUD).
    pub presentation_mode: bool,

    // -- Presentation aids --
    /// Whether the laser pointer is active.
    pub laser_active: bool,
    /// Current pointer position (normalized 0..1), None if pointer is off-slide.
    pub pointer_position: Option<(f32, f32)>,
    /// Pointer visual style.
    pub pointer_style: PointerStyle,
    /// Pointer appearance settings for each visual style.
    pub pointer_appearances: PointerAppearances,
    /// Whether ink drawing mode is active.
    pub ink_active: bool,
    /// Active pen settings — used to initialise the next stroke.
    pub active_pen: ActivePen,
    /// Per-page slide ink annotations (`page_index` → strokes).
    pub slide_ink_by_page: HashMap<usize, Vec<InkStroke>>,
    /// Per-page text box overlays (`page_index` → boxes).
    pub slide_text_boxes_by_page: HashMap<usize, Vec<TextBox>>,
    /// Whether text box placement mode is active.
    pub text_box_mode: bool,
    /// The currently selected text box id, if any.
    pub selected_text_box: Option<u64>,
    /// Whether the selected text box is in inline edit mode.
    pub text_box_editing: bool,
    /// Counter for assigning unique text box IDs.
    pub next_text_box_id: u64,
    /// Whether the spotlight overlay is active.
    pub spotlight_active: bool,
    /// Spotlight center position (normalized 0..1).
    pub spotlight_position: Option<(f32, f32)>,
    /// Spotlight radius in logical pixels.
    pub spotlight_radius: f32,
    /// Spotlight dim opacity from 0.0 to 1.0.
    pub spotlight_dim_opacity: f32,
    /// Whether zoom is active on the audience display.
    pub zoom_active: bool,
    /// Current zoom region, if zoom is active.
    pub zoom_region: Option<ZoomRegion>,

    // -- Timer --
    /// Timer state.
    pub timer: TimerState,
    /// Total time spent on the current logical slide during this session.
    pub slide_elapsed: Duration,
    /// Accumulated time spent on each logical slide during this session.
    pub slide_elapsed_by_logical: Vec<Duration>,

    // -- UI --
    /// Whether the slide overview grid is visible.
    pub overview_visible: bool,
    /// Whether the notes panel is visible.
    pub notes_visible: bool,
    /// Whether the notes panel is in markdown edit mode.
    pub notes_editing: bool,
    /// Current notes font size in points.
    pub notes_font_size: f32,
    /// Step size for font increment/decrement.
    pub notes_font_size_step: f32,
    /// Whether a quit confirmation dialog is showing.
    pub quit_requested: bool,

    // -- Content --
    /// Markdown notes for the current logical slide, if any.
    pub current_notes: Option<String>,
}

impl PresentationState {
    /// Create a new state for a presentation with the given slide groups.
    pub fn new(total_pages: usize, slide_groups: Vec<SlideGroup>) -> Self {
        let total_logical_slides = slide_groups.len();
        let current_notes = slide_groups.first().and_then(|g| g.notes.clone());
        Self {
            total_pages,
            slide_groups,
            total_logical_slides,
            current_page: 0,
            current_logical_slide: 0,
            current_overlay_within_group: 0,
            frozen: false,
            frozen_page: None,
            blacked_out: false,
            whiteboard_active: false,
            whiteboard_strokes: Vec::new(),
            screen_share_mode: false,
            presentation_mode: false,
            laser_active: true,
            pointer_position: None,
            pointer_style: PointerStyle::Dot,
            pointer_appearances: PointerAppearances::default(),
            ink_active: false,
            active_pen: ActivePen::default(),
            slide_ink_by_page: HashMap::new(),
            slide_text_boxes_by_page: HashMap::new(),
            text_box_mode: false,
            selected_text_box: None,
            text_box_editing: false,
            next_text_box_id: 1,
            spotlight_active: false,
            spotlight_position: None,
            spotlight_radius: 80.0,
            spotlight_dim_opacity: 0.6,
            zoom_active: false,
            zoom_region: None,
            timer: TimerState::default(),
            slide_elapsed: Duration::ZERO,
            slide_elapsed_by_logical: vec![Duration::ZERO; total_logical_slides],
            overview_visible: false,
            notes_visible: true,
            notes_editing: false,
            notes_font_size: 16.0,
            notes_font_size_step: 2.0,
            quit_requested: false,
            current_notes,
        }
    }

    /// The page the audience should see (respects freeze).
    pub fn audience_page(&self) -> usize {
        self.frozen_page.unwrap_or(self.current_page)
    }

    /// Ink strokes for the current presenter page (read-only convenience for UI).
    pub fn current_page_ink(&self) -> &[InkStroke] {
        self.slide_ink_by_page.get(&self.current_page).map_or(&[], |v| v.as_slice())
    }

    /// Text boxes for the current presenter page (read-only convenience for UI).
    pub fn current_page_text_boxes(&self) -> &[TextBox] {
        self.slide_text_boxes_by_page.get(&self.current_page).map_or(&[], |v| v.as_slice())
    }

    /// Appearance for the currently selected pointer style.
    pub fn current_pointer_appearance(&self) -> PointerAppearance {
        self.pointer_appearances.for_style(self.pointer_style)
    }
}

/// Visual style for the audience laser pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerStyle {
    /// Circular laser dot with glow.
    #[default]
    Dot,
    /// Minimal dot without glow.
    Minimal,
    /// Crosshair with a small center dot.
    Crosshair,
    /// Arrow-style marker.
    Arrow,
    /// Hollow circle pointer.
    Ring,
    /// Ring with a center dot.
    Bullseye,
    /// Translucent filled highlight circle.
    Highlight,
}

/// Color and size for one pointer visual style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerAppearance {
    /// Pointer color as RGBA.
    pub color: [u8; 4],
    /// Pointer size in logical pixels.
    pub size: f32,
}

impl Default for PointerAppearance {
    fn default() -> Self {
        Self { color: [255, 0, 0, 255], size: 12.0 }
    }
}

/// Per-style pointer appearance settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerAppearances {
    /// Appearance for the dot pointer.
    pub dot: PointerAppearance,
    /// Appearance for the minimal dot pointer.
    pub minimal: PointerAppearance,
    /// Appearance for the crosshair pointer.
    pub crosshair: PointerAppearance,
    /// Appearance for the arrow pointer.
    pub arrow: PointerAppearance,
    /// Appearance for the ring pointer.
    pub ring: PointerAppearance,
    /// Appearance for the bullseye pointer.
    pub bullseye: PointerAppearance,
    /// Appearance for the highlight pointer.
    pub highlight: PointerAppearance,
}

impl PointerAppearances {
    /// Return the appearance for a pointer style.
    pub fn for_style(&self, style: PointerStyle) -> PointerAppearance {
        match style {
            PointerStyle::Dot => self.dot,
            PointerStyle::Minimal => self.minimal,
            PointerStyle::Crosshair => self.crosshair,
            PointerStyle::Arrow => self.arrow,
            PointerStyle::Ring => self.ring,
            PointerStyle::Bullseye => self.bullseye,
            PointerStyle::Highlight => self.highlight,
        }
    }
}

impl Default for PointerAppearances {
    fn default() -> Self {
        let default = PointerAppearance::default();
        Self {
            dot: default,
            minimal: default,
            crosshair: default,
            arrow: default,
            ring: default,
            bullseye: default,
            highlight: default,
        }
    }
}

/// A single ink stroke drawn on a slide.
#[derive(Debug, Clone)]
pub struct InkStroke {
    /// Points along the stroke (normalized 0..1 coordinates).
    pub points: Vec<(f32, f32)>,
    /// Stroke color as RGBA — snapshotted from `ActivePen` at stroke creation.
    pub color: [u8; 4],
    /// Stroke width in logical pixels — snapshotted from `ActivePen` at stroke creation.
    pub width: f32,
    /// Whether this stroke is complete (pen lifted).
    pub finished: bool,
}

/// The currently selected pen settings used to initialise the next stroke.
///
/// These are runtime-only; they are not persisted. Changing them never mutates
/// already-existing strokes — each stroke snapshots `ActivePen` at creation time.
#[derive(Debug, Clone, Copy)]
pub struct ActivePen {
    /// Pen color as RGBA (alpha included for highlighter / semitransparent pens).
    pub color: [u8; 4],
    /// Stroke width in logical pixels.
    pub width: f32,
}

impl Default for ActivePen {
    fn default() -> Self {
        Self { color: [255, 0, 0, 255], width: 3.0 }
    }
}

/// Defines a zoom region on the slide.
#[derive(Debug, Clone, Copy)]
pub struct ZoomRegion {
    /// Center of the zoom region (normalized 0..1).
    pub center: (f32, f32),
    /// Magnification factor (e.g., 2.0 = 2x zoom).
    pub factor: f32,
}

/// Timer state for the presentation.
#[derive(Debug, Clone)]
pub struct TimerState {
    /// Timer mode.
    pub mode: TimerMode,
    /// Configured total duration, if any.
    pub duration: Option<Duration>,
    /// Time elapsed since the timer started.
    pub elapsed: Duration,
    /// Whether the timer is currently running.
    pub running: bool,
    /// Threshold for the warning phase.
    pub warning_threshold: Option<Duration>,
}

/// Timer counting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimerMode {
    /// Count up from zero.
    Elapsed,
    /// Count down from the configured duration.
    Countdown,
}

/// Visual phase of the timer, derived from state each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerPhase {
    /// Normal — plenty of time remaining.
    Normal,
    /// Warning — less than the warning threshold remaining.
    Warning,
    /// Overrun — past the configured duration.
    Overrun,
}

impl TimerState {
    /// Compute the current timer phase.
    pub fn phase(&self) -> TimerPhase {
        let Some(duration) = self.duration else {
            return TimerPhase::Normal;
        };

        if self.elapsed >= duration {
            TimerPhase::Overrun
        } else if self.warning_threshold.is_some_and(|warning| self.elapsed + warning >= duration) {
            TimerPhase::Warning
        } else {
            TimerPhase::Normal
        }
    }

    /// The display time for the timer.
    /// For countdown mode, returns time remaining (clamped to 0).
    /// For elapsed mode, returns time elapsed.
    pub fn display_time(&self) -> Duration {
        match self.mode {
            TimerMode::Countdown => {
                self.duration.map_or(self.elapsed, |duration| duration.saturating_sub(self.elapsed))
            }
            TimerMode::Elapsed => self.elapsed,
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            mode: TimerMode::Elapsed,
            duration: None,
            elapsed: Duration::ZERO,
            running: false,
            warning_threshold: None,
        }
    }
}
