//! The 30-second recompute throttle.
//!
//! This is documented product behaviour, not an optimisation: Lexicon
//! recomputes a smartlist when you select it, but at most once every 30
//! seconds, so switching between smartlists stays instant on a large library.
//! The visible consequence — a newly added track can take up to 30s to appear —
//! is stated in the manual, and the UI shows a loading state when a recompute
//! actually runs.
//!
//! Time is injected rather than read from the clock so the behaviour is
//! testable without sleeping.

use std::collections::HashMap;

/// Minimum seconds between recomputes of the same smartlist.
pub const RECOMPUTE_INTERVAL_SECS: i64 = 30;

#[derive(Debug, Clone)]
struct Entry {
    computed_at: i64,
    track_ids: Vec<String>,
}

/// Per-smartlist result cache with a minimum recompute interval.
#[derive(Debug, Default)]
pub struct RecomputeCache {
    entries: HashMap<String, Entry>,
}

impl RecomputeCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cached results for `id`, if they are still fresh at `now`.
    pub fn get(&self, id: &str, now: i64) -> Option<&[String]> {
        let entry = self.entries.get(id)?;
        if now.saturating_sub(entry.computed_at) < RECOMPUTE_INTERVAL_SECS {
            Some(&entry.track_ids)
        } else {
            None
        }
    }

    /// Whether a call at `now` would actually recompute. The UI uses this to
    /// decide whether to show the loading state.
    pub fn would_recompute(&self, id: &str, now: i64) -> bool {
        self.get(id, now).is_none()
    }

    pub fn put(&mut self, id: &str, now: i64, track_ids: Vec<String>) {
        self.entries.insert(
            id.to_string(),
            Entry {
                computed_at: now,
                track_ids,
            },
        );
    }

    /// Drop a smartlist's cached results. Call after editing its rules so the
    /// next read recomputes immediately rather than serving a stale set for up
    /// to 30 seconds.
    pub fn invalidate(&mut self, id: &str) {
        self.entries.remove(id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Serve cached results or recompute via `compute`, storing the result.
    pub fn get_or_compute<F>(&mut self, id: &str, now: i64, compute: F) -> Vec<String>
    where
        F: FnOnce() -> Vec<String>,
    {
        if let Some(hit) = self.get(id, now) {
            return hit.to_vec();
        }
        let fresh = compute();
        self.put(id, now, fresh.clone());
        fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_on_empty_cache() {
        let cache = RecomputeCache::new();
        assert!(cache.get("s1", 100).is_none());
        assert!(cache.would_recompute("s1", 100));
    }

    #[test]
    fn hit_within_the_interval() {
        let mut cache = RecomputeCache::new();
        cache.put("s1", 100, vec!["t1".into()]);
        assert_eq!(cache.get("s1", 100).unwrap(), ["t1".to_string()]);
        assert_eq!(cache.get("s1", 129).unwrap(), ["t1".to_string()]);
        assert!(!cache.would_recompute("s1", 129));
    }

    #[test]
    fn miss_once_the_interval_elapses() {
        let mut cache = RecomputeCache::new();
        cache.put("s1", 100, vec!["t1".into()]);
        assert!(cache.get("s1", 130).is_none());
        assert!(cache.would_recompute("s1", 130));
    }

    #[test]
    fn entries_are_independent_per_smartlist() {
        let mut cache = RecomputeCache::new();
        cache.put("s1", 100, vec!["t1".into()]);
        assert!(cache.get("s2", 100).is_none());
    }

    #[test]
    fn invalidate_forces_an_immediate_recompute() {
        let mut cache = RecomputeCache::new();
        cache.put("s1", 100, vec!["t1".into()]);
        cache.invalidate("s1");
        assert!(cache.get("s1", 100).is_none());
    }

    #[test]
    fn get_or_compute_only_runs_once_within_the_interval() {
        let mut cache = RecomputeCache::new();
        let mut calls = 0;
        let a = cache.get_or_compute("s1", 100, || {
            calls += 1;
            vec!["t1".into()]
        });
        let b = cache.get_or_compute("s1", 120, || {
            calls += 1;
            vec!["t2".into()]
        });
        assert_eq!(a, b);
        assert_eq!(calls, 1);

        let c = cache.get_or_compute("s1", 200, || {
            calls += 1;
            vec!["t2".into()]
        });
        assert_eq!(c, vec!["t2".to_string()]);
        assert_eq!(calls, 2);
    }

    #[test]
    fn clock_going_backwards_does_not_panic() {
        let mut cache = RecomputeCache::new();
        cache.put("s1", 100, vec!["t1".into()]);
        // saturating_sub keeps this a cache hit rather than an overflow.
        assert!(cache.get("s1", 50).is_some());
    }
}
