//! Presentation engine — the state authority for Dais.
//!
//! The engine processes commands from the [`CommandBus`](dais_core::bus::CommandBus),
//! maintains the authoritative [`PresentationState`](dais_core::state::PresentationState),
//! and broadcasts state changes to UI subscribers.

pub mod aids;
pub mod engine;
pub mod monitor;
pub mod navigation;
pub mod timer;
