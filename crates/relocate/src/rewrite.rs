//! Prefix rewriting — the advanced relocate path.
//!
//! The fuzzy matcher in [`crate`] answers "where did this one file go?". This
//! answers a different question: "the drive letter changed, rewrite all four
//! thousand of them." Nothing is guessed. The user states a source prefix and a
//! target prefix, and every path that starts with the source is rewritten.
//!
//! Deliberately **not automatic**, per the spec: it is built for known,
//! deterministic changes, and a tool that inferred the rewrite would eventually
//! infer the wrong one over an entire library.
//!
//! Pure — nothing here touches the filesystem or a database. See
//! `docs/lexicon/07-health.md §Find Lost Tracks / Relocate`.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// What to rewrite and how.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RewriteSpec {
    pub from_prefix: String,
    pub to_prefix: String,
    /// Substitute the file extension too — the WAV→MP3 re-encode case, where
    /// the originals are gone and only the converted files remain.
    #[serde(default)]
    pub new_extension: Option<String>,
    /// Rewrite every track, not only the ones whose file is missing. Off by
    /// default: the common case is repairing breakage, and sweeping working
    /// paths into a rewrite is how a working library stops working.
    #[serde(default)]
    pub all_tracks: bool,
}

/// A track as the rewriter needs to see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteInput {
    pub track_id: String,
    pub path: String,
    /// Whether the file is currently missing.
    pub missing: bool,
}

