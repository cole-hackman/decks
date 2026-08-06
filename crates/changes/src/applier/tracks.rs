use crate::applier::{KeyFormat, SyncOptions};
use crate::{key_format, StagedChange};
use anyhow::{anyhow, bail};
use rusqlite::{params, Transaction};
use serde_json::Value;

/// Columns on `djmdContent` writable via `TrackMetadataEdit`.
/// FK-shaped fields (Artist/Genre/Album/Key/Label) are intercepted before
/// the direct-column path; everything else here is a plain scalar column.
const ALLOWED_CONTENT_FIELDS: &[&str] = &[
    "Title",
    "Commnt",
    "Rating",
    "BPM",
    "ReleaseYear",
    "DJPlayCount",
    "Artist",
    "Genre",
    "Album",
    "Key",
    "Label",
];

pub(super) fn apply_metadata_edit(
    tx: &Transaction,
    change: &StagedChange,
    options: &SyncOptions,
    warnings: &mut Vec<String>,
) -> anyhow::Result<()> {
    let target_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let field = change
        .field
        .as_ref()
        .ok_or_else(|| anyhow!("Missing field"))?;
    let new_value = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;

    if !ALLOWED_CONTENT_FIELDS.contains(&field.as_str()) {
        bail!("Field {} is not in the allowlist", field);
    }

    // Honor SyncOptions.convert_keys for Key edits: rewrite the staged value
    // through the Camelot / Open-Key table. On parse failure we log + write
    // the original — failing the whole sync because one track has a typo'd
    // key is worse than letting it through unchanged.
    let converted_value;
    let new_value: &Value = if field == "Key" && options.convert_keys != KeyFormat::Original {
        if let Value::String(s) = new_value {
            let converted = match options.convert_keys {
                KeyFormat::Camelot => key_format::to_camelot(s),
                KeyFormat::OpenKey => key_format::to_open_key(s),
                KeyFormat::Original => unreachable!(),
            };
            match converted {
                Some(rewritten) => {
                    converted_value = Value::String(rewritten);
                    &converted_value
                }
                None => {
                    tracing::warn!(
                        original = %s,
                        format = ?options.convert_keys,
                        "key conversion failed; writing original value"
                    );
                    let format_label = match options.convert_keys {
                        KeyFormat::Camelot => "Camelot",
                        KeyFormat::OpenKey => "OpenKey",
                        KeyFormat::Original => unreachable!(),
                    };
                    warnings.push(format!(
                        "Failed to convert key for track {}: '{}' could not be mapped to {}",
                        target_id, s, format_label
                    ));
                    new_value
                }
            }
        } else {
            new_value
        }
    } else {
        new_value
    };

    match field.as_str() {
        "Artist" | "Genre" | "Album" | "Key" | "Label" => {
            apply_fk_edit(tx, target_id, field, new_value)
        }
        _ => apply_scalar_edit(tx, target_id, field, new_value),
    }
}

fn apply_fk_edit(
    tx: &Transaction,
    target_id: &str,
    field: &str,
    new_value: &Value,
) -> anyhow::Result<()> {
    let table = match field {
        "Artist" => "djmdArtist",
        "Genre" => "djmdGenre",
        "Album" => "djmdAlbum",
        "Key" => "djmdKey",
        "Label" => "djmdLabel",
        _ => unreachable!(),
    };
    let name_col = if field == "Key" { "ScaleName" } else { "Name" };
    let id_col = format!("{}ID", field);

    let fk_id = match new_value {
        Value::Null => None,
        Value::String(s) => Some(get_or_create_fk(tx, table, name_col, s)?),
        _ => bail!("FK fields must be string or null"),
    };

    let sql = format!("UPDATE djmdContent SET {} = ? WHERE ID = ?", id_col);
    let rows = match fk_id {
        Some(id) => tx.execute(&sql, params![id, target_id])?,
        None => tx.execute(&sql, params![rusqlite::types::Null, target_id])?,
    };
    if rows == 0 {
        bail!("No rows updated (target_id {} not found)", target_id);
    }
    Ok(())
}

fn apply_scalar_edit(
    tx: &Transaction,
    target_id: &str,
    field: &str,
    new_value: &Value,
) -> anyhow::Result<()> {
    let sql = format!("UPDATE djmdContent SET {} = ? WHERE ID = ?", field);
    let val = json_to_sql(new_value)?;
    let rows = tx.execute(&sql, params![val, target_id])?;
    if rows == 0 {
        bail!("No rows updated (target_id {} not found)", target_id);
    }
    Ok(())
}

