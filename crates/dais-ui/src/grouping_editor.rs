//! Manual slide grouping editor mode.
//!
//! A standalone `eframe::App` for visually editing slide overlay groups.
//! Launched via `dais --edit <file.pdf>`.

use std::path::{Path, PathBuf};

use dais_document::cache::PageCache;
use dais_document::page::RenderSize;
use dais_document::source::DocumentSource;
use dais_sidecar::format::SidecarFormat;
use dais_sidecar::types::{PresentationMetadata, SlideGroupMeta};

use crate::widgets::SlideThumbnail;

/// The grouping editor application.
pub struct GroupingEditor {
    doc: Box<dyn DocumentSource>,
    cache: PageCache,
    pdf_path: PathBuf,
    sidecar_format: String,
    /// Existing metadata loaded from sidecar (if any).
    metadata: PresentationMetadata,
    /// Group boundaries: a sorted list of page indices where a new group starts.
    /// Page 0 always starts a group (implicit). This stores *additional* boundaries.
    boundaries: Vec<usize>,
    /// Pre-allocated thumbnails (one per page).
    thumbnails: Vec<SlideThumbnail>,
    /// Status message shown briefly after save.
    status_message: Option<(String, std::time::Instant)>,
}

/// Thumbnail display height in logical pixels.
const THUMB_HEIGHT: f32 = 120.0;
/// Render resolution multiplier for higher quality thumbnails.
const RENDER_SCALE: f32 = 3.0;
/// Horizontal spacing between page cells within a group.
const PAGE_CELL_GAP: f32 = 10.0;
/// Vertical spacing between wrapped page rows within a group.
const PAGE_ROW_GAP: f32 = 12.0;
/// Status message display duration.
const STATUS_DURATION_SECS: f64 = 3.0;
/// Alternating group background colors.
const GROUP_BG_A: egui::Color32 = egui::Color32::from_rgb(44, 51, 63);
const GROUP_BG_B: egui::Color32 = egui::Color32::from_rgb(52, 60, 74);
/// Editor background colors.
const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(20, 24, 31);
const TOP_BAR_BG: egui::Color32 = egui::Color32::from_rgb(28, 33, 42);
/// Text colors.
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(241, 245, 249);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(224, 232, 240);
/// Accent color for group editing controls.
const ACTION_COLOR: egui::Color32 = egui::Color32::from_rgb(124, 178, 255);
/// Flatter button fills.
const BUTTON_FILL: egui::Color32 = egui::Color32::from_rgb(58, 72, 92);
const BUTTON_FILL_HOVER: egui::Color32 = egui::Color32::from_rgb(71, 88, 112);
const BUTTON_FILL_ACTIVE: egui::Color32 = egui::Color32::from_rgb(84, 104, 132);

fn flat_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    egui::Button::new(text)
        .fill(BUTTON_FILL)
        .stroke(egui::Stroke::new(1.0, ACTION_COLOR.gamma_multiply(0.65)))
        .corner_radius(4.0)
}

fn small_flat_button(text: impl Into<egui::WidgetText>) -> egui::Button<'static> {
    flat_button(text).small()
}

impl GroupingEditor {
    /// Create a new grouping editor for the given document.
    pub fn new(
        doc: Box<dyn DocumentSource>,
        pdf_path: &Path,
        metadata: PresentationMetadata,
        sidecar_format: &str,
    ) -> Self {
        let page_count = doc.page_count();
        let thumbnails = (0..page_count).map(|_| SlideThumbnail::new()).collect();

        // Convert existing group metadata into boundary set
        let boundaries = groups_to_boundaries(&metadata.groups, page_count);

        Self {
            doc,
            cache: PageCache::new(128),
            pdf_path: pdf_path.to_path_buf(),
            sidecar_format: sidecar_format.to_string(),
            metadata,
            boundaries,
            thumbnails,
            status_message: None,
        }
    }

    /// Compute current groups from the boundary set.
    fn compute_groups(&self) -> Vec<Vec<usize>> {
        let page_count = self.doc.page_count();
        if page_count == 0 {
            return Vec::new();
        }

        let mut all_boundaries: Vec<usize> =
            std::iter::once(0).chain(self.boundaries.iter().copied()).collect();
        all_boundaries.sort_unstable();
        all_boundaries.dedup();

        let mut groups = Vec::new();
        for i in 0..all_boundaries.len() {
            let start = all_boundaries[i];
            let end = if i + 1 < all_boundaries.len() { all_boundaries[i + 1] } else { page_count };
            let pages: Vec<usize> = (start..end).collect();
            if !pages.is_empty() {
                groups.push(pages);
            }
        }
        groups
    }

    /// Toggle a boundary at the given page index.
    fn toggle_boundary(&mut self, page: usize) {
        if page == 0 {
            return; // page 0 is always a boundary
        }
        if let Some(pos) = self.boundaries.iter().position(|&b| b == page) {
            self.boundaries.remove(pos);
        } else {
            self.boundaries.push(page);
            self.boundaries.sort_unstable();
        }
    }

