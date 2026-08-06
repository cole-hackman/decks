//! Watch folders (Epic 4).
//!
//! A folder the user drops music into. Arrivals are found by a debounced scan
//! rather than a native filesystem watcher — the arrival set is a pure function
//! of (files on disk, library, dismissed), which means it cannot miss something
//! that happened while the app was closed, and it needs no platform-specific
//! dependency. See `file_organizer::watch`.
//!
//! Importing is staged, never applied: adding a row to `djmdContent` needs
//! columns `decks` does not model, so `TrackCreate` is export-only and reaches
//! Rekordbox through its own XML import.
//!
//! Per `docs/lexicon/06-files.md §Watch Folder` and §Incoming Tracks.

use std::path::{Path, PathBuf};

use file_organizer::{scan_watch_folders, KnownPaths, WatchScan};
use serde::Serialize;

use crate::cache_db;
use crate::organizer::path_mappings;

#[derive(Debug, Serialize)]
pub struct WatchFolderRow {
    pub id: String,
    pub path: String,
}

#[tauri::command]
pub fn list_watch_folders(app: tauri::AppHandle) -> Result<Vec<WatchFolderRow>, String> {
    Ok(cache_db(&app)?
        .list_watch_folders()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| WatchFolderRow {
            id: f.id,
            path: f.path,
        })
        .collect())
}

#[tauri::command]
pub fn add_watch_folder(app: tauri::AppHandle, path: String) -> Result<String, String> {
    cache_db(&app)?
        .add_watch_folder(&path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_watch_folder(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    cache_db(&app)?
        .remove_watch_folder(&id)
        .map_err(|e| e.to_string())
}

/// Forget every dismissal, so a folder can be triaged from scratch.
#[tauri::command]
pub fn clear_dismissed_arrivals(app: tauri::AppHandle) -> Result<usize, String> {
    cache_db(&app)?
        .clear_dismissed_watch_paths()
        .map_err(|e| e.to_string())
}

/// Mark arrivals as dealt with so later scans stop offering them.
#[tauri::command]
pub fn dismiss_arrivals(app: tauri::AppHandle, paths: Vec<String>) -> Result<usize, String> {
    cache_db(&app)?
        .dismiss_watch_paths(&paths)
        .map_err(|e| e.to_string())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Scan every configured watch folder for files the library does not have.
#[tauri::command]
pub async fn scan_arrivals(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<WatchScan, String> {
    let mappings = path_mappings(&app);
    let cache = cache_db(&app)?;
    let roots: Vec<PathBuf> = cache
        .list_watch_folders()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|f| PathBuf::from(f.path))
        .collect();
    let dismissed = KnownPaths::new(
        cache
            .dismissed_watch_paths()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(PathBuf::from),
    );

    tauri::async_runtime::spawn_blocking(move || {
        if roots.is_empty() {
            return Ok(WatchScan::default());
        }
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        // Resolved through path mappings, or a track the library holds under
        // another machine's prefix would be offered as a fresh arrival.
        let known = KnownPaths::new(
            db.tracks()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|t| t.folder_path)
                .map(|p| mappings.resolve(&p)),
        );
        Ok(scan_watch_folders(&roots, &known, &dismissed, now_secs()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Default, Serialize)]
pub struct ImportResult {
    pub staged: Vec<String>,
    /// `(path, reason)` — a file whose tags cannot be read fails alone.
    pub failed: Vec<(String, String)>,
}

/// Stage arrivals for import as `TrackCreate` changes.
///
/// Tags are read at stage time so the review screen shows what will actually
/// land in Rekordbox, rather than a bare path. Staging also dismisses the file,
/// since it has now been dealt with — otherwise the next scan offers it again
/// and the user stages it twice.
#[tauri::command]
pub async fn stage_arrival_imports(
    app: tauri::AppHandle,
    library_path: String,
    paths: Vec<String>,
) -> Result<ImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut result = ImportResult::default();
        let mut dismissed = Vec::new();

        for path in paths {
            let tags = match audio_tags::read_tags(Path::new(&path)) {
                Ok(t) => t,
                Err(e) => {
                    result.failed.push((path, e.to_string()));
                    continue;
                }
            };
            let payload = serde_json::json!({
                "path": path,
                "title": tags.title,
                "artist": tags.artist,
                "album": tags.album,
                "genre": tags.genre,
                "bpm": tags.bpm,
                "musical_key": tags.musical_key,
                "comment": tags.comment,
                "year": tags.year,
                "duration_secs": tags.duration_secs,
            });
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackCreate,
                    target_id: None,
                    field: None,
                    old_value: None,
                    new_value: Some(payload),
                    reason: Some("Watch folder arrival".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            result.staged.push(record.id);
            dismissed.push(path);
        }

        if !dismissed.is_empty() {
            cache
                .dismiss_watch_paths(&dismissed)
                .map_err(|e| e.to_string())?;
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
