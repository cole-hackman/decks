//! Watch folders — music files that have arrived but are not in the library.
//!
//! Lexicon describes a folder "under continuous observation". This is
//! implemented as a **debounced scan** rather than a native filesystem watcher:
//! the arrival set is a pure function of (files on disk, library, dismissed),
//! so it is testable without a running event loop, it cannot miss an event that
//! happened while the app was closed, and it needs no platform-specific
//! dependency. A push-based watcher is an optimisation that can be added behind
//! the same function later; it would not change what the user sees.
//!
//! See `docs/lexicon/06-files.md §Watch Folder`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::unused::{is_skipped_directory, KnownPaths};

/// Extensions treated as music.
///
/// Matches what `crates/audio-tags` can actually read — offering to import a
/// file we cannot read the tags of just produces a nameless library entry.
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "wav", "aiff", "aif", "aac", "ogg"];

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.as_str()))
        .unwrap_or(false)
}

/// A file sitting in a watch folder that the library does not have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrival {
    pub path: String,
    pub size_bytes: u64,
    /// Seconds since the file was last modified, as of the scan. The UI uses
    /// this to hold back files still being copied.
    pub age_secs: u64,
}

/// How long a file must have been still before it is offered for import.
///
/// A large FLAC copied over a network share exists on disk long before it is
/// complete; importing it mid-copy reads truncated tags and, worse, records a
/// wrong duration. Ten seconds of quiet is cheap insurance.
pub const SETTLE_SECS: u64 = 10;

/// Whether an arrival has been still long enough to touch.
pub fn has_settled(arrival: &Arrival) -> bool {
    arrival.age_secs >= SETTLE_SECS
}

/// Decide whether one visited file is an arrival.
///
/// Pure, so every rule is testable without a filesystem: it must be audio, the
/// library must not already have it, and the user must not have dismissed it.
pub fn is_arrival(path: &Path, known: &KnownPaths, dismissed: &KnownPaths) -> bool {
    is_audio_file(path) && !known.contains(path) && !dismissed.contains(path)
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WatchScan {
    /// Arrivals that have settled and can be acted on.
    pub arrivals: Vec<Arrival>,
    /// Arrivals still being written. Reported separately so the UI can say
    /// "3 files still copying" rather than silently omitting them.
    pub pending: Vec<Arrival>,
    pub errors: Vec<String>,
}

/// Scan watch folders for files the library does not have.
///
/// `now_secs` is passed in rather than read from the clock so the settle rule
/// is testable.
pub fn scan_watch_folders(
    roots: &[PathBuf],
    known: &KnownPaths,
    dismissed: &KnownPaths,
    now_secs: u64,
) -> WatchScan {
    let mut out = WatchScan::default();

    for root in roots {
        let walker = walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
            if !e.file_type().is_dir() || e.depth() == 0 {
                return true;
            }
            !is_skipped_directory(&e.file_name().to_string_lossy())
        });

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    out.errors.push(e.to_string());
                    continue;
                }
            };
            if entry.file_type().is_dir() {
                continue;
            }
            let path = entry.path();
            if !is_arrival(path, known, dismissed) {
                continue;
            }

            let meta = entry.metadata().ok();
            let size_bytes = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // Saturating, because a file with a modification time in the
            // future (clock skew, or a copy that preserved a later mtime)
            // should read as "not settled", not as an enormous age.
            let age_secs = now_secs.saturating_sub(modified);

            let arrival = Arrival {
                path: path.to_string_lossy().into_owned(),
                size_bytes,
                age_secs,
            };
            if has_settled(&arrival) {
                out.arrivals.push(arrival);
            } else {
                out.pending.push(arrival);
            }
        }
    }

    // Oldest first: the natural triage order is the order they landed.
    out.arrivals.sort_by_key(|a| std::cmp::Reverse(a.age_secs));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(paths: &[&str]) -> KnownPaths {
        KnownPaths::new(paths.iter().map(Path::new))
    }

    #[test]
    fn only_audio_extensions_count() {
        assert!(is_audio_file(Path::new("/w/a.mp3")));
        assert!(is_audio_file(Path::new("/w/a.FLAC")));
        assert!(!is_audio_file(Path::new("/w/cover.jpg")));
        assert!(!is_audio_file(Path::new("/w/README")));
    }

    #[test]
    fn a_file_already_in_the_library_is_not_an_arrival() {
        let lib = known(&["/w/a.mp3"]);
        let none = KnownPaths::default();
        assert!(!is_arrival(Path::new("/w/a.mp3"), &lib, &none));
        assert!(is_arrival(Path::new("/w/b.mp3"), &lib, &none));
    }

    #[test]
    fn a_dismissed_file_stops_being_offered() {
        let none = KnownPaths::default();
        let dismissed = known(&["/w/b.mp3"]);
        assert!(!is_arrival(Path::new("/w/b.mp3"), &none, &dismissed));
    }

    #[test]
    fn library_matching_survives_case_and_separator_differences() {
        let lib = known(&["/W/Sub/A.mp3"]);
        assert!(!is_arrival(
            Path::new("/w/sub/a.mp3"),
            &lib,
            &KnownPaths::default()
        ));
    }

    #[test]
    fn a_file_still_being_written_has_not_settled() {
        let fresh = Arrival {
            path: "/w/a.mp3".into(),
            size_bytes: 1,
            age_secs: 1,
        };
        let old = Arrival {
            age_secs: SETTLE_SECS,
            ..fresh.clone()
        };
        assert!(!has_settled(&fresh));
        assert!(has_settled(&old));
    }

    fn tree(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("decks-watch-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("PioneerDJ")).unwrap();
        std::fs::write(dir.join("new.mp3"), b"aaaa").unwrap();
        std::fs::write(dir.join("cover.jpg"), b"bb").unwrap();
        std::fs::write(dir.join("PioneerDJ/analysis.mp3"), b"cc").unwrap();
        dir
    }

    /// A modification time far enough in the past that the settle rule passes.
    fn long_ago_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + SETTLE_SECS
            + 1
    }

    #[test]
    fn scanning_finds_new_audio_and_ignores_everything_else() {
        let dir = tree("scan");
        let got = scan_watch_folders(
            std::slice::from_ref(&dir),
            &KnownPaths::default(),
            &KnownPaths::default(),
            long_ago_now(),
        );
        let paths: Vec<_> = got.arrivals.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths.len(), 1, "unexpected arrivals: {paths:?}");
        assert!(paths[0].ends_with("new.mp3"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_freshly_written_file_is_pending_rather_than_offered() {
        let dir = tree("pending");
        // "now" is the file's own mtime, so its age is 0.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let got = scan_watch_folders(
            std::slice::from_ref(&dir),
            &KnownPaths::default(),
            &KnownPaths::default(),
            now,
        );
        assert!(got.arrivals.is_empty());
        assert_eq!(got.pending.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_clock_skewed_future_mtime_reads_as_not_settled_rather_than_ancient() {
        let dir = tree("skew");
        // "now" before the file's mtime — saturating_sub must not wrap.
        let got = scan_watch_folders(
            std::slice::from_ref(&dir),
            &KnownPaths::default(),
            &KnownPaths::default(),
            0,
        );
        assert!(got.arrivals.is_empty());
        assert_eq!(got.pending.len(), 1);
        assert_eq!(got.pending[0].age_secs, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_scan_with_no_roots_finds_nothing_and_does_not_error() {
        let got = scan_watch_folders(
            &[],
            &KnownPaths::default(),
            &KnownPaths::default(),
            long_ago_now(),
        );
        assert!(got.arrivals.is_empty());
        assert!(got.errors.is_empty());
    }
}
