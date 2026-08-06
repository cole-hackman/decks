//! Tauri commands for Import Tags From CSV (Epic 5).
//!
//! Preview-then-apply, like everything else that edits in bulk: the preview is
//! a per-row report, and applying stages `TrackMetadataEdit` changes that go
//! through review and Sync. Nothing is written to `master.db` here.
//!
//! Per `docs/lexicon/10-recipes.md §Import Tags From CSV`.

use std::collections::BTreeMap;
use std::path::Path;

use track_matcher::csv_import::{
    headers, plan, report, ImportCandidate, ImportColumns, ImportReport, PlannedRow, RowOutcome,
};

use crate::cache_db;
use crate::recipes::{column_for, recipe_fields};

/// The header row, for the column-picker dropdowns.
#[tauri::command]
pub fn csv_import_headers(csv: String) -> Result<Vec<String>, String> {
    headers(&csv).map_err(|e| e.to_string())
}

/// Track fields a CSV may write into.
///
/// The same vocabulary the recipes offer, and for the same reason: a field the
/// applier's allowlist will not write would produce a preview full of changes
/// that silently vanish at sync time.
#[tauri::command]
pub fn csv_import_fields() -> Vec<String> {
    recipe_fields()
}

#[derive(Debug, serde::Serialize)]
pub struct CsvImportPreview {
    pub rows: Vec<PlannedRow>,
    pub report: ImportReport,
}

fn candidates(library_path: &str) -> Result<Vec<ImportCandidate>, String> {
    let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(library_path))
        .map_err(|e| e.to_string())?;
    Ok(db
        .tracks()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| {
            let mut current = BTreeMap::new();
            let mut put = |k: &str, v: Option<String>| {
                if let Some(v) = v.filter(|s| !s.trim().is_empty()) {
                    current.insert(k.to_string(), v);
                }
            };
            put("title", Some(t.title.clone()));
            put("artist", t.artist.clone());
            put("album", t.album.clone());
            put("genre", t.genre.clone());
            put("comment", t.comment.clone());
            put("key", t.musical_key.clone());
            put("bpm", t.bpm.map(|b| format!("{b}")));
            put("rating", t.rating.map(|v| v.to_string()));
            put("year", t.release_year.map(|v| v.to_string()));
            put("playCount", t.dj_play_count.map(|v| v.to_string()));
            ImportCandidate {
                id: t.id,
                title: t.title,
                artist: t.artist,
                folder_path: t.folder_path,
                current,
            }
        })
        .collect())
}

/// Resolve every row against the library without changing anything.
#[tauri::command]
pub async fn csv_import_preview(
    library_path: String,
    csv: String,
    columns: ImportColumns,
) -> Result<CsvImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Reject unwritable targets here rather than at apply time, so the
        // preview cannot show a change that was never going to land.
        for (_, field) in &columns.fields {
            if column_for(field).is_none() {
                return Err(format!("{field} is not a field this can write"));
            }
        }
        let rows = track_matcher::csv_import::parse(&csv, &columns).map_err(|e| e.to_string())?;
        let library = candidates(&library_path)?;
        let planned = plan(&rows, &library);
        Ok(CsvImportPreview {
            report: report(&planned),
            rows: planned,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the reviewed rows.
///
/// Takes the planned rows back rather than re-parsing, so what is staged is
/// exactly what was on screen. Rows that matched nothing, matched several
/// tracks, or already agreed carry no changes and are skipped here rather than
/// filtered out by the caller — the report already accounts for them.
#[tauri::command]
pub async fn csv_import_apply(
    app: tauri::AppHandle,
    library_path: String,
    rows: Vec<PlannedRow>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut staged = Vec::new();
        for planned in &rows {
            let RowOutcome::Matched {
                track_id, changes, ..
            } = &planned.outcome
            else {
                continue;
            };
            for (field, before, after) in changes {
                let Some(column) = column_for(field) else {
                    continue;
                };
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::TrackMetadataEdit,
                        target_id: Some(track_id.clone()),
                        field: Some(column.to_string()),
                        old_value: Some(match before {
                            Some(v) => serde_json::json!(v),
                            None => serde_json::Value::Null,
                        }),
                        new_value: Some(serde_json::json!(after)),
                        reason: Some(format!("CSV import — row {}", planned.row.line)),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| e.to_string())?;
                staged.push(record.id);
            }
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_importable_field_maps_to_a_writable_column() {
        // The preview rejects anything else, so the two lists must agree.
        for field in csv_import_fields() {
            assert!(column_for(&field).is_some(), "{field} has no column");
        }
    }

    #[test]
    fn an_excel_byte_order_mark_does_not_break_the_header_list() {
        let got = csv_import_headers("\u{feff}Artist,Title\n".into()).unwrap();
        assert_eq!(got, vec!["Artist", "Title"]);
    }

    #[test]
    fn a_ragged_file_reports_rather_than_panicking() {
        // A row with more cells than the header is the classic hand-edited-CSV
        // failure. The reader is lenient about the *header* — an unterminated
        // quote just runs to the end of the file — so the error surfaces when
        // the rows are read, which is where the command reports it.
        let columns = ImportColumns {
            artist: Some("Artist".into()),
            title: Some("Title".into()),
            fields: vec![("Genre".into(), "genre".into())],
            ..Default::default()
        };
        assert!(track_matcher::csv_import::parse(
            "Artist,Title,Genre\na,b,House,extra\n",
            &columns,
        )
        .is_err());
    }
}
