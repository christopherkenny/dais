//! Sidecar format abstraction and `.pdfpc` implementation for Dais.
//!
//! This crate defines Dais's internal presentation metadata types and provides
//! a trait-based abstraction for reading/writing sidecar formats. The `.pdfpc`
//! format provides compatibility with existing presenter workflows, while the
//! Dais-native format stores richer application metadata.

pub mod dais_format;
pub mod format;
pub mod metadata;
pub mod pdfpc;
pub mod types;
