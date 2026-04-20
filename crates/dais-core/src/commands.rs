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
    /// Advance to the first page of the next logical slide group.
    NextSlide,
    /// Go back to the first page of the previous logical slide group.
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

    // -- System --
    /// Quit the application.
    Quit,
    /// Reload configuration from disk.
    ReloadConfig,
    /// Save the current sidecar data to disk.
    SaveSidecar,
}
