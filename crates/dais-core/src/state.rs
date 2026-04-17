use std::time::Duration;

use crate::slide_group::SlideGroup;

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
    /// Whether screen-share mode is active.
    pub screen_share_mode: bool,

    // -- Presentation aids --
    /// Whether the laser pointer is active.
    pub laser_active: bool,
    /// Current pointer position (normalized 0..1), None if pointer is off-slide.
    pub pointer_position: Option<(f32, f32)>,
    /// Whether ink drawing mode is active.
    pub ink_active: bool,
    /// Ink strokes on the current page.
    pub ink_strokes: Vec<InkStroke>,
    /// Whether the spotlight overlay is active.
    pub spotlight_active: bool,
    /// Spotlight center position (normalized 0..1).
    pub spotlight_position: Option<(f32, f32)>,
    /// Whether zoom is active on the audience display.
    pub zoom_active: bool,
    /// Current zoom region, if zoom is active.
    pub zoom_region: Option<ZoomRegion>,

    // -- Timer --
    /// Timer state.
    pub timer: TimerState,

    // -- UI --
    /// Whether the slide overview grid is visible.
    pub overview_visible: bool,
    /// Whether the notes panel is visible.
    pub notes_visible: bool,
    /// Current notes font size in points.
    pub notes_font_size: f32,

    // -- Content --
    /// Markdown notes for the current logical slide, if any.
    pub current_notes: Option<String>,
}

/// A single ink stroke drawn on a slide.
#[derive(Debug, Clone)]
pub struct InkStroke {
    /// Points along the stroke (normalized 0..1 coordinates).
    pub points: Vec<(f32, f32)>,
    /// Stroke color as RGBA.
    pub color: [u8; 4],
    /// Stroke width in logical pixels.
    pub width: f32,
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
    /// Configured total duration.
    pub duration: Duration,
    /// Time elapsed since the timer started.
    pub elapsed: Duration,
    /// Whether the timer is currently running.
    pub running: bool,
    /// Threshold for the warning phase.
    pub warning_threshold: Duration,
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
        match self.mode {
            TimerMode::Countdown => {
                if self.elapsed >= self.duration {
                    TimerPhase::Overrun
                } else if self.elapsed + self.warning_threshold >= self.duration {
                    TimerPhase::Warning
                } else {
                    TimerPhase::Normal
                }
            }
            TimerMode::Elapsed => {
                if self.elapsed >= self.duration {
                    TimerPhase::Overrun
                } else if self.elapsed + self.warning_threshold >= self.duration {
                    TimerPhase::Warning
                } else {
                    TimerPhase::Normal
                }
            }
        }
    }
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            mode: TimerMode::Countdown,
            duration: Duration::from_mins(20),
            elapsed: Duration::ZERO,
            running: false,
            warning_threshold: Duration::from_mins(5),
        }
    }
}