    /// Save groups to the configured sidecar file format.
    fn save_sidecar(&mut self) {
        let groups = self.compute_groups();
        let group_metas: Vec<SlideGroupMeta> = groups
            .iter()
            .map(|g| SlideGroupMeta {
                start_page: *g.first().unwrap_or(&0),
                end_page: *g.last().unwrap_or(&0),
            })
            .collect();

        let mut meta = self.metadata.clone();
        meta.groups = group_metas;

        let (sidecar_path, format): (PathBuf, Box<dyn SidecarFormat>) = if self.sidecar_format
            == "dais"
        {
            (self.pdf_path.with_extension("dais"), Box::new(dais_sidecar::dais_format::DaisFormat))
        } else {
            (self.pdf_path.with_extension("pdfpc"), Box::new(dais_sidecar::pdfpc::PdfpcFormat))
        };

        match format.write(&sidecar_path, &meta) {
            Ok(()) => {
                tracing::info!("Saved grouping to {}", sidecar_path.display());
                self.status_message = Some((
                    format!("Saved to {}", sidecar_path.display()),
                    std::time::Instant::now(),
                ));
                self.metadata = meta;
            }
            Err(e) => {
                tracing::error!("Failed to save sidecar: {e}");
                self.status_message = Some((format!("Error: {e}"), std::time::Instant::now()));
            }
        }
    }

    /// Ensure a page thumbnail is rendered and uploaded.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn ensure_thumbnail(&mut self, ctx: &egui::Context, page_index: usize) {
        let render_height = (THUMB_HEIGHT * RENDER_SCALE) as u32;
        let render_width = (THUMB_HEIGHT * RENDER_SCALE * 16.0 / 9.0) as u32;
        let render_size = RenderSize { width: render_width, height: render_height };

        if self.cache.get(page_index, render_size).is_none()
            && let Ok(rendered) = self.doc.render_page(page_index, render_size)
        {
            self.cache.insert(page_index, render_size, rendered);
        }

        if let Some(page) = self.cache.get(page_index, render_size) {
            let page = page.clone();
            self.thumbnails[page_index].update(ctx, &page, page_index);
        }
    }

    /// Render the top header bar.
    fn show_top_bar(&mut self, ctx: &egui::Context, page_count: usize, group_count: usize) {
        egui::TopBottomPanel::top("grouping_top")
            .frame(egui::Frame::new().fill(TOP_BAR_BG).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("Grouping Editor").color(TEXT_PRIMARY));
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!("{page_count} pages → {group_count} slides"))
                            .color(TEXT_SECONDARY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(flat_button(egui::RichText::new("Close").color(TEXT_PRIMARY)))
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .add(
                                flat_button(
                                    egui::RichText::new("Save").color(TEXT_PRIMARY).strong(),
                                )
                                .fill(ACTION_COLOR.gamma_multiply(0.25)),
                            )
                            .clicked()
                        {
                            self.save_sidecar();
                        }

                        // Status message
                        if let Some((ref msg, when)) = self.status_message
                            && when.elapsed().as_secs_f64() < STATUS_DURATION_SECS
                        {
                            ui.label(egui::RichText::new(msg).color(TEXT_PRIMARY).size(13.0));
                        }
                    });
                });
            });
    }

    /// Render a single group card, returning any boundary toggle requests.
    #[allow(clippy::cast_precision_loss)]
    fn show_group(
        thumbnails: &[SlideThumbnail],
        ui: &mut egui::Ui,
        group: &[usize],
        group_idx: usize,
    ) -> Vec<usize> {
        let card_width = ui.available_width();
        let thumb_width = THUMB_HEIGHT * 16.0 / 9.0;
        let thumb_size = egui::vec2(thumb_width, THUMB_HEIGHT);
        let page_cell_width = thumb_width;
        let frame_inner_margin = 24.0;
        let usable_width = (card_width - frame_inner_margin).max(page_cell_width);
        let pages_per_row = ((usable_width + PAGE_CELL_GAP) / (page_cell_width + PAGE_CELL_GAP))
            .floor()
            .max(1.0) as usize;
        let bg_color = if group_idx.is_multiple_of(2) { GROUP_BG_A } else { GROUP_BG_B };
        let mut toggles = Vec::new();

        ui.allocate_ui(egui::vec2(card_width, 0.0), |ui| {
            egui::Frame::group(ui.style())
                .fill(bg_color)
                .stroke(egui::Stroke::new(1.0, ACTION_COLOR.gamma_multiply(0.2)))
                .corner_radius(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Slide {}", group_idx + 1))
                                .strong()
                                .size(16.0)
                                .color(TEXT_PRIMARY),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!(
                                "{} page{}",
                                group.len(),
                                if group.len() == 1 { "" } else { "s" }
                            ))
                            .color(TEXT_SECONDARY),
                        );
                        if let (Some(first), Some(last)) = (group.first(), group.last()) {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!("pages {}-{}", first + 1, last + 1))
                                    .color(TEXT_SECONDARY),
                            );
                        }
                    });

                    ui.add_space(6.0);

                    let row_count = group.len().div_ceil(pages_per_row);
                    for (row_idx, row) in group.chunks(pages_per_row).enumerate() {
                        ui.horizontal(|ui| {
                            for (col_idx, &page_idx) in row.iter().enumerate() {
                                let page_position = row_idx * pages_per_row + col_idx;
                                ui.vertical(|ui| {
                                    thumbnails[page_idx].show(ui, thumb_size);
                                    ui.label(
                                        egui::RichText::new(format!("Page {}", page_idx + 1))
                                            .size(12.0)
                                            .color(TEXT_PRIMARY),
                                    );

                                    if page_position + 1 < group.len() {
                                        let next_page = group[page_position + 1];
                                        let response = ui
                                            .add(small_flat_button(
                                                egui::RichText::new("Split after")
                                                    .color(TEXT_PRIMARY),
                                            ))
                                            .on_hover_text(format!(
                                                "Start a new slide at page {}",
                                                next_page + 1
                                            ));
                                        if response.clicked() {
                                            toggles.push(next_page);
                                        }
                                    } else {
                                        ui.add_space(ui.spacing().interact_size.y);
                                    }
                                });

                                if col_idx + 1 < row.len() {
                                    ui.add_space(PAGE_CELL_GAP);
                                }
                            }
                        });

                        if row_idx + 1 < row_count {
                            ui.add_space(PAGE_ROW_GAP);
                        }
                    }
                });
        });

        toggles
    }
}

