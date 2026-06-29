/// Every user action in Dais is represented as a discrete typed command.
///
/// Commands are dispatched through the [`CommandBus`](crate::bus::CommandBus) and processed
/// by the presentation engine. The UI never mutates state directly — all mutations flow
/// through commands.
///
/// This design enables future external control surfaces (REST API, WebSocket, mobile remote)
/// to be added as new command sources without modifying the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // -- Navigation --
    /// Advance to the next build step, or to the next logical slide if the current slide is complete.
    NextSlide,
    /// Go back to the previous build step, or to the previous logical slide if already at the start.
    PreviousSlide,
    /// Advance one PDF page (next overlay/build step within a group).
    NextOverlay,
    /// Go back one PDF page (previous overlay/build step).
    PreviousOverlay,
    /// Jump to the first slide.
    FirstSlide,
    /// Jump to the last slide.
    LastSlide,
    /// Jump to a specific logical slide by index (0-based).
    GoToSlide(usize),

    // -- Display modes --
    /// Freeze/unfreeze the audience display.
    ToggleFreeze,
    /// Black out/restore the audience display.
    ToggleBlackout,
    /// Toggle the whiteboard (blank white drawing canvas on audience).
    ToggleWhiteboard,
    /// Toggle screen-share mode (audience window as normal window).
    ToggleScreenShareMode,
    /// Toggle presentation mode (single-monitor: console ↔ fullscreen HUD).
    TogglePresentationMode,
    /// Swap presenter and audience monitors for this session.
    SwapDisplays,

    // -- Presentation aids --
    /// Toggle the laser pointer on/off.
    ToggleLaser,
    /// Cycle the laser pointer visual style.
    CycleLaserStyle,
    /// Update the pointer position (normalized 0..1 coordinates relative to slide).
    SetPointerPosition(f32, f32),
    /// Toggle freehand ink drawing mode.
    ToggleInk,
    /// Add a point to the current ink stroke (normalized coordinates).
    AddInkPoint(f32, f32),
    /// Finish the current ink stroke (pen lifted).
    FinishInkStroke,
    /// Clear all ink on the current page.
    ClearInk,
    /// Set the active pen color (RGBA). Affects only future strokes.
    SetInkColor([u8; 4]),
    /// Set the active pen width in logical pixels. Affects only future strokes.
    SetInkWidth(f32),
    /// Cycle the active pen color through the built-in preset list.
    CycleInkColor,
    /// Cycle the active pen width through the built-in preset list.
    CycleInkWidth,
    /// Toggle the spotlight overlay.
    ToggleSpotlight,
    /// Update the spotlight center position (normalized coordinates).
    SetSpotlightPosition(f32, f32),
    /// Toggle zoom mode on the audience display.
    ToggleZoom,
    /// Set the zoom region center and magnification factor.
    SetZoomRegion { center: (f32, f32), factor: f32 },

    // -- Timer --
    /// Start the timer (or resume if paused).
    StartTimer,
    /// Pause the timer.
    PauseTimer,
    /// Toggle the timer between running and paused.
    ToggleTimer,
    /// Reset the timer to its initial state.
    ResetTimer,

    // -- UI panels --
    /// Toggle the slide overview grid.
    ToggleSlideOverview,
    /// Toggle the notes panel visibility.
    ToggleNotesPanel,
    /// Toggle inline markdown editing for the current slide's notes.
    ToggleNotesEdit,
    /// Replace the current slide's notes markdown.
    SetCurrentSlideNotes(String),
    /// Increase notes font size by one step.
    IncrementNotesFontSize,
    /// Decrease notes font size by one step.
    DecrementNotesFontSize,

    // -- Text boxes --
    /// Toggle text box placement mode.
    ToggleTextBoxMode,
    /// Place a new text box on the current slide (normalized coordinates).
    PlaceTextBox { x: f32, y: f32, w: f32, h: f32 },
    /// Update the content of a text box.
    EditTextBoxContent { id: u64, content: String },
    /// Move a text box to a new position (normalized coordinates).
    MoveTextBox { id: u64, x: f32, y: f32 },
    /// Resize a text box (normalized size).
    ResizeTextBox { id: u64, w: f32, h: f32 },
    /// Delete a text box by ID.
    DeleteTextBox { id: u64 },
    /// Select a text box by ID.
    SelectTextBox(u64),
    /// Deselect the currently selected text box.
    DeselectTextBox,
    /// Enter inline edit mode for the selected text box.
    BeginTextBoxEdit { id: u64 },
    /// Set the font size of a text box.
    SetTextBoxFontSize { id: u64, size: f32 },
    /// Set the text color of a text box (RGBA).
    SetTextBoxColor { id: u64, color: [u8; 4] },
    /// Set the background fill of a text box (RGBA, or None to clear).
    SetTextBoxBackground { id: u64, color: Option<[u8; 4]> },

    // -- System --
    /// Quit the application.
    Quit,
    /// Save the current sidecar data to disk.
    SaveSidecar,
}
