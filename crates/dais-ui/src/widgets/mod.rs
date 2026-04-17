//! Reusable UI widgets.

pub mod ink_canvas;
pub mod slide_thumbnail;
pub mod toast;

pub use ink_canvas::draw_ink_strokes;
pub use slide_thumbnail::SlideThumbnail;
pub use toast::ToastManager;
