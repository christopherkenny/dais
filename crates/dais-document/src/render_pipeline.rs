//! Asynchronous render pipeline.
//!
//! Offloads PDF page rendering to a background thread so the UI thread
//! never blocks waiting for hayro.  The UI submits render requests and
//! polls for completed results each frame.

use std::collections::HashSet;
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, TrySendError};

use crate::cache::PageCache;
use crate::page::{RenderSize, RenderedPage};
use crate::source::DocumentSource;

/// A request for the background renderer.
struct RenderRequest {
    page_index: usize,
    size: RenderSize,
}

/// A completed render from the background thread.
struct RenderResult {
    page_index: usize,
    size: RenderSize,
    page: RenderedPage,
}

/// Manages background rendering and result collection.
pub struct RenderPipeline {
    request_tx: Sender<RenderRequest>,
    result_rx: Receiver<RenderResult>,
    /// Pages currently being rendered (to avoid duplicate requests).
    pending: HashSet<(usize, RenderSize)>,
}

/// Default render size used for all display contexts.
/// The GPU handles scaling to any window size with bilinear filtering.
pub const CANONICAL_RENDER_SIZE: RenderSize = RenderSize { width: 1280, height: 720 };

impl RenderPipeline {
    /// Spawn the render pipeline with `num_workers` background threads.
    pub fn new(doc: Arc<dyn DocumentSource>, num_workers: usize) -> Self {
        // Bounded request channel — don't queue hundreds of requests
        let (request_tx, request_rx) = crossbeam_channel::bounded::<RenderRequest>(64);
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<RenderResult>();

        for _ in 0..num_workers {
            let rx = request_rx.clone();
            let tx = result_tx.clone();
            let doc = Arc::clone(&doc);
            std::thread::Builder::new()
                .name("dais-render".into())
                .spawn(move || render_worker(doc, rx, tx))
                .expect("failed to spawn render thread");
        }

        Self {
            request_tx,
            result_rx,
            pending: HashSet::new(),
        }
    }

    /// Poll for completed renders and insert them into the cache.
    /// Call once per frame at the start of `update()`.
    pub fn poll_results(&mut self, cache: &mut PageCache) {
        while let Ok(result) = self.result_rx.try_recv() {
            let key = (result.page_index, result.size);
            self.pending.remove(&key);
            cache.insert(result.page_index, result.size, result.page);
        }
    }

    /// Ensure a page is rendered (or being rendered). If not in cache and not
    /// pending, submits a background render request.
    pub fn ensure_rendered(&mut self, page_index: usize, size: RenderSize, cache: &mut PageCache) {
        if cache.get(page_index, size).is_some() {
            return;
        }
        let key = (page_index, size);
        if self.pending.contains(&key) {
            return;
        }
        match self.request_tx.try_send(RenderRequest { page_index, size }) {
            Ok(()) => {
                self.pending.insert(key);
            }
            Err(TrySendError::Full(_)) => {
                // Queue full — will retry next frame
            }
            Err(TrySendError::Disconnected(_)) => {
                tracing::error!("Render pipeline disconnected");
            }
        }
    }

    /// Request the current page and its neighbors for smooth navigation.
    pub fn prefetch_neighborhood(
        &mut self,
        current_page: usize,
        total_pages: usize,
        size: RenderSize,
        cache: &mut PageCache,
    ) {
        // Current page (highest priority — submitted first)
        self.ensure_rendered(current_page, size, cache);

        // Next page
        if current_page + 1 < total_pages {
            self.ensure_rendered(current_page + 1, size, cache);
        }

        // Previous page (for back-navigation)
        if current_page > 0 {
            self.ensure_rendered(current_page - 1, size, cache);
        }

        // Two pages ahead (look-ahead for fast clicking)
        if current_page + 2 < total_pages {
            self.ensure_rendered(current_page + 2, size, cache);
        }
    }
}

fn render_worker(
    doc: Arc<dyn DocumentSource>,
    rx: Receiver<RenderRequest>,
    tx: Sender<RenderResult>,
) {
    while let Ok(req) = rx.recv() {
        match doc.render_page(req.page_index, req.size) {
            Ok(page) => {
                let _ = tx.send(RenderResult {
                    page_index: req.page_index,
                    size: req.size,
                    page,
                });
            }
            Err(e) => {
                tracing::warn!("Background render failed for page {}: {e}", req.page_index);
            }
        }
    }
}
