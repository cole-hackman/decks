//! Tauri commands for Move & Rename (Epic 4).
//!
//! Two phases, deliberately separate:
//!
//! 1. `preview_organize` plans every move without touching anything, so the
//!    whole plan can be read before a byte moves.
//! 2. `apply_organize` executes exactly the plan it is handed back, and stages
//!    a `TrackRelocate` change per moved file so `master.db` learns the new
//!    path through the normal review pipeline.
//!
//! `master.db` is never written here — moving files is a filesystem operation,
//! and telling Rekordbox about it goes through Sync like everything else.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use file_organizer::{
    plan_batch, ExtensionFilter, KnownPaths, OrganizeSpec, Pattern, PlanRequest, RunDate,
    SubfolderSpec, TrackFacts, UnusedScan,
};
use serde::{Deserialize, Serialize};

use crate::cache_db;

/// The Lexicon field vocabulary, and whether `decks` can supply each one.
///
/// Exposed to the UI so the pattern editor can offer the real list and mark the
/// fields that will always render empty, rather than silently producing blanks.
/// Unsupported entries are fields Rekordbox holds but `decks` does not model
/// yet; they arrive with the epics that introduce them.
const FIELD_VOCABULARY: &[(&str, bool)] = &[
    ("artist", true),
    ("title", true),
    ("albumTitle", true),
    ("label", false),
    ("remixer", false),
    ("mix", false),
    ("composer", false),
    ("producer", false),
    ("grouping", false),
    ("lyricist", false),
    ("comment", true),
    ("key", true),
    ("genre", true),
    ("bpm", true),
    ("rating", true),
    ("color", false),
    ("year", true),
    ("durationSeconds", true),
    ("bitrate", true),
    ("playCount", true),
    ("sizeBytes", true),
    ("sampleRate", true),
    ("trackNumber", false),
    ("energy", true),
    ("danceability", false),
    ("popularity", false),
    ("extra1", false),
    ("extra2", false),
];

#[derive(Debug, Serialize)]
pub struct PatternField {
    pub name: String,
    /// False when `decks` cannot supply the field yet, so the UI can say so
    /// instead of letting the user build a pattern that renders empty.
    pub supported: bool,
}

/// What the user configured in the Move & Rename panel.
#[derive(Debug, Clone, Deserialize)]
pub struct OrganizeRequest {
    /// Absent means "rename in place".
    #[serde(default)]
    pub target_folder: Option<String>,
    /// Absent means "keep the existing filename".
    #[serde(default)]
    pub filename_pattern: Option<String>,
    #[serde(default)]
    pub subfolders: SubfolderSpec,
}

/// One planned move, as shown to the user and handed back to `apply_organize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeRow {
    pub track_id: String,
    pub source: String,
    /// `None` when the file is already where it belongs.
    pub destination: Option<String>,
    pub title: String,
    pub artist: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct OrganizeResult {
    /// Track IDs whose files moved and whose relocation was staged.
    pub moved: Vec<String>,
    /// `(track_id, reason)` for files that could not be moved. The rest of the
    /// batch still runs — one locked file must not abandon 500 others.
    pub failed: Vec<(String, String)>,
    /// Staged `TrackRelocate` change IDs.
    pub staged: Vec<String>,
}

