//! Tauri commands for Undo History (Epic 5).
//!
//! `decks` gates hard *before* a write: every change is reviewed, Sync is
//! opt-in, and `WriteGuard` takes a timestamped backup. What it had no answer
//! for is the change you accept and then regret — restoring the backup throws
//! away everything else in the session too.
//!
//! An undo stages the *inverse* of what a Sync run applied, as ordinary
//! proposed changes. So an undo is reviewed before it is written, goes through
//! the same guarded Sync, and there is no second write path into `master.db`.
//! The trade is that an undo is two steps rather than one, which is the right
//! trade for a program whose first rule is that the library is read-only.
//!
//! The staging itself lives in `cache::CacheDb::stage_undo_run` so the chat
//! panel, the MCP server and the CLI share one implementation of the
//! once-only guard.
//!
//! Per `docs/lexicon/10-recipes.md §Undo History`.

use crate::cache_db;

/// Sync runs for a library, newest first.
#[tauri::command]
pub async fn list_undo_runs(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<cache::store::UndoRun>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .list_undo_runs(&library_path)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What one run did, and what an undo would and would not put back.
#[tauri::command]
pub async fn undo_run_entries(
    app: tauri::AppHandle,
    run_id: String,
) -> Result<Vec<cache::store::UndoEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .undo_run_entries(&run_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage a run's inverses for review.
#[tauri::command]
pub async fn undo_run(
    app: tauri::AppHandle,
    library_path: String,
    run_id: String,
) -> Result<cache::store::StagedUndo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cache_db(&app)?
            .stage_undo_run(&library_path, &run_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
