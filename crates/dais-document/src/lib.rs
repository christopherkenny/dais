//! Document source abstraction and PDF rendering for Dais.
//!
//! Defines the [`source::DocumentSource`] trait used by the Hayro-backed PDF renderer and related render pipeline components.

pub mod cache;
pub mod page;
pub mod render_pipeline;
pub mod source;
pub mod typst_renderer;

#[cfg(feature = "hayro")]
pub mod pdf_hayro;

#[cfg(feature = "hayro")]
pub mod typst_export;