/// Why a track was not rewritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum SkipReason {
    /// The path does not start with the source prefix.
    NoMatch,
    /// The file is not missing and `all_tracks` is off.
    NotMissing,
    /// The rewrite would not change the path.
    Unchanged,
    /// Another track already occupies the new path.
    ///
    /// The spec's constraint: you may only relocate to a path not already in
    /// the library. Two rows pointing at one file is a duplicate the user did
    /// not ask for and cannot see.
    Taken(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rewrite {
    pub track_id: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewritePlan {
    pub rewrites: Vec<Rewrite>,
    /// `(track_id, path, reason)` — every track that was considered and passed
    /// over, so "12 of 4000 rewritten" has an explanation attached.
    pub skipped: Vec<(String, String, SkipReason)>,
}

/// Compare path prefixes the way a user means them.
///
/// Separators interchangeable and case ignored: a user typing `D:\Music` should
/// match a stored `D:/music/`, because they are describing the same folder and
/// the difference is an artefact of which program wrote the path.
fn normalise(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Swap a file extension, keeping everything else.
fn with_extension(path: &str, extension: &str) -> String {
    let ext = extension.trim().trim_start_matches('.');
    // Only the last dot *after* the last separator: `/a.b/c` has no extension.
    let cut = path.rfind('/').map(|i| i + 1).unwrap_or(0);
    match path[cut..].rfind('.') {
        Some(dot) => format!("{}.{ext}", &path[..cut + dot]),
        None => format!("{path}.{ext}"),
    }
}

/// Plan a prefix rewrite.
///
/// `existing` is every path already in the library, so the "not already taken"
/// constraint can be checked. Paths produced by this same plan count too — two
/// tracks rewriting onto one path is the same collision.
pub fn plan(spec: &RewriteSpec, tracks: &[RewriteInput], existing: &[String]) -> RewritePlan {
    let mut out = RewritePlan::default();
    if spec.from_prefix.trim().is_empty() {
        return out;
    }
    let from = normalise(&spec.from_prefix);
    let mut taken: HashSet<String> = existing.iter().map(|p| normalise(p)).collect();

    for track in tracks {
        if !track.missing && !spec.all_tracks {
            out.skipped.push((
                track.track_id.clone(),
                track.path.clone(),
                SkipReason::NotMissing,
            ));
            continue;
        }
        let normalised = normalise(&track.path);
        if !normalised.starts_with(&from) {
            out.skipped.push((
                track.track_id.clone(),
                track.path.clone(),
                SkipReason::NoMatch,
            ));
            continue;
        }

        // The remainder keeps its original case — only the prefix is replaced,
        // and a rewrite that lower-cased the rest of the path would break every
        // file on a case-sensitive filesystem.
        let remainder = &track.path[from.len().min(track.path.len())..];
        let joined = format!("{}{remainder}", spec.to_prefix);
        let next = match spec
            .new_extension
            .as_deref()
            .filter(|e| !e.trim().is_empty())
        {
            Some(ext) => with_extension(&joined, ext),
            None => joined,
        };

        if normalise(&next) == normalised {
            out.skipped.push((
                track.track_id.clone(),
                track.path.clone(),
                SkipReason::Unchanged,
            ));
            continue;
        }
        let key = normalise(&next);
        // The track's own current path is not a collision with itself.
        if key != normalised && taken.contains(&key) {
            out.skipped.push((
                track.track_id.clone(),
                track.path.clone(),
                SkipReason::Taken(next),
            ));
            continue;
        }
        taken.insert(key);
        out.rewrites.push(Rewrite {
            track_id: track.track_id.clone(),
            from: track.path.clone(),
            to: next,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, path: &str, missing: bool) -> RewriteInput {
        RewriteInput {
            track_id: id.into(),
            path: path.into(),
            missing,
        }
    }

    fn spec(from: &str, to: &str) -> RewriteSpec {
        RewriteSpec {
            from_prefix: from.into(),
            to_prefix: to.into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_drive_letter_change_rewrites_every_missing_track() {
        let tracks = [
            track("1", "D:/Music/a.mp3", true),
            track("2", "D:/Music/sets/b.mp3", true),
        ];
        let got = plan(&spec("D:/Music", "/Volumes/Music"), &tracks, &[]);
        assert_eq!(got.rewrites.len(), 2);
        assert_eq!(got.rewrites[0].to, "/Volumes/Music/a.mp3");
        assert_eq!(got.rewrites[1].to, "/Volumes/Music/sets/b.mp3");
    }

    #[test]
    fn separators_and_case_do_not_stop_a_match() {
        // A user typing D:\Music means the folder stored as D:/music/.
        let tracks = [track("1", "D:/music/a.mp3", true)];
        let got = plan(&spec("D:\\Music", "/Volumes/Music"), &tracks, &[]);
        assert_eq!(got.rewrites[0].to, "/Volumes/Music/a.mp3");
    }

    #[test]
    fn the_remainder_keeps_its_original_case() {
        // Lower-casing the rest of the path would break every file on a
        // case-sensitive filesystem.
        let tracks = [track("1", "d:/music/Daft Punk/Get Lucky.mp3", true)];
        let got = plan(&spec("D:/Music", "/Volumes/Music"), &tracks, &[]);
        assert_eq!(got.rewrites[0].to, "/Volumes/Music/Daft Punk/Get Lucky.mp3");
    }

    #[test]
    fn a_path_that_does_not_match_the_prefix_is_left_alone() {
        let tracks = [track("1", "/elsewhere/a.mp3", true)];
        let got = plan(&spec("D:/Music", "/Volumes/Music"), &tracks, &[]);
        assert!(got.rewrites.is_empty());
        assert_eq!(got.skipped[0].2, SkipReason::NoMatch);
    }

    #[test]
    fn tracks_that_are_not_missing_are_left_alone_by_default() {
        // Sweeping working paths into a rewrite is how a working library stops
        // working.
        let tracks = [track("1", "D:/Music/a.mp3", false)];
        let got = plan(&spec("D:/Music", "/Volumes/Music"), &tracks, &[]);
        assert!(got.rewrites.is_empty());
        assert_eq!(got.skipped[0].2, SkipReason::NotMissing);
    }

    #[test]
    fn all_tracks_mode_rewrites_working_paths_too() {
        let tracks = [track("1", "D:/Music/a.mp3", false)];
        let s = RewriteSpec {
            all_tracks: true,
            ..spec("D:/Music", "/Volumes/Music")
        };
        assert_eq!(plan(&s, &tracks, &[]).rewrites.len(), 1);
    }

    #[test]
    fn a_rewrite_that_changes_nothing_is_reported_not_staged() {
        let tracks = [track("1", "/Music/a.mp3", true)];
        let got = plan(&spec("/Music", "/Music"), &tracks, &[]);
        assert!(got.rewrites.is_empty());
        assert_eq!(got.skipped[0].2, SkipReason::Unchanged);
    }

    #[test]
    fn a_path_already_in_the_library_is_refused() {
        // The spec's constraint. Two rows pointing at one file is a duplicate
        // the user did not ask for and cannot see.
        let tracks = [track("1", "D:/Music/a.mp3", true)];
        let existing = vec!["/Volumes/Music/a.mp3".to_string()];
        let got = plan(&spec("D:/Music", "/Volumes/Music"), &tracks, &existing);
        assert!(got.rewrites.is_empty());
        assert_eq!(
            got.skipped[0].2,
            SkipReason::Taken("/Volumes/Music/a.mp3".into())
        );
    }

    #[test]
    fn two_tracks_cannot_rewrite_onto_the_same_path() {
        // The collision does not have to pre-exist to be a collision.
        let tracks = [
            track("1", "D:/A/x.mp3", true),
            track("2", "D:/B/x.mp3", true),
        ];
        let mut got = plan(&spec("D:/A", "/M"), &tracks, &[]);
        got = plan(
            &spec("D:/B", "/M"),
            &tracks[1..],
            &[got.rewrites[0].to.clone()],
        );
        assert!(got.rewrites.is_empty());
    }

    #[test]
    fn an_extension_substitution_swaps_only_the_extension() {
        // The WAV→MP3 re-encode case.
        let s = RewriteSpec {
            new_extension: Some("mp3".into()),
            ..spec("/Music", "/Music")
        };
        let tracks = [track("1", "/Music/Daft.Punk/Get Lucky.wav", true)];
        assert_eq!(
            plan(&s, &tracks, &[]).rewrites[0].to,
            "/Music/Daft.Punk/Get Lucky.mp3"
        );
    }

    #[test]
    fn a_leading_dot_on_the_new_extension_is_tolerated() {
        let s = RewriteSpec {
            new_extension: Some(".mp3".into()),
            ..spec("/Music", "/Music")
        };
        let tracks = [track("1", "/Music/a.wav", true)];
        assert_eq!(plan(&s, &tracks, &[]).rewrites[0].to, "/Music/a.mp3");
    }

    #[test]
    fn a_file_with_no_extension_gains_one() {
        let s = RewriteSpec {
            new_extension: Some("mp3".into()),
            ..spec("/Music", "/Music")
        };
        let tracks = [track("1", "/Music/nameless", true)];
        assert_eq!(plan(&s, &tracks, &[]).rewrites[0].to, "/Music/nameless.mp3");
    }

    #[test]
    fn a_dot_in_a_folder_name_is_not_an_extension() {
        let s = RewriteSpec {
            new_extension: Some("mp3".into()),
            ..spec("/Music", "/Music")
        };
        let tracks = [track("1", "/Music/v1.0/track", true)];
        assert_eq!(
            plan(&s, &tracks, &[]).rewrites[0].to,
            "/Music/v1.0/track.mp3"
        );
    }

    #[test]
    fn an_empty_source_prefix_rewrites_nothing() {
        // It would match every path in the library — never what anyone meant.
        let tracks = [track("1", "/Music/a.mp3", true)];
        assert!(plan(&spec("", "/x"), &tracks, &[]).rewrites.is_empty());
    }

    #[test]
    fn every_track_is_accounted_for() {
        let tracks = [
            track("1", "D:/Music/a.mp3", true),
            track("2", "/elsewhere/b.mp3", true),
            track("3", "D:/Music/c.mp3", false),
        ];
        let got = plan(&spec("D:/Music", "/M"), &tracks, &[]);
        assert_eq!(got.rewrites.len() + got.skipped.len(), 3);
    }
}