/// `TrackDelete`:
/// - `target_id` = content (track) ID
///
/// Soft-delete: sets `rb_local_deleted = 1` rather than removing the row.
/// Rekordbox uses this flag to hide tracks; preserving the row keeps cue +
/// playlist references valid.
pub(super) fn apply_delete(tx: &Transaction, change: &crate::StagedChange) -> anyhow::Result<()> {
    let target_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let rows = tx.execute(
        "UPDATE djmdContent SET rb_local_deleted = 1 WHERE ID = ?",
        params![target_id],
    )?;
    if rows == 0 {
        bail!("No track soft-deleted (id {} not found)", target_id);
    }
    Ok(())
}

/// Columns that carry a track's filename alongside `FolderPath`.
///
/// Rekordbox 7 stores the full path in `FolderPath` and a denormalised copy of
/// the filename in `FileNameL` / `FileNameS`. Only `FolderPath` is modelled and
/// read by `decks`, so the filename columns are written **only if they exist**
/// — detected per-database rather than assumed. Writing a column that is not
/// there would fail the whole sync; leaving a stale filename behind would make
/// Rekordbox display the old name next to the new path.
const FILENAME_COLUMNS: &[&str] = &["FileNameL", "FileNameS"];

fn has_column(tx: &Transaction, table: &str, column: &str) -> anyhow::Result<bool> {
    let mut stmt = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `TrackRelocate`:
/// - `target_id` = content (track) ID
/// - `new_value` = `{ "folder_path": "…", "file_name": "…" }`
///
/// The move on disk has already happened — this is the database catching up.
pub(super) fn apply_relocate(tx: &Transaction, change: &StagedChange) -> anyhow::Result<()> {
    let target_id = change
        .target_id
        .as_ref()
        .ok_or_else(|| anyhow!("Missing target_id"))?;
    let new_value = change
        .new_value
        .as_ref()
        .ok_or_else(|| anyhow!("Missing new_value"))?;
    let folder_path = new_value
        .get("folder_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Missing folder_path"))?;
    let file_name = new_value.get("file_name").and_then(Value::as_str);

    let rows = tx.execute(
        "UPDATE djmdContent SET FolderPath = ? WHERE ID = ?",
        params![folder_path, target_id],
    )?;
    if rows == 0 {
        bail!("No rows updated (target_id {} not found)", target_id);
    }

    if let Some(file_name) = file_name {
        for column in FILENAME_COLUMNS {
            if has_column(tx, "djmdContent", column)? {
                tx.execute(
                    &format!("UPDATE djmdContent SET {column} = ? WHERE ID = ?"),
                    params![file_name, target_id],
                )?;
            }
        }
    }
    Ok(())
}

/// `TrackCreate` cannot be applied to `master.db`.
///
/// Not an oversight and not a stub: a `djmdContent` row needs columns this
/// repo does not model (`FileNameL`, `FileSize`, `ContentLink`, the analysis
/// pointers), and there is no real fixture to verify an INSERT against. A
/// half-populated row in a performing library is worse than no row. New tracks
/// reach Rekordbox through its own XML import, which the export already emits
/// them into — so the failure message points there rather than being generic.
pub(super) fn refuse_create(change: &StagedChange) -> anyhow::Result<()> {
    let what = change
        .new_value
        .as_ref()
        .and_then(|v| v.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("track");
    bail!(
        "{what} is a new track: sync cannot insert into master.db. \
         Export the XML and import it in Rekordbox instead."
    )
}

pub(super) fn json_to_sql(v: &Value) -> anyhow::Result<rusqlite::types::Value> {
    Ok(match v {
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                bail!("Unsupported number type");
            }
        }
        Value::Bool(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
        Value::Null => rusqlite::types::Value::Null,
        _ => bail!("Unsupported JSON type for SQL"),
    })
}

