//! Local Path Mappings — per-computer prefix rewriting.
//!
//! A database restored on a second machine points at paths that machine does
//! not have: `D:\Music\…` on the desktop, `/Users/me/Music/…` on the laptop.
//! A mapping rewrites the prefix so the files are found without a bulk
//! relocate and without editing the library.
//!
//! Mappings are **per-computer**, so they live in the local cache and are never
//! staged, exported, or synced. Rewriting is read-side only: the library keeps
//! saying `D:\Music\…`, and that is the point — the same database works on both
//! machines at once.
//!
//! See `docs/lexicon/06-files.md §Local Path Mappings`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One prefix rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathMapping {
    /// The prefix as stored in the library.
    pub from: String,
    /// The prefix on this machine.
    pub to: String,
}

/// Split a path into comparable components.
///
/// Both separators are treated as separators regardless of platform: a database
/// written on Windows is read on macOS and vice versa, which is the entire
/// situation this feature exists for.
fn parts(path: &str) -> Vec<String> {
    path.split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// An ordered set of mappings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathMappings {
    pub mappings: Vec<PathMapping>,
}

impl PathMappings {
    pub fn new(mappings: Vec<PathMapping>) -> Self {
        PathMappings { mappings }
    }

    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Rewrite a stored path for this machine.
    ///
    /// The **longest matching prefix wins**, so `/Music/Live` can be mapped to a
    /// different disk than `/Music` without ordering mattering. Matching is on
    /// whole path components — `/Music` must not match `/MusicVideos` — and is
    /// case-insensitive, because macOS and Windows filesystems are and a
    /// mapping that silently fails on case is worse than no mapping.
    ///
    /// A path that matches nothing comes back unchanged.
    pub fn resolve(&self, stored: &str) -> PathBuf {
        let stored_parts = parts(stored);

        let best = self
            .mappings
            .iter()
            .filter_map(|m| {
                let from = parts(&m.from);
                if from.is_empty() || from.len() > stored_parts.len() {
                    return None;
                }
                if stored_parts[..from.len()] != from[..] {
                    return None;
                }
                Some((from.len(), m))
            })
            .max_by_key(|(len, _)| *len);

        let Some((matched, mapping)) = best else {
            return PathBuf::from(stored);
        };

        // Rebuild from the *original* remainder, not the lower-cased one — the
        // filesystem may well be case-sensitive even when the comparison is not.
        let remainder: Vec<&str> = stored
            .split(['/', '\\'])
            .filter(|s| !s.is_empty())
            .skip(matched)
            .collect();

        let mut out = PathBuf::from(&mapping.to);
        for part in remainder {
            out.push(part);
        }
        out
    }

    /// Resolve, then report whether the file is actually there.
    ///
    /// Convenience for the callers whose next move is an existence check.
    pub fn resolve_existing(&self, stored: &str) -> Option<PathBuf> {
        let resolved = self.resolve(stored);
        Path::new(&resolved).exists().then_some(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(from: &str, to: &str) -> PathMapping {
        PathMapping {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn rewrites_a_matching_prefix() {
        let m = PathMappings::new(vec![map("D:\\Music", "/Users/me/Music")]);
        assert_eq!(
            m.resolve("D:\\Music\\House\\track.mp3"),
            PathBuf::from("/Users/me/Music/House/track.mp3")
        );
    }

    #[test]
    fn separators_are_interchangeable_because_the_databases_cross_platforms() {
        let m = PathMappings::new(vec![map("D:/Music", "/Users/me/Music")]);
        assert_eq!(
            m.resolve("D:\\Music\\track.mp3"),
            PathBuf::from("/Users/me/Music/track.mp3")
        );
    }

    #[test]
    fn matching_is_case_insensitive() {
        let m = PathMappings::new(vec![map("/music", "/Volumes/Audio")]);
        assert_eq!(
            m.resolve("/Music/track.mp3"),
            PathBuf::from("/Volumes/Audio/track.mp3")
        );
    }

    #[test]
    fn the_remainder_keeps_its_original_case() {
        // The comparison is case-insensitive; the filesystem may not be.
        let m = PathMappings::new(vec![map("/music", "/Volumes/Audio")]);
        assert_eq!(
            m.resolve("/Music/DeepHouse/Track.MP3"),
            PathBuf::from("/Volumes/Audio/DeepHouse/Track.MP3")
        );
    }

    #[test]
    fn a_prefix_must_match_whole_components() {
        // "/Music" must not swallow "/MusicVideos".
        let m = PathMappings::new(vec![map("/Music", "/Volumes/Audio")]);
        assert_eq!(
            m.resolve("/MusicVideos/clip.mp4"),
            PathBuf::from("/MusicVideos/clip.mp4")
        );
    }

    #[test]
    fn the_longest_matching_prefix_wins_regardless_of_order() {
        let general = map("/Music", "/Volumes/Audio");
        let specific = map("/Music/Live", "/Volumes/Live");

        for mappings in [
            vec![general.clone(), specific.clone()],
            vec![specific, general],
        ] {
            let m = PathMappings::new(mappings);
            assert_eq!(
                m.resolve("/Music/Live/set.wav"),
                PathBuf::from("/Volumes/Live/set.wav")
            );
            assert_eq!(
                m.resolve("/Music/House/a.mp3"),
                PathBuf::from("/Volumes/Audio/House/a.mp3")
            );
        }
    }

    #[test]
    fn an_unmatched_path_comes_back_unchanged() {
        let m = PathMappings::new(vec![map("/Music", "/Volumes/Audio")]);
        assert_eq!(
            m.resolve("/Downloads/a.mp3"),
            PathBuf::from("/Downloads/a.mp3")
        );
    }

    #[test]
    fn no_mappings_is_the_identity() {
        let m = PathMappings::default();
        assert!(m.is_empty());
        assert_eq!(m.resolve("/Music/a.mp3"), PathBuf::from("/Music/a.mp3"));
    }

    #[test]
    fn an_empty_from_prefix_never_matches() {
        // Otherwise it would match every path and rewrite the whole library.
        let m = PathMappings::new(vec![map("", "/Volumes/Audio")]);
        assert_eq!(m.resolve("/Music/a.mp3"), PathBuf::from("/Music/a.mp3"));
    }

    #[test]
    fn a_prefix_longer_than_the_path_does_not_match() {
        let m = PathMappings::new(vec![map("/Music/House/Deep", "/x")]);
        assert_eq!(m.resolve("/Music/House"), PathBuf::from("/Music/House"));
    }

    #[test]
    fn mapping_the_whole_path_yields_the_target_itself() {
        let m = PathMappings::new(vec![map("/Music/a.mp3", "/Volumes/b.mp3")]);
        assert_eq!(m.resolve("/Music/a.mp3"), PathBuf::from("/Volumes/b.mp3"));
    }

    #[test]
    fn mappings_round_trip_through_json() {
        let m = PathMappings::new(vec![map("D:\\Music", "/Users/me/Music")]);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<PathMappings>(&json).unwrap(), m);
    }
}
