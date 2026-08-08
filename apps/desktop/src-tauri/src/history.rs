//! Play history (Epic 6) — the gig log.
//!
//! Per `docs/lexicon/09-history-backup.md §History`. Sessions import from
//! Rekordbox's `djmdHistory` into **our own snapshot tables**, because the
//! spec's central design decision is that history is a historical record, not
//! a view over current data: editing a track later must not rewrite what the
//! log says was played.
//!
//! Nothing here writes to `master.db`. Saving a set as a playlist stages
//! changes like everything else.

use std::path::Path;

use cache::store::{
    HistoryImportReport, HistorySetRow, HistoryTrackRow, IncomingHistorySet, IncomingHistoryTrack,
};
use decks_core::rekordbox_db::RekordboxDb;
use serde::{Deserialize, Serialize};

use crate::cache_db;

fn open_db(path: &str) -> Result<RekordboxDb, String> {
    RekordboxDb::open(Path::new(path)).map_err(|e| e.to_string())
}

/// Import every session Rekordbox has logged.
///
/// Idempotent by `djmdHistory.ID`: sets already known are counted, not
/// duplicated, and sets the user deleted stay deleted.
#[tauri::command]
pub async fn import_history(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<HistoryImportReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&library_path)?;
        let sets = db.history_sets().map_err(|e| e.to_string())?;

        let incoming: Vec<IncomingHistorySet> = sets
            .into_iter()
            .map(|set| {
                let tracks = db
                    .history_entries(&set.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| IncomingHistoryTrack {
                        content_id: Some(e.content_id),
                        title: e.title,
                        artist: e.artist,
                        album: e.album,
                        genre: e.genre,
                        musical_key: e.musical_key,
                        bpm: e.bpm,
                        duration_secs: e.duration_secs,
                        folder_path: e.folder_path,
                    })
                    .collect();
                IncomingHistorySet {
                    source_id: set.id,
                    name: set.name,
                    played_at: set.played_at,
                    tracks,
                }
            })
            .collect();

        let mut cache = cache_db(&app)?;
        cache
            .import_history(&library_path, &incoming)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_history_sets(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<HistorySetRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .list_history_sets(&library_path)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn history_set_tracks(
    app: tauri::AppHandle,
    set_id: String,
) -> Result<Vec<HistoryTrackRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .history_set_tracks(&set_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_history_metadata(
    app: tauri::AppHandle,
    set_id: String,
    rating: Option<i64>,
    location: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .set_history_metadata(&set_id, rating, location.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_history_set(
    app: tauri::AppHandle,
    library_path: String,
    set_id: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .delete_history_set(&library_path, &set_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_history_track(app: tauri::AppHandle, track_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .remove_history_track(&track_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Save a set as a playlist ─────────────────────────────────────────────────

/// How a snapshot row was matched back to a live track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// The library id from play time still resolves. Strongest signal there is.
    ContentId,
    /// Same file, same place.
    Path,
    /// Same filename somewhere else — the file moved. The spec's fallback, and
    /// the one that depends on filenames not having changed.
    Filename,
    /// Nothing found. The track is not in the library any more.
    None,
}

#[derive(Debug, Serialize)]
pub struct HistoryMatch {
    pub history_track_id: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    /// `None` when `kind` is `none`.
    pub track_id: Option<String>,
    pub kind: MatchKind,
}

#[derive(Debug, Serialize)]
pub struct HistoryMatchReport {
    pub matches: Vec<HistoryMatch>,
    pub matched: usize,
    pub unmatched: usize,
}

/// The filename, lower-cased, from a path with either separator.
fn file_name_key(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then(|| name.to_lowercase())
}

fn path_key(path: &str) -> String {
    path.trim().replace('\\', "/").to_lowercase()
}

/// Re-match snapshot rows against the live library, strongest signal first.
///
/// The spec calls for "a priority system" and notes it depends on filenames not
/// having changed. Ours is id → path → filename, and it reports **which** rule
/// matched, because "we found something with the same filename" is a materially
/// weaker claim than "this is the same track" and the user deserves to see the
/// difference before staging a playlist (ADR-0008).
///
/// A filename that matches more than one library track is treated as no match:
/// picking one arbitrarily would silently put the wrong track in the set.
pub fn match_history_tracks(
    snapshot: &[HistoryTrackRow],
    library: &[decks_core::rekordbox_db::Track],
) -> HistoryMatchReport {
    use std::collections::HashMap;

    let by_id: HashMap<&str, &str> = library
        .iter()
        .map(|t| (t.id.as_str(), t.id.as_str()))
        .collect();
    let mut by_path: HashMap<String, &str> = HashMap::new();
    let mut by_name: HashMap<String, Vec<&str>> = HashMap::new();
    for track in library {
        let Some(path) = track.folder_path.as_deref() else {
            continue;
        };
        by_path.entry(path_key(path)).or_insert(track.id.as_str());
        if let Some(name) = file_name_key(path) {
            by_name.entry(name).or_default().push(track.id.as_str());
        }
    }

    let matches: Vec<HistoryMatch> = snapshot
        .iter()
        .map(|row| {
            let (track_id, kind) = row
                .content_id
                .as_deref()
                .and_then(|id| {
                    by_id
                        .get(id)
                        .map(|t| ((*t).to_string(), MatchKind::ContentId))
                })
                .or_else(|| {
                    row.folder_path
                        .as_deref()
                        .and_then(|p| by_path.get(&path_key(p)))
                        .map(|t| ((*t).to_string(), MatchKind::Path))
                })
                .or_else(|| {
                    let name = file_name_key(row.folder_path.as_deref()?)?;
                    // Ambiguous filename → no match. Picking one arbitrarily
                    // would put the wrong track in the set.
                    match by_name.get(&name)?.as_slice() {
                        [only] => Some(((*only).to_string(), MatchKind::Filename)),
                        _ => None,
                    }
                })
                .map_or((None, MatchKind::None), |(id, kind)| (Some(id), kind));

            HistoryMatch {
                history_track_id: row.id.clone(),
                title: row.title.clone(),
                artist: row.artist.clone(),
                track_id,
                kind,
            }
        })
        .collect();

    let matched = matches.iter().filter(|m| m.kind != MatchKind::None).count();
    HistoryMatchReport {
        unmatched: matches.len() - matched,
        matched,
        matches,
    }
}

/// Preview turning a set into a playlist: what would be found, and how.
#[tauri::command]
pub async fn preview_history_as_playlist(
    app: tauri::AppHandle,
    library_path: String,
    set_id: String,
) -> Result<HistoryMatchReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = cache_db(&app)?
            .history_set_tracks(&set_id)
            .map_err(|e| e.to_string())?;
        let library = open_db(&library_path)?
            .tracks()
            .map_err(|e| e.to_string())?;
        Ok(match_history_tracks(&snapshot, &library))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the playlist. Unmatched rows are simply absent — the preview said so.
#[tauri::command]
pub async fn save_history_as_playlist(
    app: tauri::AppHandle,
    library_path: String,
    name: String,
    track_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("the playlist needs a name".into());
    }
    if track_ids.is_empty() {
        return Err("nothing matched, so there is no playlist to make".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let playlist_id = uuid::Uuid::new_v4().to_string();
        let mut staged = Vec::new();

        let stage = |kind, target, value: serde_json::Value| -> Result<String, String> {
            cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind,
                    target_id: Some(target),
                    field: None,
                    old_value: None,
                    new_value: Some(value),
                    reason: Some("Saved from play history".to_string()),
                    confidence: Some(1.0),
                })
                .map(|r| r.id)
                .map_err(|e| e.to_string())
        };

        staged.push(stage(
            changes::ChangeKind::PlaylistCreate,
            playlist_id.clone(),
            serde_json::json!({ "name": name, "parent_id": null, "attribute": 0 }),
        )?);
        for track_id in &track_ids {
            staged.push(stage(
                changes::ChangeKind::PlaylistAddTrack,
                playlist_id.clone(),
                serde_json::json!(track_id),
            )?);
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use decks_core::rekordbox_db::Track;

    fn snapshot(id: &str, content_id: Option<&str>, path: Option<&str>) -> HistoryTrackRow {
        HistoryTrackRow {
            id: id.into(),
            seq: 1,
            content_id: content_id.map(str::to_string),
            title: Some(format!("Track {id}")),
            artist: Some("Someone".into()),
            album: None,
            genre: None,
            musical_key: None,
            bpm: None,
            duration_secs: None,
            folder_path: path.map(str::to_string),
        }
    }

    fn track(id: &str, path: Option<&str>) -> Track {
        Track {
            id: id.into(),
            title: format!("Live {id}"),
            artist: None,
            album: None,
            genre: None,
            musical_key: None,
            bpm: None,
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: path.map(str::to_string),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: None,
            dj_play_count: None,
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
            energy: None,
        }
    }

    #[test]
    fn the_library_id_wins_when_it_still_resolves() {
        let report = match_history_tracks(
            &[snapshot("h1", Some("t1"), Some("/moved/elsewhere.mp3"))],
            &[track("t1", Some("/music/a.mp3"))],
        );
        assert_eq!(report.matches[0].kind, MatchKind::ContentId);
        assert_eq!(report.matches[0].track_id.as_deref(), Some("t1"));
        assert_eq!(report.matched, 1);
    }

    #[test]
    fn the_path_matches_when_the_id_is_gone() {
        let report = match_history_tracks(
            &[snapshot("h1", Some("stale"), Some("/music/a.mp3"))],
            &[track("t1", Some("/music/a.mp3"))],
        );
        assert_eq!(report.matches[0].kind, MatchKind::Path);
        assert_eq!(report.matches[0].track_id.as_deref(), Some("t1"));
    }

    #[test]
    fn path_matching_ignores_separators_and_case() {
        let report = match_history_tracks(
            &[snapshot("h1", None, Some("D:\\Music\\A.mp3"))],
            &[track("t1", Some("d:/music/a.mp3"))],
        );
        assert_eq!(report.matches[0].kind, MatchKind::Path);
    }

    #[test]
    fn the_filename_is_the_last_resort_and_is_labelled_as_such() {
        // "Something with the same filename" is a materially weaker claim than
        // "this is the same track", and the report says which.
        let report = match_history_tracks(
            &[snapshot("h1", None, Some("/old/place/a.mp3"))],
            &[track("t1", Some("/new/place/a.mp3"))],
        );
        assert_eq!(report.matches[0].kind, MatchKind::Filename);
        assert_eq!(report.matches[0].track_id.as_deref(), Some("t1"));
    }

    #[test]
    fn an_ambiguous_filename_is_no_match_rather_than_a_guess() {
        // Two tracks called a.mp3; picking one would silently put the wrong
        // track in the set.
        let report = match_history_tracks(
            &[snapshot("h1", None, Some("/old/a.mp3"))],
            &[
                track("t1", Some("/one/a.mp3")),
                track("t2", Some("/two/a.mp3")),
            ],
        );
        assert_eq!(report.matches[0].kind, MatchKind::None);
        assert_eq!(report.matches[0].track_id, None);
        assert_eq!(report.unmatched, 1);
    }

    #[test]
    fn a_track_that_is_gone_reports_as_unmatched_with_its_snapshot_intact() {
        let report = match_history_tracks(
            &[snapshot("h1", Some("gone"), Some("/deleted/x.mp3"))],
            &[track("t1", Some("/music/a.mp3"))],
        );
        assert_eq!(report.matches[0].kind, MatchKind::None);
        // The gig log still knows what was played, which is the point.
        assert_eq!(report.matches[0].title.as_deref(), Some("Track h1"));
        assert_eq!(report.matched, 0);
        assert_eq!(report.unmatched, 1);
    }

    #[test]
    fn a_snapshot_row_with_no_path_at_all_is_unmatched() {
        let report = match_history_tracks(&[snapshot("h1", None, None)], &[track("t1", None)]);
        assert_eq!(report.matches[0].kind, MatchKind::None);
    }

    #[test]
    fn every_row_comes_back_in_order() {
        // The report is what the UI lists, so it must not silently drop rows.
        let report = match_history_tracks(
            &[
                snapshot("h1", Some("t1"), None),
                snapshot("h2", Some("gone"), None),
                snapshot("h3", Some("t2"), None),
            ],
            &[track("t1", None), track("t2", None)],
        );
        assert_eq!(
            report
                .matches
                .iter()
                .map(|m| m.history_track_id.as_str())
                .collect::<Vec<_>>(),
            vec!["h1", "h2", "h3"]
        );
        assert_eq!(report.matched, 2);
        assert_eq!(report.unmatched, 1);
    }

    #[test]
    fn match_kinds_serialise_as_the_names_the_ui_reads() {
        for (kind, json) in [
            (MatchKind::ContentId, "\"content_id\""),
            (MatchKind::Path, "\"path\""),
            (MatchKind::Filename, "\"filename\""),
            (MatchKind::None, "\"none\""),
        ] {
            assert_eq!(serde_json::to_string(&kind).unwrap(), json);
        }
    }
}
