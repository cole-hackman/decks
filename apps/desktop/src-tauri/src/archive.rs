//! Archive — hiding tracks without deleting them (Epic 5).
//!
//! Two rules from the spec that are easy to get backwards:
//!
//! **The playlist rule.** Archiving *from inside a playlist* removes the track
//! from **that** playlist and leaves it in every other. Archiving *from the
//! main browser* leaves it in all of them. The stated intent is that you can
//! archive freely without breaking your sets — and the asymmetry is the whole
//! point: from a playlist you are saying "not in this set", from the browser
//! you are saying "not in my way".
//!
//! **Cleanup is where tracks finally leave every playlist**, not archiving.
//! Until then an archived track is still in the sets it was in, which is what
//! makes archiving safe to do on a whim.
//!
//! Per `docs/lexicon/02-library.md §Archive`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cache_db;

/// What the selection helper should pick out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ArchiveCriterion {
    /// Archived longer ago than this many days.
    OlderThanDays(u32),
    /// No hot cues or memory cues — never prepared, so never played.
    WithoutCues,
    /// In no playlist at all.
    InNoPlaylist,
}

/// One archived track and the facts the criteria ask about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedFacts {
    pub track_id: String,
    pub archived_at: i64,
    pub has_cues: bool,
    pub in_a_playlist: bool,
}

/// Pick the archived tracks a criterion describes.
///
/// `now` is passed in rather than read so the age criterion is testable
/// without waiting six months.
pub fn select(facts: &[ArchivedFacts], criterion: ArchiveCriterion, now: i64) -> Vec<String> {
    facts
        .iter()
        .filter(|f| match criterion {
            ArchiveCriterion::OlderThanDays(days) => {
                // Strictly older: "older than 0 days" should not sweep up
                // something archived a second ago, which is almost certainly a
                // misclick on the way to choosing a real threshold.
                now - f.archived_at > i64::from(days) * 86_400
            }
            ArchiveCriterion::WithoutCues => !f.has_cues,
            ArchiveCriterion::InNoPlaylist => !f.in_a_playlist,
        })
        .map(|f| f.track_id.clone())
        .collect()
}

#[derive(Debug, Default, Serialize)]
pub struct ArchiveResult {
    /// Tracks now archived.
    pub archived: Vec<String>,
    /// Staged `PlaylistRemoveTrack` ids, when archiving from inside a playlist.
    pub staged: Vec<String>,
}

