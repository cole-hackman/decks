//! Backup and restore of `decks`'s own derived state.
//!
//! `WriteGuard` already takes a timestamped copy of `master.db` before the
//! first write of a session. That protects the *source*. It does nothing for
//! everything `decks` knows that Rekordbox does not — custom tags, the archive,
//! smartlists, staged changes, path mappings, watch folders. All of that lives
//! in the cache DB, and until now none of it could be moved to another machine
//! or recovered after a mistake.
//!
//! **A JSON document rather than a ZIP**, which is where this diverges from the
//! spec. Lexicon ZIPs because it bundles several files; this is one document,
//! and compressing a few hundred kilobytes of text buys nothing a user can
//! feel. What it buys instead is worth more: the backup is *inspectable*, and
//! it survives schema changes — restoring a copied SQLite file into a newer
//! schema is a gamble, while restoring named columns is not.
//!
//! **Analysis caches are excluded on purpose.** Waveform peaks, fingerprints
//! and audio features are derived from files that are still on disk and can be
//! regenerated; including them would multiply the backup's size by a hundred to
//! save the user some CPU time they can spend once.
//!
//! See `docs/lexicon/09-history-backup.md §Database Backup`.

use crate::migrations;
use anyhow::{bail, Context, Result};
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tables a backup carries.
///
/// Everything a user would be upset to lose and could not reconstruct.
/// Conversations are in for the same reason — they are written by hand.
pub const BACKED_UP_TABLES: &[&str] = &[
    "tag_categories",
    "tags",
    "track_tags",
    "archived_tracks",
    "incoming_reviewed",
    "incoming_watermark",
    "smartlists",
    "path_mappings",
    "quick_move_folders",
    "watch_folders",
    "watch_dismissed",
    "field_mapping_rules",
    "common_text_blocklist",
    "staged_changes",
    "conversations",
    "conversation_messages",
];

/// Derived caches, deliberately left out — regenerable from files on disk.
pub const EXCLUDED_TABLES: &[&str] = &["audio_features", "audio_fingerprints", "waveform_peaks"];

pub type Row = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheBackup {
    /// Cache schema version at export time. A backup from a *newer* schema is
    /// refused rather than partially restored.
    pub schema_version: u32,
    pub created_at: i64,
    /// Table name → rows. A table absent from the map is left untouched on
    /// restore, so an old backup does not silently wipe a newer feature.
    pub tables: BTreeMap<String, Vec<Row>>,
}

impl CacheBackup {
    /// Rows across every table, for a "this backup holds N things" summary.
    pub fn row_count(&self) -> usize {
        self.tables.values().map(Vec::len).sum()
    }
}

fn value_to_json(v: ValueRef<'_>) -> serde_json::Value {
    match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => serde_json::json!(i),
        ValueRef::Real(f) => serde_json::json!(f),
        ValueRef::Text(t) => serde_json::json!(String::from_utf8_lossy(t)),
        // Blobs are base64-free by design: nothing in BACKED_UP_TABLES stores
        // one, and silently lossy-converting a blob would be worse than saying
        // it is not supported.
        ValueRef::Blob(_) => serde_json::Value::Null,
    }
}

fn json_to_value(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::Integer)
            .or_else(|| n.as_f64().map(Value::Real))
            .unwrap_or(Value::Null),
        serde_json::Value::String(s) => Value::Text(s.clone()),
        // Nested JSON round-trips as its text form, which is how the columns
        // that hold JSON (`clauses_json`, `content_json`) are stored anyway.
        other => Value::Text(other.to_string()),
    }
}

fn columns_of(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn table_exists(conn: &rusqlite::Connection, table: &str) -> Result<bool> {
    Ok(!columns_of(conn, table)?.is_empty())
}

/// Read the whole of `decks`'s derived state.
pub fn export(conn: &rusqlite::Connection, now: i64) -> Result<CacheBackup> {
    let mut tables = BTreeMap::new();
    for table in BACKED_UP_TABLES {
        // A table the current schema does not have is skipped rather than
        // erroring: this must keep working across versions in both directions.
        if !table_exists(conn, table)? {
            continue;
        }
        let columns = columns_of(conn, table)?;
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table}"))?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let mut map = Row::new();
            for (i, name) in columns.iter().enumerate() {
                map.insert(name.clone(), value_to_json(row.get_ref(i)?));
            }
            out.push(map);
        }
        tables.insert((*table).to_string(), out);
    }
    Ok(CacheBackup {
        schema_version: migrations::current_version(conn)?,
        created_at: now,
        tables,
    })
}

