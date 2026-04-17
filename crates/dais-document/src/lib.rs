//! Document source abstraction and PDF rendering for Dais.
//!
//! Defines the [`DocumentSource`] trait that abstracts over PDF rendering backends.
//! In v1, the implementation is hayro-backed (or mupdf-rs as fallback).
//! A future native Typst source implementation would be another `DocumentSource`.

pub mod cache;
pub mod page;
pub mod source;

#[cfg(feature = "hayro")]
pub mod pdf_hayro;

#[cfg(feature = "mupdf")]
pub mod pdf_mupdf;
