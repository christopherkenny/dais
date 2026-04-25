use std::collections::HashMap;

use crate::page::{RenderSize, RenderedPage};

/// LRU cache for rendered pages, keyed on (`page_index`, `render_size`).
///
/// Each window gets pages rendered at its own resolution, so the same page
/// may be cached at multiple sizes (e.g., presenter DPI and audience DPI).
pub struct PageCache {
    entries: HashMap<CacheKey, RenderedPage>,
    order: Vec<CacheKey>,
    capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
    page_index: usize,
    size: RenderSize,
}

impl PageCache {
    /// Create a new cache with the given capacity (number of entries).
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            order: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a cached page, if available.
    pub fn get(&mut self, page_index: usize, size: RenderSize) -> Option<&RenderedPage> {
        let key = CacheKey { page_index, size };
        if self.entries.contains_key(&key) {
            // Move to end (most recently used)
            self.order.retain(|k| *k != key);
            self.order.push(key);
            self.entries.get(&key)
        } else {
            None
        }
    }

    /// Insert a rendered page into the cache, evicting the least recently used if full.
    pub fn insert(&mut self, page_index: usize, size: RenderSize, page: RenderedPage) {
        let key = CacheKey { page_index, size };

        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            // Evict LRU
            if let Some(evicted) = self.order.first().copied() {
                self.order.remove(0);
                self.entries.remove(&evicted);
            }
        }

        self.order.retain(|k| *k != key);
        self.order.push(key);
        self.entries.insert(key, page);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_page(id: u8) -> RenderedPage {
        RenderedPage { data: vec![id; 4], width: 1, height: 1 }
    }

    #[test]
    fn cache_insert_and_get() {
        let mut cache = PageCache::new(5);
        let size = RenderSize { width: 1920, height: 1080 };
        cache.insert(0, size, dummy_page(0));
        assert!(cache.get(0, size).is_some());
        assert!(cache.get(1, size).is_none());
    }

    #[test]
    fn cache_evicts_lru() {
        let mut cache = PageCache::new(2);
        let size = RenderSize { width: 1920, height: 1080 };
        cache.insert(0, size, dummy_page(0));
        cache.insert(1, size, dummy_page(1));
        cache.insert(2, size, dummy_page(2)); // should evict page 0
        assert!(cache.get(0, size).is_none());
        assert!(cache.get(1, size).is_some());
        assert!(cache.get(2, size).is_some());
    }
}
