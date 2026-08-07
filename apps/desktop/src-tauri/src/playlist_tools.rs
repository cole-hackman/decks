//! Playlist Tools (Epic 6) — the IPC surface for `changes::playlist_tools`.
//!
//! Per `docs/lexicon/02-library.md §Playlist Tools`. Five tools, all
//! preview-then-apply, all staging changes that go through review and Sync —
//! nothing here writes to `master.db`.
//!
//! **Rewrite Order is the point of the set.** It has no visible effect inside
//! `decks`, exactly as it has none inside Lexicon: its purpose is that a CDJ
//! can only sort by a handful of columns and knows nothing about Energy. Sort
//! by Energy here, rewrite the order, and the playlist arrives on the gear in
//! that order.

use std::path::Path;

use changes::playlist_tools::{
    cross_reference, merge, plan_rewrite_order, prefix_names, sort_playlists, CrossReferenceMode,
    PlaylistRename, PlaylistSortMode, PlaylistSummary, PrefixSpec, RewriteOrderPlan,
};
use decks_core::rekordbox_db::RekordboxDb;
use serde::{Deserialize, Serialize};

use crate::cache_db;

fn open_db(path: &str) -> Result<RekordboxDb, String> {
    RekordboxDb::open(Path::new(path)).map_err(|e| e.to_string())
}

/// Load the given playlists with their tracks, in the order asked for.
///
/// The order of `ids` is load-bearing: Merge concatenates in it, and Prefix
/// numbers in it. Silently returning database order would make both tools do
/// something other than what the user arranged.
fn summaries(db: &RekordboxDb, ids: &[String]) -> Result<Vec<PlaylistSummary>, String> {
    let all = db.playlists().map_err(|e| e.to_string())?;
    ids.iter()
        .map(|id| {
            let Some(p) = all.iter().find(|p| &p.id == id) else {
                return Err(format!("playlist not found: {id}"));
            };
            let mut entries = db.playlist_entries(id).map_err(|e| e.to_string())?;
            entries.sort_by_key(|e| e.track_no.unwrap_or(i64::MAX));
            Ok(PlaylistSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                parent_id: p.parent_id.clone(),
                track_ids: entries.into_iter().map(|e| e.content_id).collect(),
            })
        })
        .collect()
}

fn stage(
    cache: &cache::CacheDb,
    library_path: &str,
    kind: changes::ChangeKind,
    target_id: Option<String>,
    new_value: serde_json::Value,
    old_value: Option<serde_json::Value>,
    reason: &str,
) -> Result<String, String> {
    cache
        .stage_change(changes::NewChange {
            library_path: Some(library_path.to_string()),
            kind,
            target_id,
            field: None,
            old_value,
            new_value: Some(new_value),
            reason: Some(reason.to_string()),
            confidence: Some(1.0),
        })
        .map(|r| r.id)
        .map_err(|e| e.to_string())
}

// ── Merge ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MergePreview {
    /// Tracks the merged playlist would hold, in order.
    pub track_ids: Vec<String>,
    /// How many rows the sources held before duplicates were dropped.
    pub source_rows: usize,
}

