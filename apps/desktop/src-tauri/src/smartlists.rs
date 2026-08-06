//! Tauri commands for smartlists (Epic 1).
//!
//! The rule model and evaluator live in `crates/smartlists`; this module owns
//! the IPC surface and the assembly of `EvalContext` from the several places
//! its inputs live (`master.db` for cues and playlists, the local cache for
//! tags and archived tracks, the filesystem for missing files).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use decks_core::rekordbox_db::{RekordboxDb, Track};
use smartlists::{
    evaluate, generate, only_missing, Clause, Combinator, EvalContext, GeneratorSpec, Smartlist,
    LEXICON_FOLDER,
};

use crate::{cache_db, hydrate_energy};

fn open_db(path: &str) -> Result<RekordboxDb, String> {
    RekordboxDb::open(Path::new(path)).map_err(|e| e.to_string())
}

/// Load every input the evaluator needs.
///
/// `missing_files` is the expensive one — it stats every track — so it is only
/// computed when some rule actually asks about it. Everything else is cheap
/// enough to load unconditionally.
fn build_context(
    app: &tauri::AppHandle,
    library_path: &str,
    db: &RekordboxDb,
    needs_missing_files: bool,
) -> Result<(Vec<Track>, EvalContext), String> {
    let mut tracks = db.tracks().map_err(|e| e.to_string())?;

    let mut ctx = EvalContext {
        tracks_with_cues: db
            .track_ids_with_cues()
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect(),
        tracks_in_any_playlist: db
            .track_ids_in_any_playlist()
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect(),
        ..Default::default()
    };

    if needs_missing_files {
        ctx.tracks_with_missing_files = tracks
            .iter()
            .filter(|t| {
                t.folder_path
                    .as_deref()
                    .map(|p| !Path::new(p).exists())
                    .unwrap_or(false)
            })
            .map(|t| t.id.clone())
            .collect();
    }

    // Cache lookups are best-effort: a missing or unreadable cache degrades to
    // "no tags, nothing archived, no energy" rather than failing the query.
    if let Ok(cache) = cache_db(app) {
        hydrate_energy(&mut tracks, &cache);
        if let Ok(ids) = cache.list_archived(library_path) {
            ctx.archived_tracks = ids.into_iter().collect();
        }
        if let Ok(map) = cache.list_track_tags_map(library_path) {
            ctx.tags_by_track = map
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().collect::<HashSet<String>>()))
                .collect();
        }
    }

    Ok((tracks, ctx))
}

fn mentions_missing_files(list: &Smartlist) -> bool {
    list.clauses
        .iter()
        .flat_map(|c| c.rules.iter())
        .any(|r| r.field == smartlists::Field::IsFileMissing)
}

