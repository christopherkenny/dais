//! Core types, command bus, and state machine for Dais.
//!
//! This crate contains the foundational types shared across all Dais crates:
//! - [`commands::Command`] — every user action as a typed message
//! - [`state::PresentationState`] — the single authoritative state struct
//! - [`bus::CommandBus`] — MPSC command dispatcher
//! - Configuration and keybinding types
//! - Slide grouping model
//! - Monitor management trait

pub mod bus;
pub mod commands;
pub mod config;
pub mod keybindings;
pub mod monitor;
pub mod slide_group;
pub mod state;