/// Archive a selection.
///
/// `from_playlist_id` is the playlist the user was looking at, if any. Passing
/// it is what turns "archive" into "and take it out of this set" — and passing
/// `None` from the browser is what leaves every playlist alone.
///
/// The removal is *staged*, not written: it is a change to `master.db` like any
/// other, and goes through review and Sync. Archiving itself is cache-only and
/// takes effect at once, which is why the two halves are reported separately.
#[tauri::command]
pub async fn archive_tracks_from(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    from_playlist_id: Option<String>,
) -> Result<ArchiveResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .archive_tracks(&library_path, &track_ids)
            .map_err(|e| e.to_string())?;

        let mut result = ArchiveResult {
            archived: track_ids.clone(),
            staged: Vec::new(),
        };

        let Some(playlist_id) = from_playlist_id else {
            return Ok(result);
        };
        for id in &track_ids {
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::PlaylistRemoveTrack,
                    target_id: Some(playlist_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(serde_json::json!(id)),
                    reason: Some("Archived from this playlist".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            result.staged.push(record.id);
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The archived tracks a criterion picks out.
#[tauri::command]
pub async fn select_archived(
    app: tauri::AppHandle,
    library_path: String,
    criterion: ArchiveCriterion,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let archived = cache
            .list_archived_with_dates(&library_path)
            .map_err(|e| e.to_string())?;
        if archived.is_empty() {
            return Ok(Vec::new());
        }

        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let with_cues: std::collections::HashSet<String> = db
            .track_ids_with_cues()
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let mut in_playlists = std::collections::HashSet::new();
        for playlist in db.playlists().map_err(|e| e.to_string())? {
            for entry in db
                .playlist_entries(&playlist.id)
                .map_err(|e| e.to_string())?
            {
                in_playlists.insert(entry.content_id);
            }
        }

        let facts: Vec<ArchivedFacts> = archived
            .into_iter()
            .map(|(track_id, archived_at)| ArchivedFacts {
                has_cues: with_cues.contains(&track_id),
                in_a_playlist: in_playlists.contains(&track_id),
                track_id,
                archived_at,
            })
            .collect();

        Ok(select(&facts, criterion, now_secs()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cleanup: stage the removal of archived tracks from the library.
///
/// This is where tracks finally leave every playlist — archiving does not, and
/// that difference is what makes archiving safe to do on a whim.
///
/// Everything is *staged*. The spec also offers deleting the audio from disk;
/// `decks` does not, on the same grounds as Find Broken Tracks: it is the one
/// operation with no undo, and a program whose first rule is that the library
/// is read-only should not be what deletes a DJ's files. See
/// `docs/lexicon/02-library.md`.
#[tauri::command]
pub async fn cleanup_archived(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let wanted: std::collections::HashSet<&String> = track_ids.iter().collect();

        let mut staged = Vec::new();

        // Playlists first: a delete that ran before the removals would leave
        // the playlist rows pointing at a track that no longer exists.
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
                if !wanted.contains(&entry.content_id) {
                    continue;
                }
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::PlaylistRemoveTrack,
                        target_id: Some(playlist.id.clone()),
                        field: None,
                        old_value: None,
                        new_value: Some(serde_json::json!(entry.content_id)),
                        reason: Some("Archive cleanup".to_string()),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| e.to_string())?;
                staged.push(record.id);
            }
        }

        for id in &track_ids {
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackDelete,
                    target_id: Some(id.clone()),
                    field: None,
                    old_value: None,
                    new_value: None,
                    reason: Some("Archive cleanup".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            staged.push(record.id);
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn facts(id: &str, archived_at: i64, has_cues: bool, in_a_playlist: bool) -> ArchivedFacts {
        ArchivedFacts {
            track_id: id.into(),
            archived_at,
            has_cues,
            in_a_playlist,
        }
    }

    fn sample() -> Vec<ArchivedFacts> {
        vec![
            // Archived a year ago, prepared, in a set.
            facts("old", 0, true, true),
            // Archived today, never prepared, in no set.
            facts("new", 365 * DAY, false, false),
        ]
    }

    #[test]
    fn age_selects_only_what_is_actually_older() {
        let now = 365 * DAY;
        assert_eq!(
            select(&sample(), ArchiveCriterion::OlderThanDays(180), now),
            vec!["old"]
        );
    }

    #[test]
    fn a_zero_day_threshold_does_not_sweep_up_something_archived_this_second() {
        // Almost certainly a misclick on the way to picking a real threshold.
        let now = 100;
        let f = vec![facts("just now", 100, false, false)];
        assert!(select(&f, ArchiveCriterion::OlderThanDays(0), now).is_empty());
    }

    #[test]
    fn without_cues_finds_the_tracks_that_were_never_prepared() {
        assert_eq!(
            select(&sample(), ArchiveCriterion::WithoutCues, 365 * DAY),
            vec!["new"]
        );
    }

    #[test]
    fn in_no_playlist_finds_the_tracks_no_set_would_miss() {
        assert_eq!(
            select(&sample(), ArchiveCriterion::InNoPlaylist, 365 * DAY),
            vec!["new"]
        );
    }

    #[test]
    fn nothing_archived_selects_nothing_rather_than_everything() {
        // A criterion over an empty archive must not be read as "all tracks".
        assert!(select(&[], ArchiveCriterion::WithoutCues, 0).is_empty());
    }

    #[test]
    fn criteria_round_trip_through_json() {
        let all = vec![
            ArchiveCriterion::OlderThanDays(180),
            ArchiveCriterion::WithoutCues,
            ArchiveCriterion::InNoPlaylist,
        ];
        let json = serde_json::to_string(&all).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<ArchiveCriterion>>(&json).unwrap(),
            all
        );
    }
}
