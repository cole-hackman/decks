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
    /// Files that were analysed on the way in, because "Auto-analyse new
    /// tracks" is on. Reported so the UI can say it happened rather than
    /// leaving the user wondering why the import took longer.
    pub analysed: Vec<String>,
    /// Files whose tags were rewritten, because "Auto-write file tags" is on.
    /// Reported separately from `analysed`: analysis is read-only, writing
    /// touches the user's file, and the two should never be confused in a
    /// summary line.
    pub tagged: Vec<String>,
    /// `(path, reason)` where a tag write was skipped or failed. Surfaced
    /// rather than swallowed — a silent skip on a setting the user turned on
    /// looks like the setting does not work.
    pub tag_skipped: Vec<(String, String)>,
}

/// Below this, an analysis result is not written into the user's file.
///
/// Auto-writing means overwriting whatever BPM or key tag the file already
/// carried, without anyone looking at it. A low-confidence detection is a
/// guess, and ADR-0008 is explicit that a guess must not be presented — still
/// less written — as fact. The threshold is deliberately high: the cost of not
/// writing is that the user does it by hand, and the cost of writing wrongly is
/// a corrupted tag they may never notice.
const AUTO_WRITE_MIN_CONFIDENCE: f64 = 0.75;

/// Write a confident analysis result back into the file's tags.
///
/// Returns `Ok(false)` when the result was not confident enough — a skip, not
/// a failure, and reported as such.
///
/// Only BPM and key are written. Everything else in the file's tags came *from*
/// the file moments earlier, so writing it back would be a no-op that still
/// rewrites the user's file — and every rewrite is a chance to lose a frame
/// `lofty` does not model.
fn write_analysis_tags(
    path: &Path,
    analysis: &audio_analysis::AnalysisResult,
) -> Result<bool, String> {
    if analysis.confidence < AUTO_WRITE_MIN_CONFIDENCE {
        return Ok(false);
    }
    let fields = audio_tags::TagWriteFields {
        bpm: Some(analysis.bpm),
        musical_key: Some(analysis.musical_key.clone()),
        ..Default::default()
    };
    audio_tags::write_tag_fields(path, &fields)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

/// Stage arrivals for import as `TrackCreate` changes.
///
/// Tags are read at stage time so the review screen shows what will actually
/// land in Rekordbox, rather than a bare path. Staging also dismisses the file,
/// since it has now been dealt with — otherwise the next scan offers it again
/// and the user stages it twice.
///
/// When "Auto-analyse new tracks" is on, BPM and key are detected here too, and
/// with "Auto-write file tags" also on they are written back into the file.
/// This is the one place the spec's rule bites: automation applies to tracks
/// the user brought in, never to tracks that came from Rekordbox — and an
/// arrival is by definition the former.
///
/// Tag writing requires analysis, because without it there is nothing new to
/// write: the tags were read off this very file a few lines earlier.
#[tauri::command]
pub async fn stage_arrival_imports(
    app: tauri::AppHandle,
    library_path: String,
    paths: Vec<String>,
) -> Result<ImportResult, String> {
    let auto_analyse = crate::automation::is_enabled(&app, crate::automation::AUTO_ANALYZE);
    // Writing tags without analysing first would have nothing new to write —
    // the tags were just read off this very file.
    let auto_write_tags =
        auto_analyse && crate::automation::is_enabled(&app, crate::automation::AUTO_WRITE_TAGS);
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

            if auto_analyse {
                // Analysis failing must not undo an import that already
                // succeeded — a track with no BPM yet is still a track.
                match audio_analysis::analyze_file_cached(Path::new(&path), &path, &cache) {
                    Ok(analysis) => {
                        result.analysed.push(path.clone());
                        if auto_write_tags {
                            match write_analysis_tags(Path::new(&path), &analysis) {
                                Ok(true) => result.tagged.push(path.clone()),
                                Ok(false) => result.tag_skipped.push((
                                    path.clone(),
                                    format!(
                                        "analysis confidence {:.0}% is below the {:.0}% needed to overwrite a tag",
                                        analysis.confidence * 100.0,
                                        AUTO_WRITE_MIN_CONFIDENCE * 100.0
                                    ),
                                )),
                                Err(e) => result.tag_skipped.push((path.clone(), e)),
                            }
                        }
                    }
                    Err(e) => tracing::warn!(path = %path, error = %e, "auto-analysis failed"),
                }
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(confidence: f64) -> audio_analysis::AnalysisResult {
        audio_analysis::AnalysisResult {
            bpm: 128.0,
            musical_key: "8A".into(),
            confidence,
            bpm_confidence: confidence,
            key_confidence: confidence,
            energy: 0.6,
            cached: false,
        }
    }

    /// The guard that keeps auto-write honest.
    #[test]
    fn a_low_confidence_analysis_is_not_written_into_the_users_file() {
        // Auto-writing overwrites whatever BPM or key tag the file carried,
        // with nobody looking. A guess written as fact is exactly what ADR-0008
        // forbids — and a wrong tag is one the user may never notice.
        //
        // The path is never touched, so a `false` here proves the confidence
        // check short-circuits before any file access.
        let missing = Path::new("/definitely/not/a/file.mp3");
        assert_eq!(write_analysis_tags(missing, &analysis(0.4)), Ok(false));
        assert_eq!(write_analysis_tags(missing, &analysis(0.74)), Ok(false));
    }

    #[test]
    fn a_confident_analysis_gets_as_far_as_the_file() {
        // At or above the threshold it stops being a skip and becomes a real
        // write attempt — which fails here only because the path is fake.
        let missing = Path::new("/definitely/not/a/file.mp3");
        assert!(
            write_analysis_tags(missing, &analysis(0.75)).is_err(),
            "0.75 should have attempted the write rather than skipping"
        );
    }

    #[test]
    fn the_threshold_is_stated_rather_than_implied() {
        // A magic number in a branch is a decision nobody can find later.
        assert_eq!(AUTO_WRITE_MIN_CONFIDENCE, 0.75);
    }
}
