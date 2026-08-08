//! Which tracks point at files that are not there, and how often we re-check.
//!
//! Per `docs/lexicon/07-health.md §Find Lost Tracks / Relocate`: "missing-state
//! is re-checked **at most every 5 minutes**; opening a track's Edit popup or
//! restarting forces a re-check."
//!
//! The cadence is not an optimisation bolted on afterwards — it is what makes
//! the check affordable. Every call stats one file per track, so on a
//! four-thousand-track library over a network volume an uncached check is
//! seconds of disk I/O, and the browser asks for this on every render that
//! touches the missing-file filter.
//!
//! The cache lives in memory rather than in the cache DB, which gives the
//! "restarting forces a re-check" half for free: there is nothing to persist,
//! so a fresh process has nothing to serve and scans.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use tauri::Manager;

/// How long a scan's answer is considered current, in seconds.
pub const FRESHNESS_SECS: i64 = 300;

#[derive(Debug, Clone)]
struct Entry {
    scanned_at: i64,
    missing: Vec<String>,
}

/// Per-library memo of the last missing-file scan.
#[derive(Debug, Default)]
pub struct MissingCache {
    by_library: Mutex<HashMap<String, Entry>>,
}

impl MissingCache {
    /// The cached answer for `library_path`, if it is still fresh at `now`.
    ///
    /// Exactly `FRESHNESS_SECS` old counts as stale rather than fresh. The spec
    /// says "at most every 5 minutes", so the boundary belongs on the
    /// re-scanning side — being a second too eager is harmless, being a second
    /// too slow means the window is not what it claims.
    pub fn get(&self, library_path: &str, now: i64) -> Option<Vec<String>> {
        let map = self.by_library.lock().ok()?;
        let entry = map.get(library_path)?;
        // A clock that moved backwards (NTP, a timezone-less VM resuming)
        // yields a negative age; treat that as stale rather than as
        // indefinitely fresh, which is what a plain `<` comparison would do.
        let age = now - entry.scanned_at;
        if (0..FRESHNESS_SECS).contains(&age) {
            Some(entry.missing.clone())
        } else {
            None
        }
    }

    pub fn put(&self, library_path: &str, now: i64, missing: Vec<String>) {
        if let Ok(mut map) = self.by_library.lock() {
            map.insert(
                library_path.to_string(),
                Entry {
                    scanned_at: now,
                    missing,
                },
            );
        }
    }

    /// Drop the memo for one library, so the next call re-scans.
    ///
    /// This is the "opening a track's Edit popup forces a re-check" half: the
    /// caller invalidates rather than passing a bypass flag, so a forced check
    /// also refreshes what everything else will see. A bypass would give the
    /// popup a fresh answer while the browser kept showing a stale one.
    pub fn invalidate(&self, library_path: &str) {
        if let Ok(mut map) = self.by_library.lock() {
            map.remove(library_path);
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Track ids whose file is not on disk.
///
/// Served from a per-library memo for up to five minutes. `force` skips the
/// memo *and* refreshes it, so a forced check does not leave the rest of the UI
/// looking at an older answer.
#[tauri::command]
pub async fn list_tracks_with_missing_files(
    app: tauri::AppHandle,
    path: String,
    force: Option<bool>,
) -> Result<Vec<String>, String> {
    let cache = app.state::<MissingCache>();
    if force.unwrap_or(false) {
        cache.invalidate(&path);
    } else if let Some(hit) = cache.get(&path, now_secs()) {
        return Ok(hit);
    }

    // A track that a Local Path Mapping resolves is not missing — reporting it
    // would send the user relocating files that are already there.
    let mappings = crate::organizer::path_mappings(&app);
    let scan_path = path.clone();
    let missing = tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&scan_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        Ok::<Vec<String>, String>(
            tracks
                .into_iter()
                .filter(|t| {
                    t.folder_path
                        .as_deref()
                        .map(|p| !mappings.resolve(p).exists())
                        .unwrap_or(false)
                })
                .map(|t| t.id)
                .collect(),
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    app.state::<MissingCache>()
        .put(&path, now_secs(), missing.clone());
    Ok(missing)
}

/// Force the next missing-file check to re-scan.
///
/// The spec's "opening a track's Edit popup forces a re-check": the popup calls
/// this, and the following check is live.
#[tauri::command]
pub fn invalidate_missing_files(app: tauri::AppHandle, path: String) {
    app.state::<MissingCache>().invalidate(&path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_cold_cache_has_no_answer() {
        let cache = MissingCache::default();
        assert_eq!(cache.get("/db", 1000), None);
    }

    #[test]
    fn a_fresh_answer_is_served_from_the_memo() {
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a", "b"]));
        assert_eq!(cache.get("/db", 1000 + 299), Some(ids(&["a", "b"])));
    }

    #[test]
    fn the_boundary_is_stale_not_fresh() {
        // "At most every 5 minutes" — the boundary belongs on the re-scanning
        // side, so the window is never longer than it claims.
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a"]));
        assert_eq!(cache.get("/db", 1000 + FRESHNESS_SECS), None);
    }

    #[test]
    fn an_expired_answer_is_not_served() {
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a"]));
        assert_eq!(cache.get("/db", 1000 + 601), None);
    }

    #[test]
    fn a_backwards_clock_expires_the_memo_rather_than_freezing_it() {
        // NTP, or a VM resuming with a stale clock. A plain `now - then <
        // FRESHNESS` test would treat a negative age as fresh forever.
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a"]));
        assert_eq!(cache.get("/db", 500), None);
    }

    #[test]
    fn memos_are_per_library() {
        let cache = MissingCache::default();
        cache.put("/a", 1000, ids(&["x"]));
        assert_eq!(cache.get("/b", 1000), None);
        assert_eq!(cache.get("/a", 1000), Some(ids(&["x"])));
    }

    #[test]
    fn invalidating_forces_the_next_check_to_rescan() {
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a"]));
        cache.invalidate("/db");
        assert_eq!(cache.get("/db", 1000), None);
    }

    #[test]
    fn invalidating_one_library_leaves_the_others_alone() {
        let cache = MissingCache::default();
        cache.put("/a", 1000, ids(&["x"]));
        cache.put("/b", 1000, ids(&["y"]));
        cache.invalidate("/a");
        assert_eq!(cache.get("/a", 1000), None);
        assert_eq!(cache.get("/b", 1000), Some(ids(&["y"])));
    }

    #[test]
    fn a_later_scan_replaces_an_earlier_one() {
        let cache = MissingCache::default();
        cache.put("/db", 1000, ids(&["a", "b"]));
        cache.put("/db", 1400, ids(&["a"]));
        assert_eq!(cache.get("/db", 1400), Some(ids(&["a"])));
    }
}
