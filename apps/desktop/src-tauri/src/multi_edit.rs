//! Tauri commands for the manual multi-track editor (Epic 5).
//!
//! Load the selection collapsed into one form, save the fields the user
//! touched. Edits stage as `TrackMetadataEdit` and go through review and Sync
//! like everything else — nothing here writes to `master.db`.
//!
//! Per `docs/lexicon/02-library.md §Manual Editing`.

use std::path::Path;

use changes::multi_edit::{plan, Edit, EditableTrack, FieldValue, TrackValues};

use crate::cache_db;
use crate::recipes::{column_for, recipe_fields};

/// Fields the editor offers.
///
/// The same vocabulary the recipes and CSV import use, for the same reason: a
/// field the applier's allowlist will not write would give the user a form
/// control whose value silently vanishes at sync time.
#[tauri::command]
pub fn multi_edit_fields() -> Vec<String> {
    recipe_fields()
}

#[derive(Debug, serde::Serialize)]
pub struct MultiEditForm {
    /// `(field, value)` — `Multiple` where the selection disagrees.
    pub fields: Vec<(String, FieldValue)>,
    /// How many tracks the form covers, so the UI can say so.
    pub track_count: usize,
}

fn editable(t: &decks_core::rekordbox_db::Track) -> EditableTrack {
    let mut values = TrackValues::new();
    let mut put = |k: &str, v: Option<String>| {
        if let Some(v) = v.filter(|s| !s.trim().is_empty()) {
            values.insert(k.to_string(), v);
        }
    };
    put("title", Some(t.title.clone()));
    put("artist", t.artist.clone());
    put("album", t.album.clone());
    put("genre", t.genre.clone());
    put("comment", t.comment.clone());
    put("key", t.musical_key.clone());
    // Integer BPM stays integer-looking, so saving an untouched form would be
    // a no-op even if the user tabbed through the field.
    put(
        "bpm",
        t.bpm.map(|b| {
            if b.fract().abs() < f64::EPSILON {
                format!("{b:.0}")
            } else {
                format!("{b}")
            }
        }),
    );
    put("rating", t.rating.map(|v| v.to_string()));
    put("year", t.release_year.map(|v| v.to_string()));
    put("playCount", t.dj_play_count.map(|v| v.to_string()));
    EditableTrack {
        id: t.id.clone(),
        title: t.title.clone(),
        values,
    }
}

fn load(library_path: &str, track_ids: &[String]) -> Result<Vec<EditableTrack>, String> {
    let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(library_path))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for id in track_ids {
        if let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? {
            out.push(editable(&track));
        }
    }
    Ok(out)
}

/// The form for a selection, with disagreeing fields collapsed.
#[tauri::command]
pub async fn multi_edit_form(
    library_path: String,
    track_ids: Vec<String>,
) -> Result<MultiEditForm, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tracks = load(&library_path, &track_ids)?;
        Ok(MultiEditForm {
            fields: changes::multi_edit::initial(&tracks, &recipe_fields()),
            track_count: tracks.len(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the fields the user touched.
///
/// Takes only the *edited* fields, never the whole form. Sending the form back
/// would mean a field showing `<multiple values>` had to be represented
/// somehow, and any representation of it is one save away from flattening the
/// selection.
#[tauri::command]
pub async fn multi_edit_apply(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    edits: Vec<Edit>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        for edit in &edits {
            if column_for(&edit.field).is_none() {
                return Err(format!("{} is not a field this can write", edit.field));
            }
        }
        let tracks = load(&library_path, &track_ids)?;
        let planned = plan(&tracks, &edits);

        let cache = cache_db(&app)?;
        let mut staged = Vec::new();
        for p in planned {
            let Some(column) = column_for(&p.field) else {
                continue;
            };
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackMetadataEdit,
                    target_id: Some(p.track_id),
                    field: Some(column.to_string()),
                    old_value: Some(match p.before {
                        Some(v) => serde_json::json!(v),
                        None => serde_json::Value::Null,
                    }),
                    new_value: Some(match p.after {
                        Some(v) => serde_json::json!(v),
                        None => serde_json::Value::Null,
                    }),
                    reason: Some("Manual edit".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn track() -> decks_core::rekordbox_db::Track {
        decks_core::rekordbox_db::Track {
            id: "t1".into(),
            title: "get lucky".into(),
            artist: Some("daft punk".into()),
            album: None,
            genre: None,
            musical_key: None,
            bpm: Some(116.0),
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: Some(2013),
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
    fn every_offered_field_maps_to_a_writable_column() {
        for field in multi_edit_fields() {
            assert!(column_for(&field).is_some(), "{field} has no column");
        }
    }

    #[test]
    fn absent_track_values_stay_absent_rather_than_becoming_empty_strings() {
        // Otherwise every empty field would look like an agreed empty value
        // *and* like a value the user might overwrite by tabbing through.
        let e = editable(&track());
        assert!(!e.values.contains_key("album"));
        assert!(!e.values.contains_key("comment"));
    }

    #[test]
    fn an_integer_bpm_reads_back_as_an_integer() {
        // A form showing "116" that saves "116.0" would stage a change on
        // every track in the selection for no reason.
        assert_eq!(editable(&track()).values.get("bpm").unwrap(), "116");
    }

    #[test]
    fn a_form_over_one_track_agrees_with_itself() {
        let tracks = vec![editable(&track())];
        let fields = changes::multi_edit::initial(&tracks, &recipe_fields());
        let genre = fields.iter().find(|(f, _)| f == "genre").unwrap();
        assert_eq!(genre.1, FieldValue::Same(None));
        let title = fields.iter().find(|(f, _)| f == "title").unwrap();
        assert_eq!(title.1, FieldValue::Same(Some("get lucky".into())));
    }
}
