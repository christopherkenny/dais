//! Core types, command bus, and state machine for Dais.
//!
//! This crate contains the foundational types shared across all Dais crates:
//! - [`Command`] — every user action as a typed message
//! - [`PresentationState`] — the single authoritative state struct
//! - [`CommandBus`] — MPSC command dispatcher
//! - Configuration and keybinding types
//! - Slide grouping model

pub mod bus;
pub mod commands;
pub mod config;
pub mod keybindings;
pub mod slide_group;
pub mod state;