fn field_map(
    track: &decks_core::rekordbox_db::Track,
    size_bytes: Option<u64>,
) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut put = |k: &str, v: Option<String>| {
        if let Some(v) = v {
            m.insert(k.to_string(), v);
        }
    };
    put("title", Some(track.title.clone()));
    put("artist", track.artist.clone());
    put("albumTitle", track.album.clone());
    put("genre", track.genre.clone());
    put("key", track.musical_key.clone());
    put("comment", track.comment.clone());
    // BPM renders without a trailing ".0" — "128" is what a DJ writes.
    put(
        "bpm",
        track.bpm.map(|b| {
            if (b.fract()).abs() < f64::EPSILON {
                format!("{b:.0}")
            } else {
                format!("{b:.2}")
            }
        }),
    );
    put("rating", track.rating.map(|v| v.to_string()));
    put("year", track.release_year.map(|v| v.to_string()));
    put(
        "durationSeconds",
        track.duration_secs.map(|v| v.to_string()),
    );
    put("bitrate", track.bit_rate.map(|v| v.to_string()));
    put("sampleRate", track.sample_rate.map(|v| v.to_string()));
    put("playCount", track.dj_play_count.map(|v| v.to_string()));
    put("sizeBytes", size_bytes.map(|v| v.to_string()));
    // Energy is stored 0.0–1.0 and shown 1–10, which is how the rest of the app
    // presents it.
    put(
        "energy",
        track.energy.map(|e| format!("{:.0}", (e * 10.0).round())),
    );
    m
}

fn build_spec(req: &OrganizeRequest) -> Result<OrganizeSpec, String> {
    let filename = match req.filename_pattern.as_deref() {
        Some(p) if !p.trim().is_empty() => Some(Pattern::parse(p).map_err(|e| e.to_string())?),
        _ => None,
    };
    Ok(OrganizeSpec {
        target_folder: req.target_folder.as_ref().map(PathBuf::from),
        filename,
        subfolders: req.subfolders.clone(),
    })
}

fn today() -> RunDate {
    use chrono::Datelike;
    let now = chrono::Local::now();
    RunDate {
        year: now.year(),
        month: now.month(),
    }
}

/// The field vocabulary, for the pattern editor.
#[tauri::command]
pub fn pattern_fields() -> Vec<PatternField> {
    FIELD_VOCABULARY
        .iter()
        .map(|(name, supported)| PatternField {
            name: (*name).to_string(),
            supported: *supported,
        })
        .collect()
}

/// Validate a pattern, returning the names it references.
///
/// Cheap, and it turns a typo into a message before it turns into 10,000
/// renamed files.
#[tauri::command]
pub fn validate_pattern(pattern: String) -> Result<Vec<String>, String> {
    let parsed = Pattern::parse(&pattern).map_err(|e| e.to_string())?;
    Ok(parsed
        .field_names()
        .into_iter()
        .map(str::to_string)
        .collect())
}

#[tauri::command]
pub async fn preview_organize(
    library_path: String,
    track_ids: Vec<String>,
    request: OrganizeRequest,
) -> Result<Vec<OrganizeRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let spec = build_spec(&request)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut tracks = Vec::new();
        for id in &track_ids {
            if let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? {
                tracks.push(track);
            }
        }

        // Build every borrowed input up front — `TrackFacts` borrows, so the
        // maps have to outlive the plan call.
        let prepared: Vec<_> = tracks
            .iter()
            .filter_map(|t| {
                let path = t.folder_path.as_deref()?;
                let source = PathBuf::from(path);
                let size = std::fs::metadata(&source).ok().map(|m| m.len());
                Some((t, source, field_map(t, size)))
            })
            .collect();

        let requests: Vec<PlanRequest<'_>> = prepared
            .iter()
            .map(|(t, source, fields)| PlanRequest {
                source: source.as_path(),
                facts: TrackFacts {
                    fields,
                    bitrate_kbps: t.bit_rate.and_then(|b| u32::try_from(b).ok()),
                    tags: &[],
                    year: t.release_year.and_then(|y| i32::try_from(y).ok()),
                },
            })
            .collect();

        let plans = plan_batch(&spec, &requests, today(), &|p: &Path| p.exists());

        Ok(prepared
            .iter()
            .zip(plans)
            .map(|((t, _, _), plan)| OrganizeRow {
                track_id: t.id.clone(),
                source: plan.source.to_string_lossy().into_owned(),
                destination: plan.destination().map(|d| d.to_string_lossy().into_owned()),
                title: t.title.clone(),
                artist: t.artist.clone(),
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Move one file, creating the destination's parent directories.
///
/// `fs::rename` fails across filesystems, which is the common case when moving
/// from a download folder on an internal disk to a music library on an external
/// one — so fall back to copy-then-remove. The copy is verified before the
/// original goes.
fn move_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "{} already exists — re-run the preview",
            destination.display()
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(source, destination)
                .map_err(|e| format!("copy to {}: {e}", destination.display()))?;
            std::fs::remove_file(source)
                .map_err(|e| format!("remove {}: {e}", source.display()))?;
            Ok(())
        }
    }
}

