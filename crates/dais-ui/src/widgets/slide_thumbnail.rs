//! Slide thumbnail rendering widget.
//!
//! Renders a PDF page as an egui texture with correct aspect ratio.

use dais_document::page::RenderedPage;
use egui::{Response, TextureHandle, Ui, Vec2};

/// A reusable widget that displays a rendered PDF page as an egui texture.
pub struct SlideThumbnail {
    texture: Option<TextureHandle>,
    page_index: usize,
    width: u32,
    height: u32,
}

impl SlideThumbnail {
    pub fn new() -> Self {
        Self {
            texture: None,
            page_index: usize::MAX,
            width: 0,
            height: 0,
        }
    }

    /// Upload new page data to the GPU texture, only if the page changed.
    pub fn update(&mut self, ctx: &egui::Context, page: &RenderedPage, page_index: usize) {
        if self.page_index == page_index && self.width == page.width && self.height == page.height {
            return;
        }

        let color_image = egui::ColorImage::from_rgba_premultiplied(
            [page.width as usize, page.height as usize],
            &page.data,
        );
        let name = format!("slide_{page_index}_{}", page.width);
        self.texture =
            Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR));
        self.page_index = page_index;
        self.width = page.width;
        self.height = page.height;
    }

    /// Display the thumbnail in the UI, fitting within `desired_size` while
    /// preserving aspect ratio. Returns the response for the image area.
    pub fn show(&self, ui: &mut Ui, desired_size: Vec2) -> Response {
        let Some(tex) = &self.texture else {
            // No texture yet — draw a placeholder rect
            let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_gray(40));
            return response;
        };

        let tex_aspect = self.width as f32 / self.height.max(1) as f32;
        let box_aspect = desired_size.x / desired_size.y.max(1.0);

        let display_size = if tex_aspect > box_aspect {
            // Width-limited
            Vec2::new(desired_size.x, desired_size.x / tex_aspect)
        } else {
            // Height-limited
            Vec2::new(desired_size.y * tex_aspect, desired_size.y)
        };

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        // Center the image within the allocated rect
        let offset = (desired_size - display_size) / 2.0;
        let image_rect = egui::Rect::from_min_size(rect.min + offset, display_size);

        // Fill background (letterbox/pillarbox)
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::BLACK);

        ui.painter().image(
            tex.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        response
    }

    /// Like `show`, but makes the thumbnail clickable and returns both the
    /// response and the image rect (for coordinate normalization).
    pub fn show_interactive(
        &self,
        ui: &mut Ui,
        desired_size: Vec2,
    ) -> (Response, egui::Rect) {
        let Some(tex) = &self.texture else {
            let (rect, response) =
                ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_gray(40));
            return (response, rect);
        };

        let tex_aspect = self.width as f32 / self.height.max(1) as f32;
        let box_aspect = desired_size.x / desired_size.y.max(1.0);

        let display_size = if tex_aspect > box_aspect {
            Vec2::new(desired_size.x, desired_size.x / tex_aspect)
        } else {
            Vec2::new(desired_size.y * tex_aspect, desired_size.y)
        };

        let (rect, response) =
            ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        let offset = (desired_size - display_size) / 2.0;
        let image_rect = egui::Rect::from_min_size(rect.min + offset, display_size);

        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::BLACK);

        ui.painter().image(
            tex.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        (response, image_rect)
    }

    pub fn has_texture(&self) -> bool {
        self.texture.is_some()
    }
}

impl Default for SlideThumbnail {
    fn default() -> Self {
        Self::new()
    }
}