pub(super) fn get_or_create_fk(
    tx: &Transaction,
    table: &str,
    name_col: &str,
    value: &str,
) -> anyhow::Result<String> {
    let sql_select = format!(
        "SELECT ID FROM {} WHERE {} = ? COLLATE NOCASE",
        table, name_col
    );
    if let Ok(id) = tx.query_row(&sql_select, params![value], |r| r.get::<_, String>(0)) {
        return Ok(id);
    }
    let new_id = uuid::Uuid::new_v4().to_string();
    let sql_insert = format!("INSERT INTO {} (ID, {}) VALUES (?, ?)", table, name_col);
    tx.execute(&sql_insert, params![new_id, value])?;
    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeKind, ChangeStatus};
    use rusqlite::Connection;

    fn fixture() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE djmdContent (ID TEXT PRIMARY KEY, Title TEXT, Commnt TEXT, BPM REAL, GenreID TEXT, ArtistID TEXT, rb_local_deleted INTEGER DEFAULT 0);
             CREATE TABLE djmdGenre (ID TEXT PRIMARY KEY, Name TEXT);
             CREATE TABLE djmdArtist (ID TEXT PRIMARY KEY, Name TEXT);
             INSERT INTO djmdContent (ID, Title, BPM) VALUES ('t1', 'Old Title', 120.0);",
        )
        .unwrap();
        conn
    }

    fn change(field: &str, new_value: Value) -> StagedChange {
        StagedChange {
            id: "c".into(),
            library_path: None,
            kind: ChangeKind::TrackMetadataEdit,
            target_id: Some("t1".into()),
            field: Some(field.into()),
            old_value: None,
            new_value: Some(new_value),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn scalar_field_updates() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        apply_metadata_edit(
            &tx,
            &change("Title", Value::String("New".into())),
            &SyncOptions::default(),
            &mut Vec::new(),
        )
        .unwrap();
        let t: String = tx
            .query_row("SELECT Title FROM djmdContent WHERE ID='t1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(t, "New");
    }

    #[test]
    fn fk_field_creates_genre_row() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        apply_metadata_edit(
            &tx,
            &change("Genre", Value::String("Deep House".into())),
            &SyncOptions::default(),
            &mut Vec::new(),
        )
        .unwrap();
        let genre_id: String = tx
            .query_row("SELECT GenreID FROM djmdContent WHERE ID='t1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let name: String = tx
            .query_row(
                "SELECT Name FROM djmdGenre WHERE ID=?",
                params![genre_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Deep House");
    }

    #[test]
    fn fk_field_null_clears_link() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions::default();
        apply_metadata_edit(
            &tx,
            &change("Genre", Value::String("House".into())),
            &opts,
            &mut Vec::new(),
        )
        .unwrap();
        apply_metadata_edit(&tx, &change("Genre", Value::Null), &opts, &mut Vec::new()).unwrap();
        let g: Option<String> = tx
            .query_row("SELECT GenreID FROM djmdContent WHERE ID='t1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(g.is_none());
    }

    #[test]
    fn track_delete_sets_rb_local_deleted() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let mut change = change("Title", Value::Null);
        change.kind = crate::ChangeKind::TrackDelete;
        apply_delete(&tx, &change).unwrap();
        let flag: i64 = tx
            .query_row(
                "SELECT rb_local_deleted FROM djmdContent WHERE ID='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(flag, 1);
    }

    #[test]
    fn track_delete_errors_when_missing() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let mut change = change("Title", Value::Null);
        change.kind = crate::ChangeKind::TrackDelete;
        change.target_id = Some("ghost".into());
        let res = apply_delete(&tx, &change);
        assert!(res.is_err());
    }

    #[test]
    fn disallowed_field_errors() {
        let mut conn = fixture();
        let tx = conn.transaction().unwrap();
        let res = apply_metadata_edit(
            &tx,
            &change("rb_local_deleted", Value::Number(1.into())),
            &SyncOptions::default(),
            &mut Vec::new(),
        );
        assert!(res.is_err());
    }

    fn key_fixture() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE djmdContent (ID TEXT PRIMARY KEY, Title TEXT, KeyID TEXT);
             CREATE TABLE djmdKey (ID TEXT PRIMARY KEY, ScaleName TEXT);
             INSERT INTO djmdContent (ID, Title) VALUES ('t1', 'X');",
        )
        .unwrap();
        conn
    }

    fn key_scale_for(tx: &Transaction, track_id: &str) -> Option<String> {
        tx.query_row(
            "SELECT k.ScaleName FROM djmdContent c JOIN djmdKey k ON c.KeyID = k.ID WHERE c.ID = ?",
            params![track_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
    }

    #[test]
    fn key_conversion_camelot_rewrites_value() {
        let mut conn = key_fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            convert_keys: KeyFormat::Camelot,
            ..Default::default()
        };
        apply_metadata_edit(
            &tx,
            &change("Key", Value::String("C minor".into())),
            &opts,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(key_scale_for(&tx, "t1").as_deref(), Some("5A"));
    }

    #[test]
    fn key_conversion_open_key_rewrites_value() {
        let mut conn = key_fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            convert_keys: KeyFormat::OpenKey,
            ..Default::default()
        };
        apply_metadata_edit(
            &tx,
            &change("Key", Value::String("C minor".into())),
            &opts,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(key_scale_for(&tx, "t1").as_deref(), Some("5m"));
    }

    #[test]
    fn key_conversion_original_passthrough() {
        let mut conn = key_fixture();
        let tx = conn.transaction().unwrap();
        apply_metadata_edit(
            &tx,
            &change("Key", Value::String("C minor".into())),
            &SyncOptions::default(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(key_scale_for(&tx, "t1").as_deref(), Some("C minor"));
    }

    #[test]
    fn key_conversion_invalid_falls_back_to_original() {
        let mut conn = key_fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            convert_keys: KeyFormat::Camelot,
            ..Default::default()
        };
        let mut warnings = Vec::new();
        apply_metadata_edit(
            &tx,
            &change("Key", Value::String("Banana".into())),
            &opts,
            &mut warnings,
        )
        .unwrap();
        // Unparseable input → write the original string rather than fail.
        assert_eq!(key_scale_for(&tx, "t1").as_deref(), Some("Banana"));
        // …and emit a warning describing the failure so the UI can surface it.
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("Banana") && warnings[0].contains("Camelot"),
            "unexpected warning: {}",
            warnings[0]
        );
    }

    #[test]
    fn apply_with_options_surfaces_key_conversion_warning() {
        // End-to-end: a key change with an unconvertible value should land in
        // `ApplyResult.warnings` while still being counted as applied.
        let mut conn = key_fixture();
        let tx = conn.transaction().unwrap();
        let opts = SyncOptions {
            convert_keys: KeyFormat::Camelot,
            ..Default::default()
        };
        let change = change("Key", Value::String("C\u{266D} Major".into()));
        let res = crate::applier::apply_with_options(&tx, &[change], &opts).unwrap();
        assert_eq!(res.applied.len(), 1);
        assert!(res.failed.is_empty());
        assert_eq!(res.warnings.len(), 1);
        assert!(
            res.warnings[0].contains("C\u{266D} Major"),
            "unexpected warning: {}",
            res.warnings[0]
        );
        // And the original value lands in the DB.
        assert_eq!(key_scale_for(&tx, "t1").as_deref(), Some("C\u{266D} Major"));
    }

    fn relocate_change(new_value: Value) -> StagedChange {
        StagedChange {
            id: "c".into(),
            library_path: None,
            kind: ChangeKind::TrackRelocate,
            target_id: Some("t1".into()),
            field: None,
            old_value: None,
            new_value: Some(new_value),
            reason: None,
            confidence: None,
            status: ChangeStatus::Accepted,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn folder_path_for(tx: &Transaction, id: &str) -> Option<String> {
        tx.query_row(
            "SELECT FolderPath FROM djmdContent WHERE ID = ?",
            params![id],
            |r| r.get(0),
        )
        .ok()
    }

    #[test]
    fn relocate_updates_the_folder_path() {
        let mut conn = fixture();
        conn.execute_batch("ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;")
            .unwrap();
        let tx = conn.transaction().unwrap();
        apply_relocate(
            &tx,
            &relocate_change(serde_json::json!({
                "folder_path": "/Music/House/A - B.mp3",
                "file_name": "A - B.mp3",
            })),
        )
        .unwrap();
        assert_eq!(
            folder_path_for(&tx, "t1").as_deref(),
            Some("/Music/House/A - B.mp3")
        );
    }

    #[test]
    fn relocate_writes_the_filename_columns_when_the_database_has_them() {
        let mut conn = fixture();
        conn.execute_batch(
            "ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;
             ALTER TABLE djmdContent ADD COLUMN FileNameL TEXT;",
        )
        .unwrap();
        let tx = conn.transaction().unwrap();
        apply_relocate(
            &tx,
            &relocate_change(serde_json::json!({
                "folder_path": "/Music/A - B.mp3",
                "file_name": "A - B.mp3",
            })),
        )
        .unwrap();
        let name: String = tx
            .query_row(
                "SELECT FileNameL FROM djmdContent WHERE ID = 't1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "A - B.mp3");
    }

    #[test]
    fn relocate_succeeds_on_a_database_without_the_filename_columns() {
        // FileNameS is absent in this fixture; the relocate must not fail the
        // whole sync over a column we only opportunistically write.
        let mut conn = fixture();
        conn.execute_batch("ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;")
            .unwrap();
        let tx = conn.transaction().unwrap();
        assert!(apply_relocate(
            &tx,
            &relocate_change(serde_json::json!({
                "folder_path": "/Music/A - B.mp3",
                "file_name": "A - B.mp3",
            })),
        )
        .is_ok());
    }

    #[test]
    fn relocating_an_unknown_track_is_an_error_rather_than_a_silent_no_op() {
        let mut conn = fixture();
        conn.execute_batch("ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;")
            .unwrap();
        let tx = conn.transaction().unwrap();
        let mut change = relocate_change(serde_json::json!({ "folder_path": "/x.mp3" }));
        change.target_id = Some("missing".into());
        assert!(apply_relocate(&tx, &change).is_err());
    }

    #[test]
    fn relocate_requires_a_folder_path() {
        let mut conn = fixture();
        conn.execute_batch("ALTER TABLE djmdContent ADD COLUMN FolderPath TEXT;")
            .unwrap();
        let tx = conn.transaction().unwrap();
        assert!(apply_relocate(&tx, &relocate_change(serde_json::json!({}))).is_err());
    }
}