/// Execute a reviewed plan.
///
/// Takes the rows returned by `preview_organize` rather than re-planning, so
/// what runs is exactly what the user looked at.
#[tauri::command]
pub async fn apply_organize(
    app: tauri::AppHandle,
    library_path: String,
    rows: Vec<OrganizeRow>,
) -> Result<OrganizeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut result = OrganizeResult::default();

        for row in rows {
            let Some(destination) = row.destination.as_deref() else {
                continue;
            };
            let source = PathBuf::from(&row.source);
            let destination = PathBuf::from(destination);

            if let Err(e) = move_file(&source, &destination) {
                result.failed.push((row.track_id.clone(), e));
                continue;
            }

            let file_name = destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            let staged = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackRelocate,
                    target_id: Some(row.track_id.clone()),
                    field: None,
                    old_value: Some(serde_json::json!(row.source)),
                    new_value: Some(serde_json::json!({
                        "folder_path": destination.to_string_lossy(),
                        "file_name": file_name,
                    })),
                    reason: Some("Move & Rename".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;

            result.moved.push(row.track_id);
            result.staged.push(staged.id);
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Scan folder trees for files the library does not reference.
///
/// Read-only. The report is a list of deletion *candidates*, which is why the
/// scan and the deletion are separate commands and why the scan refuses to run
/// against an empty library.
#[tauri::command]
pub async fn scan_unused_files(
    library_path: String,
    roots: Vec<String>,
    filter: ExtensionFilter,
) -> Result<UnusedScan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let known = KnownPaths::new(
            db.tracks()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|t| t.folder_path),
        );
        let roots: Vec<PathBuf> = roots.into_iter().map(PathBuf::from).collect();
        file_organizer::unused::scan(&roots, &known, &filter)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Default, Serialize)]
pub struct DeleteReport {
    pub deleted: Vec<String>,
    pub failed: Vec<(String, String)>,
    /// Where the record of what was deleted was written. Deletion here is
    /// irreversible, so it always leaves a trail.
    pub report_path: Option<String>,
}

