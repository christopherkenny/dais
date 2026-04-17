/// Dimensions of a document page.
#[derive(Debug, Clone, Copy)]
pub struct PageDimensions {
    /// Width in PDF points (1 point = 1/72 inch).
    pub width_pts: f32,
    /// Height in PDF points.
    pub height_pts: f32,
}

impl PageDimensions {
    /// Aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        self.width_pts / self.height_pts
    }

    /// Whether this is approximately 16:9 (within tolerance).
    pub fn is_widescreen(&self) -> bool {
        (self.aspect_ratio() - 16.0 / 9.0).abs() < 0.05
    }

    /// Whether this is approximately 4:3 (within tolerance).
    pub fn is_standard(&self) -> bool {
        (self.aspect_ratio() - 4.0 / 3.0).abs() < 0.05
    }
}

/// Target size for rendering a page.
///
/// Each window requests rasterization at its own appropriate resolution.
/// This is a runtime parameter, not a global constant — the architectural
/// prerequisite for zoom/magnification features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderSize {
    /// Target width in pixels.
    pub width: u32,
    /// Target height in pixels.
    pub height: u32,
}

/// A rendered page as an RGBA bitmap.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    /// RGBA pixel data (4 bytes per pixel, row-major).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}
