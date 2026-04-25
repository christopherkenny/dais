//! Slide thumbnail rendering widget.
//!
//! Renders a PDF page as an egui texture with correct aspect ratio.

use dais_document::page::RenderedPage;
use egui::{Response, Sense, TextureHandle, Ui, Vec2};

/// A reusable widget that displays a rendered PDF page as an egui texture.
pub struct SlideThumbnail {
    texture: Option<TextureHandle>,
    page_index: usize,
    width: u32,
    height: u32,
}

impl SlideThumbnail {
    pub fn new() -> Self {
        Self { texture: None, page_index: usize::MAX, width: 0, height: 0 }
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
        self.texture = Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR));
        self.page_index = page_index;
        self.width = page.width;
        self.height = page.height;
    }

    /// Display the thumbnail in the UI, fitting within `desired_size` while
    /// preserving aspect ratio. Returns the response for the image area.
    #[allow(clippy::cast_precision_loss)]
    pub fn show(&self, ui: &mut Ui, desired_size: Vec2) -> Response {
        let Some(tex) = &self.texture else {
            let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(40));
            return response;
        };

        let tex_aspect = self.width as f32 / self.height.max(1) as f32;
        let box_aspect = desired_size.x / desired_size.y.max(1.0);

        let display_size = if tex_aspect > box_aspect {
            Vec2::new(desired_size.x, desired_size.x / tex_aspect)
        } else {
            Vec2::new(desired_size.y * tex_aspect, desired_size.y)
        };

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        let offset = (desired_size - display_size) / 2.0;
        let image_rect = egui::Rect::from_min_size(rect.min + offset, display_size);

        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);

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
    #[allow(clippy::cast_precision_loss)]
    pub fn show_interactive(&self, ui: &mut Ui, desired_size: Vec2) -> (Response, egui::Rect) {
        self.show_with_sense(ui, desired_size, egui::Sense::click_and_drag())
    }

    /// Display the thumbnail with a caller-provided interaction sense.
    #[allow(clippy::cast_precision_loss)]
    pub fn show_with_sense(
        &self,
        ui: &mut Ui,
        desired_size: Vec2,
        sense: Sense,
    ) -> (Response, egui::Rect) {
        let Some(tex) = &self.texture else {
            let (rect, response) = ui.allocate_exact_size(desired_size, sense);
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(40));
            return (response, rect);
        };

        let tex_aspect = self.width as f32 / self.height.max(1) as f32;
        let box_aspect = desired_size.x / desired_size.y.max(1.0);

        let display_size = if tex_aspect > box_aspect {
            Vec2::new(desired_size.x, desired_size.x / tex_aspect)
        } else {
            Vec2::new(desired_size.y * tex_aspect, desired_size.y)
        };

        let (rect, response) = ui.allocate_exact_size(desired_size, sense);

        let offset = (desired_size - display_size) / 2.0;
        let image_rect = egui::Rect::from_min_size(rect.min + offset, display_size);

        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);

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

    /// Display the thumbnail zoomed into a specific region.
    /// `center` is normalized (0..1, 0..1), `factor` is the zoom multiplier.
    #[allow(clippy::cast_precision_loss)]
    pub fn show_zoomed(
        &self,
        ui: &mut Ui,
        desired_size: Vec2,
        center: (f32, f32),
        factor: f32,
    ) -> Response {
        let Some(tex) = &self.texture else {
            let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(40));
            return response;
        };

        let tex_aspect = self.width as f32 / self.height.max(1) as f32;
        let box_aspect = desired_size.x / desired_size.y.max(1.0);

        let display_size = if tex_aspect > box_aspect {
            Vec2::new(desired_size.x, desired_size.x / tex_aspect)
        } else {
            Vec2::new(desired_size.y * tex_aspect, desired_size.y)
        };

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        let offset = (desired_size - display_size) / 2.0;
        let image_rect = egui::Rect::from_min_size(rect.min + offset, display_size);

        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);

        // Compute UV sub-rect for the zoomed region
        let half_u = 1.0 / (factor * 2.0);
        let half_v = 1.0 / (factor * 2.0);
        let u_center = center.0.clamp(half_u, 1.0 - half_u);
        let v_center = center.1.clamp(half_v, 1.0 - half_v);
        let uv = egui::Rect::from_min_max(
            egui::pos2(u_center - half_u, v_center - half_v),
            egui::pos2(u_center + half_u, v_center + half_v),
        );

        ui.painter().image(tex.id(), image_rect, uv, egui::Color32::WHITE);

        response
    }
}

impl Default for SlideThumbnail {
    fn default() -> Self {
        Self::new()
    }
}
