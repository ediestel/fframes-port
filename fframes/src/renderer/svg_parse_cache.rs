//! Per-worker SVG parse cache.
//!
//! Amortises `usvgr::Tree::from_str` across frames that produce the same SVG document text, such
//! as a runtime (LLM-generated) document held on screen for several frames. Keyed by a content
//! hash of the text. On a hit the cached `Arc<Tree>` is cloned: one refcount increment, no
//! allocation. Trees built by the `svgr!` macro with `compile-time-svgtree` have no text and
//! bypass the cache; their static subtrees are already cached by `SvgrCache`.
//!
//! Backed by `lru::LruCache` with `ahash::RandomState`. Not shared across workers: the renderer
//! is sharded by thread and per-worker caches need no synchronisation.

use ahash::{AHasher, RandomState};
use lru::LruCache;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::Arc;
use usvgr::Tree;

/// The CPU backend keeps one tree per worker: a held frame is a hit, and an animated video keeps
/// no more trees alive than without the cache.
pub const DEFAULT_CACHE_CAPACITY: usize = 1;

/// Per-worker SVG parse cache. See module docs.
pub struct SvgParseCache {
    inner: Option<LruCache<u64, Arc<Tree>, RandomState>>,
    hits: u64,
    misses: u64,
}

impl SvgParseCache {
    /// Build a cache with an explicit capacity. A capacity of 0
    /// produces a disabled cache (see [`SvgParseCache::disabled`]).
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::disabled();
        }
        let cap = NonZeroUsize::new(capacity).expect("capacity > 0 checked above");
        Self {
            inner: Some(LruCache::with_hasher(cap, RandomState::new())),
            hits: 0,
            misses: 0,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CACHE_CAPACITY)
    }

    /// Build a cache that never stores anything. Every `get_or_parse`
    /// is a miss; every call to `parse` runs. Use when the caller
    /// knows the SVG varies per frame and caching would pay pure
    /// overhead.
    pub fn disabled() -> Self {
        Self {
            inner: None,
            hits: 0,
            misses: 0,
        }
    }

    /// Return the cached tree for `hash` (plus a `hit: true` signal)
    /// or invoke `parse` to build one and store it (with `hit:
    /// false`). Cache hits bump the stored `Arc` refcount (zero
    /// allocation); cache misses parse, wrap, insert.
    ///
    pub fn get_or_parse<F, E>(&mut self, hash: u64, parse: F) -> Result<(Arc<Tree>, bool), E>
    where
        F: FnOnce() -> Result<Tree, E>,
    {
        if let Some(cache) = self.inner.as_mut() {
            if let Some(tree) = cache.get(&hash) {
                self.hits += 1;
                return Ok((Arc::clone(tree), true));
            }
        }

        self.misses += 1;
        let tree = Arc::new(parse()?);
        if let Some(cache) = self.inner.as_mut() {
            cache.put(hash, Arc::clone(&tree));
        }
        Ok((tree, false))
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Hit rate as a fraction in `[0, 1]`. Returns 0 if no lookups
    /// have been recorded yet.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }
}

/// Content hash for SVG document text — `ahash`, stable within a process run,
/// non-DoS-vulnerable. The cache key of [`SvgParseCache`].
pub fn content_hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = AHasher::default();
    bytes.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_minimal() -> Result<Tree, usvgr::Error> {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#;
        let opt = usvgr::Options::default();
        let fontdb = usvgr::fontdb::Database::new();
        Tree::from_str(svg, &opt, &fontdb)
    }

    #[test]
    fn hit_on_second_call() {
        let mut cache = SvgParseCache::new(4);
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(42, parse_minimal)
            .unwrap()
            .0;
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(42, parse_minimal)
            .unwrap()
            .0;
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn evicts_past_capacity() {
        let mut cache = SvgParseCache::new(2);
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap()
            .0;
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(2, parse_minimal)
            .unwrap()
            .0;
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(3, parse_minimal)
            .unwrap()
            .0;
        // hash 1 should now be evicted
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap()
            .0;
        assert_eq!(cache.misses(), 4);
        assert_eq!(cache.hits(), 0);
    }

    #[test]
    fn promotes_accessed_entry_to_front() {
        let mut cache = SvgParseCache::new(2);
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap()
            .0;
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(2, parse_minimal)
            .unwrap()
            .0;
        // access 1 — LRU promotion to MRU slot
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap()
            .0;
        // insert 3 — evicts 2 (LRU), not 1
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(3, parse_minimal)
            .unwrap()
            .0;
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap()
            .0;
        assert_eq!(cache.hits(), 2);
    }

    #[test]
    fn disabled_never_stores() {
        let mut cache = SvgParseCache::disabled();
        assert!(!cache.is_enabled());
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        assert_eq!(cache.hits(), 0);
        assert_eq!(cache.misses(), 2);
    }

    #[test]
    fn capacity_zero_is_disabled() {
        let cache = SvgParseCache::new(0);
        assert!(!cache.is_enabled());
    }

    #[test]
    fn hit_rate_reports_fraction() {
        let mut cache = SvgParseCache::new(4);
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        let _ = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        // 1 miss, 2 hits → 2/3
        assert!((cache.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn hit_rate_zero_before_lookups() {
        let cache = SvgParseCache::new(4);
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn hit_flag_reports_outcome_directly() {
        let mut cache = SvgParseCache::new(4);
        let (_tree, hit1) = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        let (_tree, hit2) = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        assert!(!hit1, "first lookup is a miss");
        assert!(hit2, "second lookup with same key is a hit");
    }

    #[test]
    fn disabled_cache_always_reports_miss() {
        let mut cache = SvgParseCache::disabled();
        let (_, hit1) = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        let (_, hit2) = cache
            .get_or_parse::<_, usvgr::Error>(1, parse_minimal)
            .unwrap();
        assert!(!hit1);
        assert!(!hit2);
    }

    #[test]
    fn content_hash_is_stable_within_run() {
        let a = content_hash_bytes(b"hello");
        let b = content_hash_bytes(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn content_hash_differs_for_different_inputs() {
        let a = content_hash_bytes(b"hello");
        let b = content_hash_bytes(b"world");
        assert_ne!(a, b);
    }
}