#[tauri::command]
pub async fn preview_playlist_merge(
    path: String,
    playlist_ids: Vec<String>,
) -> Result<MergePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let lists = summaries(&db, &playlist_ids)?;
        let source_rows = lists.iter().map(|p| p.track_ids.len()).sum();
        Ok(MergePreview {
            track_ids: merge(&lists),
            source_rows,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the new playlist and its members.
///
/// The sources are left alone. Lexicon's Merge creates a *new* playlist rather
/// than consuming the ones it combines, and a tool that quietly deleted four
/// playlists to make a fifth would be a different, much worse tool.
#[tauri::command]
pub async fn apply_playlist_merge(
    app: tauri::AppHandle,
    library_path: String,
    name: String,
    parent_id: Option<String>,
    track_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("the merged playlist needs a name".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let playlist_id = uuid::Uuid::new_v4().to_string();
        let mut staged = vec![stage(
            &cache,
            &library_path,
            changes::ChangeKind::PlaylistCreate,
            Some(playlist_id.clone()),
            serde_json::json!({ "name": name, "parent_id": parent_id, "attribute": 0 }),
            None,
            "Playlist merge",
        )?];
        for track_id in &track_ids {
            staged.push(stage(
                &cache,
                &library_path,
                changes::ChangeKind::PlaylistAddTrack,
                Some(playlist_id.clone()),
                serde_json::json!(track_id),
                None,
                "Playlist merge",
            )?);
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Sort ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SortPreview {
    /// Playlist ids in the new order, with their names for display.
    pub order: Vec<(String, String)>,
    pub unchanged: bool,
}

/// Preview sorting the playlists **inside one folder** — not the tracks.
///
/// `parent_id` is `None` for the root level.
#[tauri::command]
pub async fn preview_playlist_sort(
    path: String,
    parent_id: Option<String>,
    mode: PlaylistSortMode,
) -> Result<SortPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let all = db.playlists().map_err(|e| e.to_string())?;
        let mut siblings: Vec<_> = all
            .iter()
            .filter(|p| p.parent_id == parent_id)
            .cloned()
            .collect();
        siblings.sort_by_key(|p| (p.seq.unwrap_or(i64::MAX), p.name.clone()));

        let lists: Vec<PlaylistSummary> = siblings
            .iter()
            .map(|p| {
                let entries = db.playlist_entries(&p.id).unwrap_or_default();
                PlaylistSummary {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    parent_id: p.parent_id.clone(),
                    track_ids: entries.into_iter().map(|e| e.content_id).collect(),
                }
            })
            .collect();

        let sorted = sort_playlists(&lists, mode);
        let current: Vec<String> = lists.iter().map(|p| p.id.clone()).collect();
        let order = sorted
            .iter()
            .map(|id| {
                let name = lists
                    .iter()
                    .find(|p| &p.id == id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                (id.clone(), name)
            })
            .collect();

        Ok(SortPreview {
            unchanged: sorted == current,
            order,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_playlist_sort(
    app: tauri::AppHandle,
    library_path: String,
    parent_id: Option<String>,
    order: Vec<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        stage(
            &cache,
            &library_path,
            changes::ChangeKind::PlaylistReorder,
            parent_id,
            serde_json::json!({ "order": order }),
            None,
            "Sort playlists",
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Cross Reference ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CrossReferencePreview {
    pub track_ids: Vec<String>,
    /// How many tracks were weighed — the playlists' union for `InAll`, the
    /// whole library for `InNone`. The spec warns `InNone` can be huge, and a
    /// count is how the UI can say so honestly.
    pub considered: usize,
}

#[tauri::command]
pub async fn preview_cross_reference(
    path: String,
    playlist_ids: Vec<String>,
    mode: CrossReferenceMode,
) -> Result<CrossReferencePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let lists = summaries(&db, &playlist_ids)?;
        let library: Vec<String> = match mode {
            CrossReferenceMode::InNone => db
                .tracks()
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|t| t.id)
                .collect(),
            CrossReferenceMode::InAll => Vec::new(),
        };
        let considered = match mode {
            CrossReferenceMode::InNone => library.len(),
            CrossReferenceMode::InAll => lists.first().map_or(0, |p| p.track_ids.len()),
        };
        Ok(CrossReferencePreview {
            track_ids: cross_reference(&lists, &library, mode),
            considered,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Prefix ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn preview_playlist_prefix(
    path: String,
    playlist_ids: Vec<String>,
    spec: PrefixSpec,
) -> Result<Vec<PlaylistRename>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let lists = summaries(&db, &playlist_ids)?;
        Ok(prefix_names(&lists, &spec))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_playlist_prefix(
    app: tauri::AppHandle,
    library_path: String,
    renames: Vec<PlaylistRename>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        renames
            .into_iter()
            .map(|r| {
                stage(
                    &cache,
                    &library_path,
                    changes::ChangeKind::PlaylistRename,
                    Some(r.id),
                    serde_json::json!({ "name": r.to }),
                    // Recorded so Undo can put the old name back — a rename
                    // with no `old_value` is undo-blocked, and renaming forty
                    // playlists is exactly when someone wants that undo.
                    Some(serde_json::json!({ "name": r.from })),
                    "Playlist prefix",
                )
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Rewrite Order ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RewriteOrderRequest {
    pub playlist_id: String,
    /// The track ids in the order they are currently shown.
    pub visible_order: Vec<String>,
}

#[tauri::command]
pub async fn preview_rewrite_order(
    path: String,
    request: RewriteOrderRequest,
) -> Result<RewriteOrderPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let lists = summaries(&db, std::slice::from_ref(&request.playlist_id))?;
        let Some(playlist) = lists.first() else {
            return Err(format!("playlist not found: {}", request.playlist_id));
        };
        Ok(plan_rewrite_order(playlist, &request.visible_order))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn apply_rewrite_order(
    app: tauri::AppHandle,
    library_path: String,
    plan: RewriteOrderPlan,
) -> Result<Option<String>, String> {
    if plan.unchanged {
        // Nothing to review and nothing to sync. Staging a no-op would put a
        // row in the change list that does not change anything.
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        stage(
            &cache,
            &library_path,
            changes::ChangeKind::PlaylistReorderTrack,
            Some(plan.playlist_id),
            serde_json::json!({ "order": plan.order }),
            None,
            "Rewrite playlist order",
        )
        .map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Playlist Occurrence ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OccurrenceReport {
    /// Tracks in exactly `n` playlists.
    pub tracks: Vec<decks_core::rekordbox_db::Track>,
    /// `[playlist_count, how_many_tracks]`, ascending. The point of shipping
    /// this alongside the answer: a bare "how many playlists?" box asks the
    /// user to guess a number, and the distribution is what makes the guess
    /// unnecessary.
    pub distribution: Vec<(usize, usize)>,
}

/// "Which tracks appear in exactly N playlists?" — `N = 0` finds orphans.
///
/// Read-only. This is a report, not a plan; nothing is staged either way.
#[tauri::command]
pub async fn playlist_occurrence(path: String, n: usize) -> Result<OccurrenceReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&path)?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let counts = db.playlist_occurrence().map_err(|e| e.to_string())?;

        // A track absent from `counts` is in zero playlists — the GROUP BY
        // cannot see it, so the zero has to come from the library side.
        let mut distribution: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        let mut matching = Vec::new();
        for track in tracks {
            let count = counts.get(&track.id).copied().unwrap_or(0);
            *distribution.entry(count).or_insert(0) += 1;
            if count == n {
                matching.push(track);
            }
        }

        Ok(OccurrenceReport {
            tracks: matching,
            distribution: distribution.into_iter().collect(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Favourite playlists ──────────────────────────────────────────────────────

/// A starred playlist, resolved to its name and hotkey.
#[derive(Debug, Serialize)]
pub struct FavouritePlaylistView {
    pub playlist_id: String,
    pub name: String,
    /// 1-based hotkey position.
    pub seq: i64,
    pub track_count: usize,
}

/// The starred playlists for a library, in hotkey order.
///
/// A favourite whose playlist no longer exists is **dropped from the result
/// and from the table**, and the rest close up. Leaving it would strand a
/// hotkey on a playlist that is gone; renumbering only in the response would
/// make the hotkey mean something different from what is stored.
#[tauri::command]
pub async fn list_favourite_playlists(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<FavouritePlaylistView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let stored = cache
            .list_favourite_playlists(&library_path)
            .map_err(|e| e.to_string())?;
        if stored.is_empty() {
            return Ok(Vec::new());
        }

        let db = open_db(&library_path)?;
        let all = db.playlists().map_err(|e| e.to_string())?;

        let live: Vec<_> = stored
            .iter()
            .filter_map(|fav| all.iter().find(|p| p.id == fav.playlist_id))
            .collect();

        if live.len() != stored.len() {
            // Prune, so the stored order and the shown order never disagree.
            let ids: Vec<String> = live.iter().map(|p| p.id.clone()).collect();
            cache
                .set_favourite_playlist_order(&library_path, &ids)
                .map_err(|e| e.to_string())?;
        }

        Ok(live
            .into_iter()
            .enumerate()
            .map(|(i, p)| FavouritePlaylistView {
                track_count: db.playlist_entries(&p.id).map(|e| e.len()).unwrap_or(0),
                playlist_id: p.id.clone(),
                name: p.name.clone(),
                seq: (i as i64) + 1,
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Star or unstar. Returns the new state.
#[tauri::command]
pub async fn toggle_favourite_playlist(
    app: tauri::AppHandle,
    library_path: String,
    playlist_id: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .toggle_favourite_playlist(&library_path, &playlist_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn set_favourite_playlist_order(
    app: tauri::AppHandle,
    library_path: String,
    playlist_ids: Vec<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        cache
            .set_favourite_playlist_order(&library_path, &playlist_ids)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Add tracks to a playlist — the "add the selection to favourite N" half of
/// the spec's fast filing system.
///
/// Staged like everything else. Tracks the playlist already holds are skipped
/// rather than added twice; returns the staged change ids.
#[tauri::command]
pub async fn add_tracks_to_playlist(
    app: tauri::AppHandle,
    library_path: String,
    playlist_id: String,
    track_ids: Vec<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&library_path)?;
        let present: std::collections::HashSet<String> = db
            .playlist_entries(&playlist_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|e| e.content_id)
            .collect();

        let cache = cache_db(&app)?;
        track_ids
            .into_iter()
            .filter(|id| !present.contains(id))
            .map(|track_id| {
                stage(
                    &cache,
                    &library_path,
                    changes::ChangeKind::PlaylistAddTrack,
                    Some(playlist_id.clone()),
                    serde_json::json!(track_id),
                    None,
                    "Filed to a favourite playlist",
                )
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── M3U import ───────────────────────────────────────────────────────────────

/// One M3U line, matched against the library.
#[derive(Debug, Serialize)]
pub struct M3uImportRow {
    pub path: String,
    /// The `#EXTINF` label, for rows that did not match — it is the only thing
    /// left to identify the track by.
    pub label: Option<String>,
    /// `None` when the path is not in the library.
    pub track_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct M3uImportPreview {
    pub rows: Vec<M3uImportRow>,
    pub matched: usize,
    pub unmatched: usize,
    /// Suggested playlist name, from the file.
    pub suggested_name: String,
}

fn path_key(path: &str) -> String {
    path.trim().replace('\\', "/").to_lowercase()
}

fn file_name_key(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next()?.trim();
    (!name.is_empty()).then(|| name.to_lowercase())
}

/// Match an M3U against the library: full path first, then filename.
///
/// Same priority as the history re-match, and for the same reason — an M3U
/// written on another machine has paths that will not resolve here, but the
/// filenames usually still do. An **ambiguous filename is no match**: putting
/// an arbitrary one of two same-named tracks into the playlist is worse than
/// saying it could not be found.
#[tauri::command]
pub async fn preview_m3u_import(
    path: String,
    file_name: String,
    content: String,
) -> Result<M3uImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entries = share::parse_m3u(&content);
        let tracks = open_db(&path)?.tracks().map_err(|e| e.to_string())?;

        let mut by_path: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
        let mut by_name: std::collections::HashMap<String, Vec<&str>> =
            std::collections::HashMap::new();
        for t in &tracks {
            let Some(p) = t.folder_path.as_deref() else {
                continue;
            };
            by_path.entry(path_key(p)).or_insert(t.id.as_str());
            if let Some(n) = file_name_key(p) {
                by_name.entry(n).or_default().push(t.id.as_str());
            }
        }

        let rows: Vec<M3uImportRow> = entries
            .into_iter()
            .map(|e| {
                let track_id = by_path
                    .get(&path_key(&e.path))
                    .map(|id| (*id).to_string())
                    .or_else(|| match by_name.get(&file_name_key(&e.path)?)?.as_slice() {
                        [only] => Some((*only).to_string()),
                        _ => None,
                    });
                M3uImportRow {
                    path: e.path,
                    label: e.label,
                    track_id,
                }
            })
            .collect();

        let matched = rows.iter().filter(|r| r.track_id.is_some()).count();
        let suggested_name = file_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&file_name)
            .rsplit_once('.')
            .map(|(stem, _)| stem.to_string())
            .unwrap_or_else(|| file_name.clone());

        Ok(M3uImportPreview {
            unmatched: rows.len() - matched,
            matched,
            rows,
            suggested_name: if suggested_name.trim().is_empty() {
                "Imported playlist".to_string()
            } else {
                suggested_name
            },
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use changes::playlist_tools::{CrossReferenceMode, PlaylistSortMode, PrefixSpec};

    #[test]
    fn sort_modes_deserialise_from_the_names_the_ui_sends() {
        for (json, expected) in [
            ("\"name_asc\"", PlaylistSortMode::NameAsc),
            ("\"name_desc\"", PlaylistSortMode::NameDesc),
            ("\"track_count_desc\"", PlaylistSortMode::TrackCountDesc),
        ] {
            assert_eq!(
                serde_json::from_str::<PlaylistSortMode>(json).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn cross_reference_modes_deserialise() {
        assert_eq!(
            serde_json::from_str::<CrossReferenceMode>("\"in_all\"").unwrap(),
            CrossReferenceMode::InAll
        );
        assert_eq!(
            serde_json::from_str::<CrossReferenceMode>("\"in_none\"").unwrap(),
            CrossReferenceMode::InNone
        );
    }

    #[test]
    fn a_prefix_spec_with_no_numbering_round_trips() {
        let spec: PrefixSpec = serde_json::from_str(r#"{"text": "2026 ", "numbering": null}"#)
            .expect("text-only prefix");
        assert_eq!(spec.text, "2026 ");
        assert!(spec.numbering.is_none());
    }

    #[test]
    fn numbering_defaults_fill_in() {
        // The UI may omit pad and replace_existing entirely.
        let spec: PrefixSpec =
            serde_json::from_str(r#"{"numbering": {"start": 1}}"#).expect("bare numbering");
        let n = spec.numbering.expect("numbering");
        assert_eq!(n.start, 1);
        assert_eq!(n.pad, 0);
        assert!(!n.replace_existing);
        assert_eq!(spec.text, "");
    }
}