/// What a restore did.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// `(table, rows restored)`.
    pub restored: Vec<(String, usize)>,
    /// Tables in the backup this schema does not have. Named rather than
    /// dropped, so a restore into an older build says what it could not carry.
    pub unknown_tables: Vec<String>,
    /// `(table, column)` pairs the backup had and this schema does not.
    pub dropped_columns: Vec<(String, String)>,
}

/// Replace the derived state with a backup's.
///
/// **This deletes what is there.** The spec flags that loudly and so does the
/// UI. It runs in one transaction, so a failure part-way leaves the cache as it
/// was rather than half-replaced — which would be the worst of both.
///
/// Only tables *present in the backup* are touched. An old backup restoring
/// into a newer build leaves the newer build's tables alone rather than
/// emptying features the backup never knew about.
pub fn restore(conn: &mut rusqlite::Connection, backup: &CacheBackup) -> Result<RestoreReport> {
    let current = migrations::current_version(conn)?;
    if backup.schema_version > current {
        bail!(
            "this backup was made by a newer version of decks (schema v{}, this build has v{}). \
             Update before restoring it.",
            backup.schema_version,
            current
        );
    }

    let mut report = RestoreReport::default();
    let tx = conn.transaction()?;

    for (table, rows) in &backup.tables {
        // Only tables this build knows: a backup naming something unknown is
        // reported, never executed as SQL.
        if !BACKED_UP_TABLES.contains(&table.as_str()) || !table_exists(&tx, table)? {
            report.unknown_tables.push(table.clone());
            continue;
        }
        let known: Vec<String> = columns_of(&tx, table)?;
        tx.execute(&format!("DELETE FROM {table}"), [])
            .with_context(|| format!("clearing {table}"))?;

        let mut written = 0usize;
        for row in rows {
            let cols: Vec<&String> = row.keys().filter(|c| known.contains(c)).collect();
            for missing in row.keys().filter(|c| !known.contains(c)) {
                let pair = (table.clone(), missing.clone());
                if !report.dropped_columns.contains(&pair) {
                    report.dropped_columns.push(pair);
                }
            }
            if cols.is_empty() {
                continue;
            }
            let placeholders = vec!["?"; cols.len()].join(", ");
            let names = cols
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let values: Vec<rusqlite::types::Value> =
                cols.iter().map(|c| json_to_value(&row[*c])).collect();
            tx.execute(
                &format!("INSERT INTO {table} ({names}) VALUES ({placeholders})"),
                rusqlite::params_from_iter(values.iter()),
            )
            .with_context(|| format!("restoring a row into {table}"))?;
            written += 1;
        }
        report.restored.push((table.clone(), written));
    }

    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CacheDb;

    fn db() -> CacheDb {
        CacheDb::open_in_memory().unwrap()
    }

    fn seed(db: &CacheDb) {
        let cat = db.create_tag_category("Vibe").unwrap();
        let tag = db.create_tag(&cat.id, "Peak Time").unwrap();
        db.add_track_tag("/lib.db", "t1", &tag.id).unwrap();
        db.archive_tracks("/lib.db", &["t2".to_string()]).unwrap();
        db.add_common_text_pattern("(Original Mix)").unwrap();
    }

    #[test]
    fn a_backup_carries_the_state_a_user_cannot_reconstruct() {
        let source = db();
        seed(&source);
        let backup = export(&source.conn, 100).unwrap();
        assert_eq!(backup.tables["tags"].len(), 1);
        assert_eq!(backup.tables["track_tags"].len(), 1);
        assert_eq!(backup.tables["archived_tracks"].len(), 1);
        assert_eq!(backup.tables["common_text_blocklist"].len(), 1);
    }

    #[test]
    fn analysis_caches_are_left_out() {
        // They are derived from files still on disk, and including them would
        // multiply the backup's size to save CPU the user spends once.
        let backup = export(&db().conn, 100).unwrap();
        for excluded in EXCLUDED_TABLES {
            assert!(!backup.tables.contains_key(*excluded), "{excluded}");
        }
    }

    #[test]
    fn a_backup_restores_onto_a_fresh_database() {
        let source = db();
        seed(&source);
        let backup = export(&source.conn, 100).unwrap();

        let mut target = db();
        let report = restore(&mut target.conn, &backup).unwrap();
        assert!(report.unknown_tables.is_empty());

        let tags = target.list_tags(None).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Peak Time");
        assert_eq!(target.list_archived("/lib.db").unwrap(), vec!["t2"]);
        assert_eq!(
            target.list_common_text_patterns().unwrap(),
            vec!["(Original Mix)"]
        );
    }

    #[test]
    fn restoring_replaces_rather_than_merges() {
        // The spec says restoring deletes what is there, and a merge would be
        // worse: the user would get their old library plus whatever the
        // machine already held, with no way to tell which was which.
        let source = db();
        seed(&source);
        let backup = export(&source.conn, 100).unwrap();

        let mut target = db();
        let cat = target.create_tag_category("Other").unwrap();
        target.create_tag(&cat.id, "Should Not Survive").unwrap();

        restore(&mut target.conn, &backup).unwrap();
        let names: Vec<String> = target
            .list_tags(None)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["Peak Time"]);
    }

    #[test]
    fn a_backup_is_idempotent_when_restored_twice() {
        let source = db();
        seed(&source);
        let backup = export(&source.conn, 100).unwrap();

        let mut target = db();
        restore(&mut target.conn, &backup).unwrap();
        restore(&mut target.conn, &backup).unwrap();
        assert_eq!(target.list_tags(None).unwrap().len(), 1);
    }

    #[test]
    fn a_backup_from_a_newer_build_is_refused_not_half_applied() {
        let mut target = db();
        let backup = CacheBackup {
            schema_version: 9_999,
            created_at: 0,
            tables: BTreeMap::new(),
        };
        let err = restore(&mut target.conn, &backup).unwrap_err().to_string();
        assert!(err.contains("newer version"), "{err}");
    }

    #[test]
    fn a_table_this_build_does_not_know_is_reported_not_executed() {
        // The table name reaches a format! string, so anything unrecognised
        // must never get that far.
        let mut target = db();
        let mut tables = BTreeMap::new();
        tables.insert("tags; DROP TABLE tags".to_string(), Vec::new());
        let backup = CacheBackup {
            schema_version: 1,
            created_at: 0,
            tables,
        };
        let report = restore(&mut target.conn, &backup).unwrap();
        assert_eq!(report.unknown_tables.len(), 1);
        // The real table is still there.
        assert!(target.list_tags(None).is_ok());
    }

    #[test]
    fn a_table_missing_from_the_backup_is_left_alone() {
        // An old backup must not empty a feature it never knew about.
        let mut target = db();
        target.add_common_text_pattern("keep me").unwrap();
        let backup = CacheBackup {
            schema_version: 1,
            created_at: 0,
            tables: BTreeMap::new(),
        };
        restore(&mut target.conn, &backup).unwrap();
        assert_eq!(target.list_common_text_patterns().unwrap(), vec!["keep me"]);
    }

    #[test]
    fn a_column_this_build_does_not_have_is_dropped_and_named() {
        let mut target = db();
        let mut tables = BTreeMap::new();
        tables.insert(
            "common_text_blocklist".to_string(),
            vec![Row::from([
                ("pattern".to_string(), serde_json::json!("(Original Mix)")),
                ("future_column".to_string(), serde_json::json!(1)),
            ])],
        );
        let backup = CacheBackup {
            schema_version: 1,
            created_at: 0,
            tables,
        };
        let report = restore(&mut target.conn, &backup).unwrap();
        assert_eq!(
            report.dropped_columns,
            vec![(
                "common_text_blocklist".to_string(),
                "future_column".to_string()
            )]
        );
        assert_eq!(
            target.list_common_text_patterns().unwrap(),
            vec!["(Original Mix)"]
        );
    }

    #[test]
    fn a_backup_round_trips_through_json() {
        let source = db();
        seed(&source);
        let backup = export(&source.conn, 100).unwrap();
        let json = serde_json::to_string(&backup).unwrap();
        let parsed: CacheBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.row_count(), backup.row_count());

        let mut target = db();
        restore(&mut target.conn, &parsed).unwrap();
        assert_eq!(target.list_tags(None).unwrap().len(), 1);
    }
}
