//! Sidecar format abstraction and `.pdfpc` implementation for Dais.
//!
//! This crate defines Dais's internal presentation metadata types and provides
//! a trait-based abstraction for reading/writing sidecar formats. The `.pdfpc`
//! format is the v1 implementation; a future `.dais` format slots in as an
//! additional implementation.

pub mod dais_format;
pub mod format;
pub mod metadata;
pub mod pdfpc;
pub mod types;
