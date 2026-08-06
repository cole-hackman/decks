//! Tauri commands for Find Broken Tracks (Epic 5).
//!
//! The existing broken-link scan asks whether a path exists. This asks whether
//! the file actually decodes — a truncated download, a half-copied file, a
//! `.mp3` that is really an HTML error page are all present and all unplayable.
//!
//! **Nothing here deletes anything.** The scan reports; removing a track from
//! the library is `stage_track_delete`, which the user drives and Sync applies
//! under the write guard. Deleting from disk is not offered at all — see the
//! divergence note in `docs/lexicon/07-health.md`.
//!
//! Per `docs/lexicon/07-health.md §Find Broken Tracks`.

use std::path::Path;

use audio_analysis::playable::{verify_playable, CheckDepth, PlaybackStatus};

use crate::organizer::path_mappings;

/// One track that did not pass.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrokenTrack {
    pub track_id: String,
    pub title: String,
    pub artist: Option<String>,
    /// The path as the library holds it, before mapping.
    pub path: String,
    pub status: PlaybackStatus,
    /// Playlists that hold this track.
    ///
    /// The spec's report exists specifically so a user can source
    /// replacements, and "which set was this in" is the question that makes
    /// that possible.
    pub playlists: Vec<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct BrokenScan {
    pub broken: Vec<BrokenTrack>,
    /// Tracks checked, so a clean result reads as "checked 4,000" rather than
    /// as a scan that silently did nothing.
    pub checked: usize,
    /// Tracks with no file path at all — nothing to check, and not a decode
    /// failure. Counted separately rather than reported as broken.
    pub no_path: usize,
}

/// Scan a library for files that do not play.
///
/// `track_ids` empty means the whole library. A `Full` depth decodes every
/// file, which is the only way to catch truncation and costs about what
/// analysing the track costs — the UI says so before running it.
#[tauri::command]
pub async fn scan_broken_tracks(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    depth: CheckDepth,
) -> Result<BrokenScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        // Resolve through path mappings first, so a library restored on a
        // second machine is not reported as 4,000 missing files.
        let mappings = path_mappings(&app);

        let mut playlists_by_track: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for playlist in db.playlists().map_err(|e| e.to_string())? {
            if !matches!(
                playlist.kind,
                decks_core::rekordbox_db::PlaylistKind::Playlist
            ) {
                continue;
            }
            for entry in db
                .playlist_entries(&playlist.id)
                .map_err(|e| e.to_string())?
            {
                playlists_by_track
                    .entry(entry.content_id)
                    .or_default()
                    .push(playlist.name.clone());
            }
        }

        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let wanted: std::collections::HashSet<&String> = track_ids.iter().collect();

        let mut out = BrokenScan::default();
        for track in &tracks {
            if !wanted.is_empty() && !wanted.contains(&track.id) {
                continue;
            }
            let Some(path) = track.folder_path.as_deref() else {
                out.no_path += 1;
                continue;
            };
            out.checked += 1;

            let resolved = mappings.resolve(path);
            let status = verify_playable(&resolved, depth);
            if !status.is_broken() {
                continue;
            }
            out.broken.push(BrokenTrack {
                track_id: track.id.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                path: path.to_string(),
                status,
                playlists: playlists_by_track
                    .get(&track.id)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The scan as a plain-text report.
///
/// The spec writes one to `Documents/Lexicon`; `decks` returns the text and
/// lets the user save it where they like rather than writing to a directory
/// they did not choose. Each line names the playlists the track was in, which
/// is the whole point of the report — it is what makes sourcing replacements
/// possible.
#[tauri::command]
pub fn broken_tracks_report(scan_broken: Vec<BrokenTrack>) -> String {
    let mut out = String::from("Broken tracks\n\n");
    for t in &scan_broken {
        let artist = t.artist.as_deref().unwrap_or("Unknown artist");
        out.push_str(&format!("{} — {}\n", artist, t.title));
        out.push_str(&format!("  {}\n", t.path));
        out.push_str(&format!("  {}\n", describe(&t.status)));
        if t.playlists.is_empty() {
            out.push_str("  in no playlists\n");
        } else {
            out.push_str(&format!("  in: {}\n", t.playlists.join(", ")));
        }
        out.push('\n');
    }
    out
}

/// Write the report where the user chose.
///
/// The spec writes to `Documents/Lexicon`; `decks` takes a path the user picked
/// in a save dialog instead. Writing into a directory nobody asked for is the
/// sort of thing that makes a tool feel like it is taking liberties, and this
/// file is for the user rather than for us.
#[tauri::command]
pub fn save_broken_tracks_report(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("could not write {path}: {e}"))
}

/// A sentence, not an enum name.
pub(crate) fn describe(status: &PlaybackStatus) -> String {
    match status {
        PlaybackStatus::Ok => "plays".into(),
        PlaybackStatus::Missing => "the file is not there".into(),
        PlaybackStatus::Unreadable(why) => format!("cannot be opened: {why}"),
        PlaybackStatus::Undecodable(why) => format!("does not decode: {why}"),
        PlaybackStatus::Truncated(why) => format!("incomplete: {why}"),
        PlaybackStatus::Damaged { bad_packets } => {
            format!("plays with {bad_packets} damaged section(s)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn broken(status: PlaybackStatus, playlists: &[&str]) -> BrokenTrack {
        BrokenTrack {
            track_id: "t1".into(),
            title: "Get Lucky".into(),
            artist: Some("Daft Punk".into()),
            path: "/music/a.mp3".into(),
            status,
            playlists: playlists.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn the_report_names_the_playlists_a_broken_track_was_in() {
        // The whole reason the report exists: sourcing a replacement means
        // knowing which set is now short a track.
        let text = broken_tracks_report(vec![broken(
            PlaybackStatus::Missing,
            &["Techno Set", "Warmup"],
        )]);
        assert!(text.contains("in: Techno Set, Warmup"), "{text}");
    }

    #[test]
    fn a_track_in_no_playlist_says_so_rather_than_leaving_a_blank_line() {
        let text = broken_tracks_report(vec![broken(PlaybackStatus::Missing, &[])]);
        assert!(text.contains("in no playlists"), "{text}");
    }

    #[test]
    fn reasons_read_as_sentences_not_enum_names() {
        assert_eq!(describe(&PlaybackStatus::Missing), "the file is not there");
        assert!(
            describe(&PlaybackStatus::Truncated("40% present".into())).starts_with("incomplete: ")
        );
        assert!(describe(&PlaybackStatus::Damaged { bad_packets: 3 }).contains("3 damaged"));
    }

    #[test]
    fn a_track_with_no_artist_still_gets_a_line() {
        let mut t = broken(PlaybackStatus::Missing, &[]);
        t.artist = None;
        let text = broken_tracks_report(vec![t]);
        assert!(text.contains("Unknown artist — Get Lucky"), "{text}");
    }

    #[test]
    fn an_empty_scan_produces_a_header_and_nothing_else() {
        // Better than an empty file, which reads as the export having failed.
        let text = broken_tracks_report(Vec::new());
        assert_eq!(text.trim(), "Broken tracks");
    }
}
