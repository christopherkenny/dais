use dais_core::bus::CommandReceiver;
use dais_core::state::PresentationState;

/// The presentation engine — processes commands and owns the authoritative state.
///
/// Called once per frame via `tick()`. All state mutations happen here.
/// The UI reads state via a watch channel and never mutates directly.
pub struct PresentationEngine {
    _receiver: CommandReceiver,
    _state: PresentationState,
}

impl PresentationEngine {
    // TODO: Implement in Phase 4
}