#[tauri::command]
pub async fn list_smartlists(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<Vec<Smartlist>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.list_smartlists(&library_path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn create_smartlist(
    app: tauri::AppHandle,
    library_path: String,
    name: String,
    parent_folder_id: Option<String>,
    combinator: Combinator,
    clauses: Vec<Clause>,
) -> Result<Smartlist, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.create_smartlist(
            &library_path,
            &name,
            parent_folder_id.as_deref(),
            combinator,
            &clauses,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_smartlist(
    app: tauri::AppHandle,
    library_path: String,
    id: String,
    name: String,
    parent_folder_id: Option<String>,
    combinator: Combinator,
    clauses: Vec<Clause>,
) -> Result<Smartlist, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.update_smartlist(
            &library_path,
            &id,
            &name,
            parent_folder_id.as_deref(),
            combinator,
            &clauses,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_smartlist(
    app: tauri::AppHandle,
    library_path: String,
    id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        db.delete_smartlist(&library_path, &id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Evaluate a stored smartlist and return the matching tracks.
#[tauri::command]
pub async fn evaluate_smartlist(
    app: tauri::AppHandle,
    library_path: String,
    id: String,
) -> Result<Vec<Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let list = cache
            .get_smartlist(&library_path, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("smartlist {id} not found"))?;
        let db = open_db(&library_path)?;
        let (tracks, ctx) = build_context(&app, &library_path, &db, mentions_missing_files(&list))?;
        let ids: HashSet<String> = evaluate(&list, &tracks, &ctx).into_iter().collect();
        Ok(tracks.into_iter().filter(|t| ids.contains(&t.id)).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Evaluate an unsaved smartlist. Powers the editor's live match count, so the
/// user sees what a rule set selects before committing to it.
#[tauri::command]
pub async fn preview_smartlist(
    app: tauri::AppHandle,
    library_path: String,
    combinator: Combinator,
    clauses: Vec<Clause>,
) -> Result<Vec<Track>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let list = Smartlist {
            id: "__preview__".into(),
            name: "preview".into(),
            parent_folder_id: None,
            combinator,
            clauses,
            created_at: 0,
            updated_at: 0,
        };
        list.validate()?;
        let db = open_db(&library_path)?;
        let (tracks, ctx) = build_context(&app, &library_path, &db, mentions_missing_files(&list))?;
        let ids: HashSet<String> = evaluate(&list, &tracks, &ctx).into_iter().collect();
        Ok(tracks.into_iter().filter(|t| ids.contains(&t.id)).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Run the Smartlist Generator, creating only the smartlists that do not yet
/// exist in the reserved `Lexicon` folder. Safe to re-run.
#[tauri::command]
pub async fn generate_smartlists(
    app: tauri::AppHandle,
    library_path: String,
    spec: GeneratorSpec,
) -> Result<Vec<Smartlist>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = open_db(&library_path)?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        let cache = cache_db(&app)?;

        let existing = cache
            .list_generated_smartlists(&library_path)
            .map_err(|e| e.to_string())?;
        let wanted = only_missing(generate(&spec, &tracks), &existing);

        let mut created = Vec::new();
        for g in wanted {
            let list = cache
                .create_smartlist(
                    &library_path,
                    &g.name,
                    Some(LEXICON_FOLDER),
                    g.combinator,
                    &g.clauses,
                )
                .map_err(|e| e.to_string())?;
            created.push(list);
        }
        Ok(created)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Match counts for every smartlist in one pass.
///
/// The playlist tree needs a count per row; doing this as one command means the
/// library and its derived sets are loaded once rather than once per smartlist.
#[tauri::command]
pub async fn smartlist_counts(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<HashMap<String, usize>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let lists = cache
            .list_smartlists(&library_path)
            .map_err(|e| e.to_string())?;
        if lists.is_empty() {
            return Ok(HashMap::new());
        }
        let db = open_db(&library_path)?;
        let needs_missing = lists.iter().any(mentions_missing_files);
        let (tracks, ctx) = build_context(&app, &library_path, &db, needs_missing)?;
        Ok(lists
            .into_iter()
            .map(|l| {
                let n = evaluate(&l, &tracks, &ctx).len();
                (l.id, n)
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage `PlaylistCreate` + `PlaylistAddTrack` changes for every smartlist, so
/// they land in Rekordbox as plain playlists. This is what
/// `SyncOptions.all_smartlists_to_playlists` turns on.
///
/// Smartlists named `Excluded From Sync…`, and tracks carrying a custom tag of
/// that name, are skipped. Returns the staged change IDs.
pub fn stage_materialization(
    app: &tauri::AppHandle,
    library_path: &str,
) -> Result<Vec<String>, String> {
    let cache = cache_db(app)?;
    let lists = cache
        .list_smartlists(library_path)
        .map_err(|e| e.to_string())?;
    if lists.is_empty() {
        return Ok(Vec::new());
    }

    let db = open_db(library_path)?;
    let needs_missing = lists.iter().any(mentions_missing_files);
    let (tracks, ctx) = build_context(app, library_path, &db, needs_missing)?;

    // Tracks carrying the `Excluded From Sync` custom tag never leave decks.
    let excluded_tracks: HashSet<String> = match cache.list_tags(None) {
        Ok(tags) => {
            let ids: HashSet<String> = tags
                .into_iter()
                .filter(|t| smartlists::is_exclusion_tag(&t.name))
                .map(|t| t.id)
                .collect();
            if ids.is_empty() {
                HashSet::new()
            } else {
                ctx.tags_by_track
                    .iter()
                    .filter(|(_, bound)| bound.iter().any(|t| ids.contains(t)))
                    .map(|(track_id, _)| track_id.clone())
                    .collect()
            }
        }
        Err(_) => HashSet::new(),
    };

    let mut staged = Vec::new();
    for list in &lists {
        if smartlists::is_excluded_by_name(&list.name) {
            continue;
        }
        let members: Vec<String> = evaluate(list, &tracks, &ctx)
            .into_iter()
            .filter(|id| !excluded_tracks.contains(id))
            .collect();

        // The playlist ID must be stable between preview and apply, so derive
        // it from the smartlist rather than generating a fresh UUID each run.
        let playlist_id = format!("smartlist-{}", list.id);
        for change in smartlists::materialize_changes(library_path, list, &playlist_id, &members) {
            let record = cache.stage_change(change).map_err(|e| e.to_string())?;
            staged.push(record.id);
        }
    }
    Ok(staged)
}

/// Rekordbox compatibility for each smartlist, for the editor's indicator.
#[tauri::command]
pub async fn smartlist_compatibility(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<HashMap<String, smartlists::Compatibility>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let lists = cache
            .list_smartlists(&library_path)
            .map_err(|e| e.to_string())?;
        // Map each tag id to its category so the 4-category MyTag cap can be
        // evaluated; degrade to zero categories if tags cannot be read.
        let tag_category: HashMap<String, String> = cache
            .list_tags(None)
            .map(|tags| tags.into_iter().map(|t| (t.id, t.category_id)).collect())
            .unwrap_or_default();

        Ok(lists
            .into_iter()
            .map(|l| {
                let categories: HashSet<&String> = l
                    .clauses
                    .iter()
                    .flat_map(|c| c.rules.iter())
                    .filter_map(|r| match &r.value {
                        smartlists::Value::Tags(ids) => Some(ids),
                        _ => None,
                    })
                    .flatten()
                    .filter_map(|id| tag_category.get(id))
                    .collect();
                let compat = smartlists::rekordbox_compatibility(&l, categories.len());
                (l.id, compat)
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Browser search ───────────────────────────────────────────────────────────

/// Run a track-browser search that uses operator syntax.
///
/// Parses with `smartlists::search` and evaluates with the **same** evaluator
/// smartlists use, which is the point: `bpm > 128`, notation-aware key
/// equality and tag semantics have one implementation, not two that drift.
///
/// Returns matching track ids; the renderer filters the list it already has.
/// A plain-text query never reaches here — the browser keeps its instant local
/// substring match for those, so typing a band's name does not wait on IPC.
#[tauri::command]
pub async fn search_tracks(
    app: tauri::AppHandle,
    path: String,
    query: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let list = smartlists::parse_search(&query);
        // An unparseable search shows the library rather than an empty screen
        // with no explanation.
        if list.clauses.is_empty() {
            return Ok(Vec::new());
        }
        let db = open_db(&path)?;
        let (tracks, ctx) = build_context(&app, &path, &db, mentions_missing_files(&list))?;
        Ok(evaluate(&list, &tracks, &ctx))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether a query needs the engine at all.
///
/// Exposed so the renderer makes the same call the parser would, rather than
/// carrying its own idea of what counts as syntax.
#[tauri::command]
pub fn search_has_operators(query: String) -> Result<bool, String> {
    Ok(smartlists::has_operators(&query))
}
