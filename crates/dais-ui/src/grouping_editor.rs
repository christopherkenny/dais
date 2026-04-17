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
const THUMB_HEIGHT: f32 = 140.0;
/// Separator width between groups.
const GROUP_SEP_WIDTH: f32 = 6.0;
/// Clickable gap between thumbnails within a group.
const INNER_GAP: f32 = 8.0;
/// Padding inside each group container.
const GROUP_PADDING: f32 = 8.0;
/// Status message display duration.
const STATUS_DURATION_SECS: f64 = 3.0;
/// Alternating group background colors.
const GROUP_BG_A: egui::Color32 = egui::Color32::from_gray(45);
const GROUP_BG_B: egui::Color32 = egui::Color32::from_gray(55);
/// Group separator color.
const SEP_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 160, 255);

impl GroupingEditor {
    /// Create a new grouping editor for the given document.
    pub fn new(
        doc: Box<dyn DocumentSource>,
        pdf_path: &Path,
        metadata: PresentationMetadata,
    ) -> Self {
        let page_count = doc.page_count();
        let thumbnails = (0..page_count).map(|_| SlideThumbnail::new()).collect();

        // Convert existing group metadata into boundary set
        let boundaries = groups_to_boundaries(&metadata.groups, page_count);

        Self {
            doc,
            cache: PageCache::new(128),
            pdf_path: pdf_path.to_path_buf(),
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

    /// Save groups to a `.pdfpc` sidecar file.
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

        let sidecar_path = self.pdf_path.with_extension("pdfpc");
        let format = dais_sidecar::pdfpc::PdfpcFormat;
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
        let render_size =
            RenderSize { width: (THUMB_HEIGHT * 16.0 / 9.0) as u32, height: THUMB_HEIGHT as u32 };

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
        egui::TopBottomPanel::top("grouping_top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Grouping Editor");
                ui.separator();
                ui.label(format!("{page_count} pages → {group_count} slides"));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕ Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("💾 Save").clicked() {
                        self.save_sidecar();
                    }

                    // Status message
                    if let Some((ref msg, when)) = self.status_message
                        && when.elapsed().as_secs_f64() < STATUS_DURATION_SECS
                    {
                        ui.label(
                            egui::RichText::new(msg).color(egui::Color32::LIGHT_GREEN).size(13.0),
                        );
                    }
                });
            });
        });
    }

    /// Render a single group of thumbnails, returning any boundary toggle request.
    #[allow(clippy::cast_precision_loss)]
    fn show_group(
        thumbnails: &[SlideThumbnail],
        ui: &mut egui::Ui,
        group: &[usize],
        group_idx: usize,
    ) -> Option<usize> {
        let thumb_width = THUMB_HEIGHT * 16.0 / 9.0;
        let thumb_size = egui::vec2(thumb_width, THUMB_HEIGHT);
        let bg_color = if group_idx.is_multiple_of(2) { GROUP_BG_A } else { GROUP_BG_B };
        let mut toggle_page = None;

        ui.vertical(|ui| {
            // Group background
            let n = group.len();
            let group_width = thumb_width
                .mul_add(n as f32, INNER_GAP * (n.saturating_sub(1)) as f32)
                + GROUP_PADDING;
            let (bg_rect, _) =
                ui.allocate_exact_size(egui::vec2(group_width, 0.0), egui::Sense::hover());
            let full_bg = egui::Rect::from_min_size(
                bg_rect.min,
                egui::vec2(group_width, THUMB_HEIGHT + 30.0),
            );
            ui.painter().rect_filled(full_bg, 4.0, bg_color);

            // Thumbnails row
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                for (i, &page_idx) in group.iter().enumerate() {
                    if i > 0 {
                        let (gap_rect, gap_resp) = ui.allocate_exact_size(
                            egui::vec2(INNER_GAP, THUMB_HEIGHT),
                            egui::Sense::click(),
                        );
                        if gap_resp.clicked() {
                            toggle_page = Some(page_idx);
                        }
                        if gap_resp.hovered() {
                            ui.painter().rect_filled(gap_rect, 2.0, SEP_COLOR.gamma_multiply(0.4));
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }

                    thumbnails[page_idx].show(ui, thumb_size);
                }
                ui.add_space(4.0);
            });

            // Group label
            let label_text = format!(
                "Slide {} ({} page{})",
                group_idx + 1,
                group.len(),
                if group.len() == 1 { "" } else { "s" }
            );
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(label_text).size(12.0).color(egui::Color32::WHITE));
            });
        });

        toggle_page
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

        // Help bar
        egui::TopBottomPanel::bottom("grouping_help").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        "Click between thumbnails to add/remove group boundaries. \
                         Groups shown with colored separators.",
                    )
                    .size(12.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );
            });
        });

        // Main scrollable area
        let mut boundary_toggles = Vec::new();

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_gray(30)))
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (group_idx, group) in groups.iter().enumerate() {
                            if let Some(page) =
                                Self::show_group(&self.thumbnails, ui, group, group_idx)
                            {
                                boundary_toggles.push(page);
                            }

                            // Group separator
                            if group_idx + 1 < groups.len() {
                                let next_group_first = groups[group_idx + 1][0];
                                let (sep_rect, sep_resp) = ui.allocate_exact_size(
                                    egui::vec2(GROUP_SEP_WIDTH, THUMB_HEIGHT + 30.0),
                                    egui::Sense::click(),
                                );
                                ui.painter().rect_filled(sep_rect, 2.0, SEP_COLOR);

                                if sep_resp.clicked() {
                                    boundary_toggles.push(next_group_first);
                                }
                                if sep_resp.hovered() {
                                    ui.painter().rect_filled(
                                        sep_rect.expand(2.0),
                                        2.0,
                                        SEP_COLOR.gamma_multiply(0.6),
                                    );
                                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        }
                    });
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