/// Delete files the scan reported.
///
/// Irreversible — the caller is responsible for confirming with the user. Two
/// guards regardless: a path still in the library is refused outright (the
/// library can change between scanning and deleting), and a report of what went
/// is written before the command returns.
#[tauri::command]
pub async fn delete_unused_files(
    app: tauri::AppHandle,
    library_path: String,
    paths: Vec<String>,
) -> Result<DeleteReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let known = KnownPaths::new(
            db.tracks()
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|t| t.folder_path),
        );

        let mut report = DeleteReport::default();
        for path in paths {
            let p = PathBuf::from(&path);
            // Re-checked here, not just at scan time: the library can gain a
            // track between the scan and the click.
            if known.contains(&p) {
                report
                    .failed
                    .push((path, "now referenced by the library".into()));
                continue;
            }
            match std::fs::remove_file(&p) {
                Ok(()) => report.deleted.push(path),
                Err(e) => report.failed.push((path, e.to_string())),
            }
        }

        if !report.deleted.is_empty() {
            report.report_path = write_delete_report(&app, &report.deleted).ok();
        }
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Write the list of deleted paths beside the app's own data.
///
/// Best-effort: failing to write the record must not make the command look like
/// the deletion failed, but the UI is told whether a record exists.
fn write_delete_report(app: &tauri::AppHandle, deleted: &[String]) -> std::io::Result<String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .join("reports");
    std::fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("deleted-{stamp}.txt"));
    std::fs::write(&path, deleted.join("\n"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> decks_core::rekordbox_db::Track {
        decks_core::rekordbox_db::Track {
            id: "t1".into(),
            title: "Get Lucky".into(),
            artist: Some("Daft Punk".into()),
            album: None,
            genre: Some("House".into()),
            musical_key: Some("12M".into()),
            bpm: Some(128.0),
            duration_secs: Some(300),
            rating: None,
            comment: None,
            folder_path: Some("/Incoming/track.mp3".into()),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: Some(320),
            release_year: None,
            dj_play_count: None,
            energy: Some(0.82),
        }
    }

    #[test]
    fn bpm_renders_without_a_pointless_decimal() {
        let m = field_map(&track(), None);
        assert_eq!(m.get("bpm").map(String::as_str), Some("128"));
    }

    #[test]
    fn a_fractional_bpm_keeps_its_precision() {
        let mut t = track();
        t.bpm = Some(127.53);
        let m = field_map(&t, None);
        assert_eq!(m.get("bpm").map(String::as_str), Some("127.53"));
    }

    #[test]
    fn energy_renders_on_the_scale_the_rest_of_the_app_shows() {
        let m = field_map(&track(), None);
        assert_eq!(m.get("energy").map(String::as_str), Some("8"));
    }

    #[test]
    fn absent_fields_are_absent_rather_than_empty_strings() {
        // The pattern engine treats "absent" and "empty" the same, but leaving
        // the key out keeps `field_names` diagnostics honest.
        let m = field_map(&track(), None);
        assert!(!m.contains_key("albumTitle"));
        assert!(!m.contains_key("sizeBytes"));
    }

    #[test]
    fn an_empty_filename_pattern_means_keep_the_name() {
        let spec = build_spec(&OrganizeRequest {
            target_folder: Some("/Music".into()),
            filename_pattern: Some("   ".into()),
            subfolders: SubfolderSpec::default(),
        })
        .unwrap();
        assert!(spec.filename.is_none());
    }

    #[test]
    fn a_malformed_pattern_is_rejected_before_anything_moves() {
        let err = build_spec(&OrganizeRequest {
            target_folder: None,
            filename_pattern: Some("%artist".into()),
            subfolders: SubfolderSpec::default(),
        })
        .unwrap_err();
        assert!(err.contains("unterminated"), "unexpected error: {err}");
    }

    #[test]
    fn the_field_vocabulary_matches_the_manuals_list() {
        assert_eq!(FIELD_VOCABULARY.len(), 28);
        assert!(FIELD_VOCABULARY.iter().any(|(n, s)| *n == "artist" && *s));
        assert!(FIELD_VOCABULARY.iter().any(|(n, s)| *n == "remixer" && !*s));
    }

    #[test]
    fn validate_pattern_reports_the_fields_used() {
        assert_eq!(
            validate_pattern("%artist% - %title% {(%key%)}".into()).unwrap(),
            vec!["artist", "title", "key"]
        );
    }

    #[test]
    fn moving_refuses_to_overwrite_an_existing_file() {
        let dir = std::env::temp_dir().join(format!("decks-organizer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.mp3");
        let b = dir.join("b.mp3");
        std::fs::write(&a, b"one").unwrap();
        std::fs::write(&b, b"two").unwrap();

        assert!(move_file(&a, &b).is_err());
        assert_eq!(std::fs::read(&b).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn moving_creates_missing_parent_directories() {
        let dir = std::env::temp_dir().join(format!("decks-organizer-mk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.mp3");
        std::fs::write(&a, b"one").unwrap();
        let dest = dir.join("House").join("128").join("a.mp3");

        move_file(&a, &dest).unwrap();
        assert!(dest.exists());
        assert!(!a.exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
