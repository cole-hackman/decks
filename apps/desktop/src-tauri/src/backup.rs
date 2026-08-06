//! Tauri commands for Database Backup (Epic 5).
//!
//! `WriteGuard` already protects `master.db`. This protects everything `decks`
//! knows that Rekordbox does not — custom tags, the archive, smartlists, staged
//! changes, path mappings, watch folders — none of which existed anywhere but
//! the local cache.
//!
//! Per `docs/lexicon/09-history-backup.md §Database Backup`.

use cache::backup::{CacheBackup, RestoreReport};

use crate::cache_db;

#[derive(Debug, serde::Serialize)]
pub struct BackupSummary {
    pub path: String,
    pub rows: usize,
    /// `(table, rows)`, so the user can see what they got rather than a size.
    pub tables: Vec<(String, usize)>,
}

/// Write a backup of the cache's derived state.
#[tauri::command]
pub async fn create_backup(app: tauri::AppHandle, path: String) -> Result<BackupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let doc = cache.export_backup(now_secs()).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("could not write {path}: {e}"))?;
        Ok(BackupSummary {
            rows: doc.row_count(),
            tables: doc
                .tables
                .iter()
                .filter(|(_, rows)| !rows.is_empty())
                .map(|(t, rows)| (t.clone(), rows.len()))
                .collect(),
            path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What a backup file holds, without restoring it.
///
/// The restore is destructive, so the user gets to see what they are about to
/// swap in first. Reading the file is also where a wrong file — a JSON that is
/// not a backup at all — is caught, rather than half-way through a wipe.
#[tauri::command]
pub async fn inspect_backup(path: String) -> Result<BackupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let doc = read(&path)?;
        Ok(BackupSummary {
            rows: doc.row_count(),
            tables: doc
                .tables
                .iter()
                .filter(|(_, rows)| !rows.is_empty())
                .map(|(t, rows)| (t.clone(), rows.len()))
                .collect(),
            path,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Replace the cache's derived state with a backup's.
///
/// **This deletes what is there.** The dialog says so; so does the spec.
#[tauri::command]
pub async fn restore_backup(app: tauri::AppHandle, path: String) -> Result<RestoreReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let doc = read(&path)?;
        let mut cache = cache_db(&app)?;
        cache.restore_backup(&doc).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn read(path: &str) -> Result<CacheBackup, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path} is not a decks backup: {e}"))
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
    use std::io::Write;

    fn write_temp(name: &str, contents: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(contents.as_bytes())
            .unwrap();
        (dir, path.to_string_lossy().into_owned())
    }

    #[test]
    fn a_file_that_is_not_a_backup_is_rejected_by_name() {
        // Caught on read, before anything is deleted.
        let (_dir, path) = write_temp("notes.json", r#"{"hello":"world"}"#);
        let err = read(&path).unwrap_err();
        assert!(err.contains("is not a decks backup"), "{err}");
    }

    #[test]
    fn a_missing_file_says_which_one() {
        let err = read("/nope/backup.json").unwrap_err();
        assert!(err.contains("could not read /nope/backup.json"), "{err}");
    }

    #[test]
    fn a_real_backup_reads_back() {
        let doc = CacheBackup {
            schema_version: 1,
            created_at: 0,
            tables: Default::default(),
        };
        let (_dir, path) = write_temp("b.json", &serde_json::to_string(&doc).unwrap());
        assert_eq!(read(&path).unwrap().schema_version, 1);
    }

    #[test]
    fn every_backed_up_table_is_a_real_table_name() {
        // The names reach a format! string on restore, so the allowlist has to
        // be exactly the set the schema defines.
        let db = cache::CacheDb::open_in_memory().unwrap();
        let doc = db.export_backup(0).unwrap();
        for table in doc.tables.keys() {
            assert!(
                cache::backup::BACKED_UP_TABLES.contains(&table.as_str()),
                "{table} exported but not on the allowlist"
            );
        }
    }
}