impl eframe::App for GroupingEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let page_count = self.doc.page_count();

        // Pre-render thumbnails
        for i in 0..page_count {
            self.ensure_thumbnail(ctx, i);
        }

        let groups = self.compute_groups();

        self.show_top_bar(ctx, page_count, groups.len());

        // Main scrollable area
        let mut boundary_toggles = Vec::new();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(PANEL_BG))
            .show(ctx, |ui| {
                let visuals = &mut ui.style_mut().visuals;
                visuals.widgets.inactive.bg_fill = BUTTON_FILL;
                visuals.widgets.inactive.weak_bg_fill = BUTTON_FILL;
                visuals.widgets.hovered.bg_fill = BUTTON_FILL_HOVER;
                visuals.widgets.hovered.weak_bg_fill = BUTTON_FILL_HOVER;
                visuals.widgets.active.bg_fill = BUTTON_FILL_ACTIVE;
                visuals.widgets.active.weak_bg_fill = BUTTON_FILL_ACTIVE;
                visuals.widgets.noninteractive.bg_fill = PANEL_BG;
                visuals.widgets.inactive.fg_stroke.color = TEXT_PRIMARY;
                visuals.widgets.hovered.fg_stroke.color = TEXT_PRIMARY;
                visuals.widgets.active.fg_stroke.color = TEXT_PRIMARY;
                visuals.override_text_color = Some(TEXT_PRIMARY);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (group_idx, group) in groups.iter().enumerate() {
                        boundary_toggles
                            .extend(Self::show_group(&self.thumbnails, ui, group, group_idx));

                        if group_idx + 1 < groups.len() {
                            let next_group_first = groups[group_idx + 1][0];
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(flat_button(
                                        egui::RichText::new("Merge with above")
                                        .color(TEXT_PRIMARY),
                                    ))
                                    .on_hover_text(format!(
                                        "Merge this slide group into the one above by removing the boundary before page {}",
                                        next_group_first + 1
                                    ))
                                    .clicked()
                                {
                                    boundary_toggles.push(next_group_first);
                                }
                            });
                            ui.add_space(8.0);
                        }
                    }
                });
            });

        // Apply boundary toggles (deferred to avoid borrow conflicts)
        for page in boundary_toggles {
            self.toggle_boundary(page);
        }
    }
}

/// Convert `SlideGroupMeta` list into a set of boundary page indices.
fn groups_to_boundaries(groups: &[SlideGroupMeta], page_count: usize) -> Vec<usize> {
    if groups.is_empty() {
        return Vec::new();
    }

    let mut boundaries: Vec<usize> =
        groups.iter().map(|g| g.start_page).filter(|&p| p > 0 && p < page_count).collect();
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_groups_produce_no_boundaries() {
        assert!(groups_to_boundaries(&[], 10).is_empty());
    }

    #[test]
    fn single_group_no_boundaries() {
        let groups = vec![SlideGroupMeta { start_page: 0, end_page: 9 }];
        assert!(groups_to_boundaries(&groups, 10).is_empty());
    }

    #[test]
    fn multiple_groups_produce_boundaries() {
        let groups = vec![
            SlideGroupMeta { start_page: 0, end_page: 2 },
            SlideGroupMeta { start_page: 3, end_page: 5 },
            SlideGroupMeta { start_page: 6, end_page: 9 },
        ];
        let boundaries = groups_to_boundaries(&groups, 10);
        assert_eq!(boundaries, vec![3, 6]);
    }

    #[test]
    fn out_of_range_boundaries_filtered() {
        let groups = vec![
            SlideGroupMeta { start_page: 0, end_page: 4 },
            SlideGroupMeta { start_page: 20, end_page: 25 },
        ];
        let boundaries = groups_to_boundaries(&groups, 10);
        assert!(boundaries.is_empty());
    }
}
