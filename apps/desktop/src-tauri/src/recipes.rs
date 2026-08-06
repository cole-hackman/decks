//! Tauri commands for Recipes (Epic 5).
//!
//! Preview-then-apply, matching how Smart Fixes already works: `recipe_preview`
//! shows every proposed change as a reviewable row, `recipe_apply` stages the
//! ones the user kept. Nothing is written to `master.db` here — recipe results
//! become `TrackMetadataEdit` changes and go through Sync like everything else.
//!
//! Per `docs/lexicon/10-recipes.md`.

use std::path::Path;

use ::recipes::{apply_all, apply_tag_recipe, FieldChange, Recipe, TagRecipe, TrackFields};
use serde::{Deserialize, Serialize};

use crate::cache_db;

/// Fields a recipe may read and write.
///
/// Deliberately the intersection of what `decks` models on a track and what the
/// applier's allowlist will actually write. Offering a recipe a field that
/// cannot be persisted would produce a preview full of changes that silently
/// vanish at sync time.
const RECIPE_FIELDS: &[&str] = &[
    "title",
    "artist",
    "album",
    "genre",
    "comment",
    "key",
    "bpm",
    "rating",
    "year",
    "playCount",
];

/// Map a recipe field name to the `djmdContent` column the applier expects.
pub(crate) fn column_for(field: &str) -> Option<&'static str> {
    Some(match field {
        "title" => "Title",
        "artist" => "Artist",
        "album" => "Album",
        "genre" => "Genre",
        "comment" => "Commnt",
        "key" => "Key",
        "bpm" => "BPM",
        "rating" => "Rating",
        "year" => "ReleaseYear",
        "playCount" => "DJPlayCount",
        _ => return None,
    })
}

/// The field vocabulary, for the recipe builder.
#[tauri::command]
pub fn recipe_fields() -> Vec<String> {
    RECIPE_FIELDS.iter().map(|s| s.to_string()).collect()
}

fn track_to_fields(t: &decks_core::rekordbox_db::Track) -> TrackFields {
    let mut f = TrackFields::new();
    let mut put = |k: &str, v: Option<String>| {
        if let Some(v) = v {
            f.set(k, v);
        }
    };
    put("title", Some(t.title.clone()));
    put("artist", t.artist.clone());
    put("album", t.album.clone());
    put("genre", t.genre.clone());
    put("comment", t.comment.clone());
    put("key", t.musical_key.clone());
    // BPM keeps its decimal only when it has one, so `AdjustNumber` round-trips
    // an integer BPM as an integer.
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
    f
}

/// One proposed change, as the review table shows it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeProposal {
    /// Stable within a preview, so the UI can deselect rows by id.
    pub id: String,
    pub track_id: String,
    pub track_title: String,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct RecipePreview {
    pub proposals: Vec<RecipeProposal>,
    /// `(track_id, reason)` for tracks a recipe could not act on. Surfaced so
    /// "340 of 400 changed" has an explanation attached rather than being a
    /// mystery.
    pub skipped: Vec<(String, String)>,
}

fn describe(skipped: &::recipes::Skipped) -> String {
    match skipped {
        ::recipes::Skipped::SourceEmpty { field } => format!("{field} is empty"),
        ::recipes::Skipped::NoMatch { field } => format!("no match in {field}"),
        ::recipes::Skipped::NotANumber { field, value } => {
            format!("{field} is not a number: {value:?}")
        }
        ::recipes::Skipped::Misconfigured { detail } => detail.clone(),
    }
}

fn proposals_for(
    track: &decks_core::rekordbox_db::Track,
    changes: &[FieldChange],
) -> Vec<RecipeProposal> {
    changes
        .iter()
        .filter(|c| column_for(&c.field).is_some())
        .map(|c| RecipeProposal {
            id: format!("{}:{}", track.id, c.field),
            track_id: track.id.clone(),
            track_title: track.title.clone(),
            field: c.field.clone(),
            before: c.before.clone(),
            after: c.after.clone(),
        })
        .collect()
}

/// Run recipes over a selection without changing anything.
#[tauri::command]
pub async fn recipe_preview(
    library_path: String,
    track_ids: Vec<String>,
    recipes: Vec<Recipe>,
) -> Result<RecipePreview, String> {
    if recipes.is_empty() {
        return Err("add at least one recipe".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut out = RecipePreview::default();
        for id in &track_ids {
            let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? else {
                continue;
            };
            let before = track_to_fields(&track);
            let (after, outcomes) = apply_all(&recipes, &before);

            for outcome in &outcomes {
                if let Some(reason) = &outcome.skipped {
                    out.skipped.push((track.id.clone(), describe(reason)));
                }
            }
            out.proposals
                .extend(proposals_for(&track, &::recipes::diff(&before, &after)));
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the proposals the user kept.
///
/// Takes the reviewed proposals back rather than re-running the recipes, so
/// what is staged is exactly what was on screen.
#[tauri::command]
pub async fn recipe_apply(
    app: tauri::AppHandle,
    library_path: String,
    proposals: Vec<RecipeProposal>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut staged = Vec::new();
        for p in proposals {
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
                    reason: Some("Recipe".to_string()),
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

// ── "Other" recipes ──────────────────────────────────────────────────────────

/// The three operations from the spec's "Other" category.
///
/// They share nothing with the field and tag recipes except the selection they
/// run over — each reaches into a different subsystem — so they are commands
/// rather than another arm of the pure engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OtherRecipe {
    /// Push tracks back onto Incoming, using it as a to-do list.
    MarkAsIncoming,
    /// Strip a track from every playlist. Deliberately does **not** touch
    /// smartlists, which are derived and would simply re-add the track.
    RemoveFromAllPlaylists,
    /// Take the file's modification time as the track's year.
    ImportDateFromFilesystem,
}

#[derive(Debug, Default, Serialize)]
pub struct OtherRecipeResult {
    /// Tracks the recipe acted on.
    pub changed: Vec<String>,
    /// Staged change ids, where the recipe stages rather than applies.
    pub staged: Vec<String>,
    /// `(track_id, reason)` for tracks it could not act on.
    pub skipped: Vec<(String, String)>,
}

/// The year a file's modification time falls in.
///
/// Modification rather than creation time: creation time is not portable
/// (Linux has no reliable `birthtime`), and a file copied between drives keeps
/// its mtime while its ctime becomes the copy date — which would be worse than
/// useless as a release year.
fn year_from_file(path: &std::path::Path) -> Option<i32> {
    use chrono::Datelike;
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let dt = chrono::DateTime::from_timestamp(secs as i64, 0)?;
    Some(dt.year())
}

#[tauri::command]
pub async fn other_recipe_apply(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    recipe: OtherRecipe,
) -> Result<OtherRecipeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let mut result = OtherRecipeResult::default();

        match recipe {
            OtherRecipe::MarkAsIncoming => {
                // The inverse of Selected done: clear the per-track reviewed
                // flag so the track surfaces on Incoming again.
                let cleared = cache
                    .unmark_incoming_reviewed(&library_path, &track_ids)
                    .map_err(|e| e.to_string())?;
                if cleared > 0 {
                    result.changed = track_ids;
                }
            }

            OtherRecipe::RemoveFromAllPlaylists => {
                let playlists = db.playlists().map_err(|e| e.to_string())?;
                for playlist in playlists
                    .iter()
                    .filter(|p| matches!(p.kind, decks_core::rekordbox_db::PlaylistKind::Playlist))
                {
                    let entries = db
                        .playlist_entries(&playlist.id)
                        .map_err(|e| e.to_string())?;
                    for entry in entries.iter().filter(|e| track_ids.contains(&e.content_id)) {
                        let record = cache
                            .stage_change(changes::NewChange {
                                library_path: Some(library_path.clone()),
                                kind: changes::ChangeKind::PlaylistRemoveTrack,
                                target_id: Some(playlist.id.clone()),
                                field: None,
                                old_value: None,
                                new_value: Some(serde_json::json!(entry.content_id)),
                                reason: Some("Recipe — remove from all playlists".to_string()),
                                confidence: Some(1.0),
                            })
                            .map_err(|e| e.to_string())?;
                        result.staged.push(record.id);
                        if !result.changed.contains(&entry.content_id) {
                            result.changed.push(entry.content_id.clone());
                        }
                    }
                }
            }

            OtherRecipe::ImportDateFromFilesystem => {
                for id in &track_ids {
                    let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? else {
                        continue;
                    };
                    let Some(path) = track.folder_path.as_deref() else {
                        result
                            .skipped
                            .push((id.clone(), "track has no file path".into()));
                        continue;
                    };
                    let Some(year) = year_from_file(Path::new(path)) else {
                        result
                            .skipped
                            .push((id.clone(), "file not readable".into()));
                        continue;
                    };
                    if track.release_year == Some(year as i64) {
                        continue;
                    }
                    let record = cache
                        .stage_change(changes::NewChange {
                            library_path: Some(library_path.clone()),
                            kind: changes::ChangeKind::TrackMetadataEdit,
                            target_id: Some(id.clone()),
                            field: Some("ReleaseYear".to_string()),
                            old_value: Some(match track.release_year {
                                Some(y) => serde_json::json!(y),
                                None => serde_json::Value::Null,
                            }),
                            new_value: Some(serde_json::json!(year)),
                            reason: Some("Recipe — import date from filesystem".to_string()),
                            confidence: Some(1.0),
                        })
                        .map_err(|e| e.to_string())?;
                    result.staged.push(record.id);
                    result.changed.push(id.clone());
                }
            }
        }

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Cue recipes ──────────────────────────────────────────────────────────────

/// Rekordbox holds eight hot-cue slots; a sort can only reassign that many.
const HOT_CUE_SLOTS: usize = 8;

/// One field of one cue, before and after.
///
/// Values are JSON rather than strings so the staged change carries the right
/// SQL type — `InMsec` has to reach `djmdCue` as an integer, not `"1000"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueChange {
    pub cue_id: String,
    /// For the preview row; not staged.
    pub cue_label: String,
    /// `djmdCue` column.
    pub field: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueDeletion {
    pub cue_id: String,
    pub cue_label: String,
    /// The whole cue, in the shape `apply_add_cue` inserts.
    ///
    /// Recorded so the deletion can be undone: a `TrackDeleteCue` with no
    /// snapshot is gone for good, and `changes::undo` refuses to invert one.
    /// The restored cue gets a new row id — same position, name and colour.
    pub snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueRecipeTrack {
    pub track_id: String,
    pub track_title: String,
    pub edits: Vec<CueChange>,
    pub deletions: Vec<CueDeletion>,
    /// Why the recipe did nothing on this track, when it did nothing.
    pub skipped: Option<String>,
}

fn cue_label(cue: &::recipes::RecipeCue) -> String {
    let time = format!(
        "{}:{:02}",
        cue.position_ms / 60_000,
        (cue.position_ms % 60_000) / 1000
    );
    match cue.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => format!("{time} {name}"),
        None => time,
    }
}

/// A track's cues in the shape the engine wants, plus the hot-cue slot each one
/// currently occupies.
///
/// The slots come back separately because they are not part of the recipe
/// model — the engine works in track order, and only a sort turns that order
/// back into slot numbers.
///
/// A colour of -1 is Rekordbox's "unset", which the engine models as `None` so
/// that "cues without a colour" means what a user would expect.
type CueSlots = std::collections::HashMap<String, i64>;

fn recipe_cues(
    db: &decks_core::rekordbox_db::RekordboxDb,
    track_id: &str,
) -> Result<(Vec<::recipes::RecipeCue>, CueSlots), String> {
    use decks_core::rekordbox_db::CueKind;

    let mut cues = Vec::new();
    let mut slots = CueSlots::new();
    for c in db.hot_cues_for_track(track_id).map_err(|e| e.to_string())? {
        let Some(position_ms) = c.in_msec else {
            continue;
        };
        if let CueKind::HotCue(slot) = c.kind {
            slots.insert(c.id.clone(), slot as i64);
        }
        cues.push(::recipes::RecipeCue {
            id: c.id,
            position_ms,
            loop_end_ms: c.out_msec.filter(|v| *v > 0),
            name: c.comment,
            color: c.color.filter(|v| *v >= 0),
            memory: matches!(c.kind, CueKind::MemoryCue),
        });
    }
    Ok((cues, slots))
}

/// The beat grid, or an empty grid when the track has no analysis file.
///
/// Empty rather than an error: only `QuantizeCues` needs it, and it reports the
/// absence itself rather than every other recipe failing on an unanalysed track.
fn beat_grid(library_path: &str, track: &decks_core::rekordbox_db::Track) -> Vec<i64> {
    use decks_core::rekordbox_db::anlz;
    let lib_dir = Path::new(library_path).parent().unwrap_or(Path::new(""));
    track
        .analysis_data_path
        .as_deref()
        .and_then(|p| anlz::resolve_anlz_path(lib_dir, p))
        .map(|resolved| anlz::read_beat_grid(&resolved).unwrap_or_default())
        .unwrap_or_default()
        .into_iter()
        .map(|b| b.time_ms as i64)
        .collect()
}

fn colour_json(colour: Option<i64>) -> serde_json::Value {
    // Rekordbox stores "no colour" as -1, not NULL.
    serde_json::json!(colour.unwrap_or(-1))
}

/// Turn an engine result into staged-change shaped edits.
///
/// Ordering matters to the user, not to `djmdCue`: cues have no stored order,
/// only hot-cue slot numbers. So a sort becomes a slot reassignment over the
/// hot cues, and memory cues — which have no slot — are left where they are.
fn diff_cues(
    track_id: &str,
    before: &[::recipes::RecipeCue],
    slots: &CueSlots,
    edits: &::recipes::CueEdits,
    reorders: bool,
) -> (Vec<CueChange>, Vec<CueDeletion>) {
    let by_id: std::collections::HashMap<&str, &::recipes::RecipeCue> =
        before.iter().map(|c| (c.id.as_str(), c)).collect();

    let mut changes = Vec::new();
    for (index, after) in edits.cues.iter().enumerate() {
        let Some(orig) = by_id.get(after.id.as_str()) else {
            continue;
        };
        let label = cue_label(orig);
        let mut push = |field: &str, b: serde_json::Value, a: serde_json::Value| {
            if b != a {
                changes.push(CueChange {
                    cue_id: after.id.clone(),
                    cue_label: label.clone(),
                    field: field.to_string(),
                    before: b,
                    after: a,
                });
            }
        };
        push(
            "InMsec",
            serde_json::json!(orig.position_ms),
            serde_json::json!(after.position_ms),
        );
        push(
            "OutMsec",
            serde_json::json!(orig.loop_end_ms),
            serde_json::json!(after.loop_end_ms),
        );
        push(
            "Commnt",
            serde_json::json!(orig.name),
            serde_json::json!(after.name),
        );
        push("Color", colour_json(orig.color), colour_json(after.color));

        if reorders && !after.memory && index < HOT_CUE_SLOTS {
            // Slot numbers run 1..=8 in the recipe's new order. Anything past
            // the eighth keeps its slot — there is nowhere else to put it.
            push(
                "Kind",
                serde_json::json!(slots.get(after.id.as_str()).copied()),
                serde_json::json!(index as i64 + 1),
            );
        }
    }

    let deletions = edits
        .deleted
        .iter()
        .map(|id| {
            let orig = by_id.get(id.as_str());
            CueDeletion {
                cue_id: id.clone(),
                cue_label: orig.map(|c| cue_label(c)).unwrap_or_default(),
                snapshot: match orig {
                    Some(c) => cue_snapshot(track_id, c, slots),
                    None => serde_json::Value::Null,
                },
            }
        })
        .collect();

    (changes, deletions)
}

/// A deleted cue, in the shape `apply_add_cue` inserts.
///
/// `content_id` rides along because a delete targets the cue while an add
/// targets the track — without it the inverse would have nothing to attach to.
fn cue_snapshot(track_id: &str, cue: &::recipes::RecipeCue, slots: &CueSlots) -> serde_json::Value {
    serde_json::json!({
        "content_id": track_id,
        "in_msec": cue.position_ms,
        "out_msec": cue.loop_end_ms,
        // 0 is a memory cue; 1..=8 a hot-cue slot. A hot cue whose slot we
        // somehow lost track of goes back as slot 1 rather than silently
        // becoming a memory cue.
        "kind": if cue.memory { 0 } else { slots.get(cue.id.as_str()).copied().unwrap_or(1) },
        "color": cue.color.unwrap_or(-1),
        "commnt": cue.name,
    })
}

/// Preview a cue recipe over a selection.
#[tauri::command]
pub async fn cue_recipe_preview(
    library_path: String,
    track_ids: Vec<String>,
    recipe: ::recipes::CueRecipe,
) -> Result<Vec<CueRecipeTrack>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let reorders = matches!(recipe, ::recipes::CueRecipe::SortCues { .. });

        let mut out = Vec::new();
        for id in &track_ids {
            let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? else {
                continue;
            };
            let (cues, slots) = recipe_cues(&db, id)?;
            if cues.is_empty() {
                continue;
            }
            let grid = beat_grid(&library_path, &track);
            let edits = ::recipes::apply_cue_recipe(&recipe, &cues, &grid);
            let (changes, deletions) = diff_cues(id, &cues, &slots, &edits, reorders);
            if changes.is_empty() && deletions.is_empty() && edits.skipped.is_none() {
                continue;
            }
            out.push(CueRecipeTrack {
                track_id: track.id.clone(),
                track_title: track.title.clone(),
                edits: changes,
                deletions,
                skipped: edits.skipped,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage reviewed cue edits.
///
/// Takes the preview back rather than re-running, so what is staged is exactly
/// what was on screen. Deletions are staged last: staging them first would make
/// the edit rows above them refer to cues the same batch is about to remove.
#[tauri::command]
pub async fn cue_recipe_apply(
    app: tauri::AppHandle,
    library_path: String,
    tracks: Vec<CueRecipeTrack>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut staged = Vec::new();

        for track in &tracks {
            for edit in &track.edits {
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::CueMetadataEdit,
                        target_id: Some(edit.cue_id.clone()),
                        field: Some(edit.field.clone()),
                        old_value: Some(edit.before.clone()),
                        new_value: Some(edit.after.clone()),
                        reason: Some("Cue recipe".to_string()),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| e.to_string())?;
                staged.push(record.id);
            }
        }

        for track in &tracks {
            for deletion in &track.deletions {
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::TrackDeleteCue,
                        target_id: Some(deletion.cue_id.clone()),
                        field: None,
                        // The snapshot is what makes the deletion undoable —
                        // `changes::undo` refuses to invert a delete that did
                        // not record what it removed.
                        old_value: Some(deletion.snapshot.clone()),
                        new_value: None,
                        reason: Some(format!("Cue recipe — delete {}", deletion.cue_label)),
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

// ── Tag recipes ──────────────────────────────────────────────────────────────

/// What a tag recipe would do to one track, as the preview shows it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagProposal {
    pub track_id: String,
    pub track_title: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// Preview a tag recipe over a selection.
///
/// Tags live in the local cache rather than `master.db`, so unlike the field
/// recipes these apply directly rather than staging — there is no sync step to
/// carry them, and `set_track_tags` is already the app's write path. The
/// preview still exists because "run this over 400 tracks" deserves a look
/// first either way.
#[tauri::command]
pub async fn tag_recipe_preview(
    app: tauri::AppHandle,
    library_path: String,
    track_ids: Vec<String>,
    recipe: TagRecipe,
) -> Result<Vec<TagProposal>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for id in &track_ids {
            let Some(track) = db.track_by_id(id).map_err(|e| e.to_string())? else {
                continue;
            };
            let current: Vec<String> = cache
                .get_track_tags(&library_path, id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|t| t.name)
                .collect();

            let change = apply_tag_recipe(&recipe, &track_to_fields(&track), &current);
            if change.is_empty() {
                continue;
            }
            out.push(TagProposal {
                track_id: track.id.clone(),
                track_title: track.title.clone(),
                added: change.added,
                removed: change.removed,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Where the spec puts tags an import invented.
pub(crate) const IMPORTED_TAGS_CATEGORY: &str = "Imported Tags";

/// Find or create the `Imported Tags` category.
///
/// Matched case-insensitively so a user who renamed it to "imported tags" — or
/// made one themselves before importing — gets theirs rather than a second one.
fn ensure_imported_category(cache: &cache::CacheDb) -> Result<String, String> {
    let existing = cache
        .list_tag_categories()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.name.trim().eq_ignore_ascii_case(IMPORTED_TAGS_CATEGORY));
    if let Some(category) = existing {
        return Ok(category.id);
    }
    cache
        .create_tag_category(IMPORTED_TAGS_CATEGORY)
        .map(|c| c.id)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Default, Serialize)]
pub struct TagApplyResult {
    pub tracks_changed: usize,
    pub tags_added: usize,
    pub tags_removed: usize,
    /// Tag names that had to be created because no tag by that name existed.
    pub tags_created: Vec<String>,
}

/// Apply reviewed tag proposals.
///
/// Takes the proposals back rather than re-running the recipe, so what happens
/// is what the user reviewed. A tag name with no existing tag is created rather
/// than skipped — importing `#Techno` from a comment is useless if it fails
/// because nobody made a Techno tag first.
///
/// **New tags land in a reserved `Imported Tags` category**, per
/// `docs/lexicon/02-library.md §Custom Tags`: "unknown tags land in an
/// `Imported Tags` category for the user to sort". They used to go into
/// whichever category happened to be first, which is arbitrary — it quietly
/// fills a real category like Genre with unsorted imports, and there is no way
/// to tell afterwards which tags the user put there and which the importer did.
/// The category is created on demand, so a library that never imports never
/// gets one.
#[tauri::command]
pub async fn tag_recipe_apply(
    app: tauri::AppHandle,
    library_path: String,
    proposals: Vec<TagProposal>,
) -> Result<TagApplyResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut result = TagApplyResult::default();

        // Resolve names to ids once for the batch. Case-insensitive, so
        // importing "#techno" finds an existing "Techno".
        let mut by_name: std::collections::HashMap<String, String> = cache
            .list_tags(None)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| (t.name.to_lowercase(), t.id))
            .collect();

        // Resolved lazily: creating an `Imported Tags` category for a run that
        // turns out to need no new tags would leave an empty category behind.
        let mut imported_category: Option<String> = None;

        for p in proposals {
            let mut touched = false;

            for name in &p.added {
                let key = name.to_lowercase();
                let id = match by_name.get(&key) {
                    Some(id) => id.clone(),
                    None => {
                        let category = match &imported_category {
                            Some(id) => id.clone(),
                            None => {
                                let id = ensure_imported_category(&cache)?;
                                imported_category = Some(id.clone());
                                id
                            }
                        };
                        let created = cache
                            .create_tag(&category, name)
                            .map_err(|e| e.to_string())?;
                        by_name.insert(key, created.id.clone());
                        result.tags_created.push(name.clone());
                        created.id
                    }
                };
                cache
                    .add_track_tag(&library_path, &p.track_id, &id)
                    .map_err(|e| e.to_string())?;
                result.tags_added += 1;
                touched = true;
            }

            for name in &p.removed {
                if let Some(id) = by_name.get(&name.to_lowercase()) {
                    cache
                        .remove_track_tag(&library_path, &p.track_id, id)
                        .map_err(|e| e.to_string())?;
                    result.tags_removed += 1;
                    touched = true;
                }
            }

            if touched {
                result.tracks_changed += 1;
            }
        }
        Ok(result)
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
            bpm: Some(128.0),
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
            energy: None,
        }
    }

    #[test]
    fn every_offered_field_maps_to_a_writable_column() {
        // A recipe field with no column would produce preview rows that
        // silently vanish at sync time.
        for field in RECIPE_FIELDS {
            assert!(column_for(field).is_some(), "{field} has no column");
        }
    }

    #[test]
    fn an_integer_bpm_survives_the_round_trip_as_an_integer() {
        let f = track_to_fields(&track());
        assert_eq!(f.get("bpm"), Some("128"));
    }

    #[test]
    fn a_fractional_bpm_keeps_its_fraction() {
        let mut t = track();
        t.bpm = Some(127.5);
        assert_eq!(track_to_fields(&t).get("bpm"), Some("127.5"));
    }

    #[test]
    fn absent_track_values_are_absent_fields_not_empty_ones() {
        let f = track_to_fields(&track());
        assert!(f.get("album").is_none());
        assert!(f.get("comment").is_none());
    }

    #[test]
    fn a_change_on_an_unwritable_field_is_dropped_from_the_proposals() {
        // The engine can edit any field name; only the mapped ones can be
        // staged, and offering the rest would be a lie.
        let changes = vec![
            FieldChange {
                field: "title".into(),
                before: Some("a".into()),
                after: Some("b".into()),
            },
            FieldChange {
                field: "extra1".into(),
                before: None,
                after: Some("x".into()),
            },
        ];
        let got = proposals_for(&track(), &changes);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].field, "title");
    }

    #[test]
    fn proposal_ids_are_unique_per_track_and_field() {
        let changes = vec![
            FieldChange {
                field: "title".into(),
                before: None,
                after: Some("x".into()),
            },
            FieldChange {
                field: "artist".into(),
                before: None,
                after: Some("y".into()),
            },
        ];
        let got = proposals_for(&track(), &changes);
        assert_ne!(got[0].id, got[1].id);
        assert!(got[0].id.starts_with("t1:"));
    }

    // ── cue recipes ─────────────────────────────────────────────────────────

    fn rcue(id: &str, pos: i64, name: Option<&str>, color: Option<i64>) -> ::recipes::RecipeCue {
        ::recipes::RecipeCue {
            id: id.into(),
            position_ms: pos,
            loop_end_ms: None,
            name: name.map(String::from),
            color,
            memory: false,
        }
    }

    fn diff_of(
        before: &[::recipes::RecipeCue],
        recipe: ::recipes::CueRecipe,
    ) -> (Vec<CueChange>, Vec<CueDeletion>) {
        let reorders = matches!(recipe, ::recipes::CueRecipe::SortCues { .. });
        let edits = ::recipes::apply_cue_recipe(&recipe, before, &[]);
        let slots: CueSlots = before
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i as i64 + 1))
            .collect();
        diff_cues("track-1", before, &slots, &edits, reorders)
    }

    #[test]
    fn a_cue_label_reads_as_a_timestamp_and_name() {
        assert_eq!(
            cue_label(&rcue("a", 65_000, Some("Drop"), None)),
            "1:05 Drop"
        );
        assert_eq!(cue_label(&rcue("a", 5_000, None, None)), "0:05");
        // A blank name is not a name.
        assert_eq!(cue_label(&rcue("a", 5_000, Some("  "), None)), "0:05");
    }

    #[test]
    fn only_the_fields_a_recipe_touched_become_edits() {
        let cues = vec![rcue("a", 1000, Some("Intro"), Some(1))];
        let (edits, _) = diff_of(&cues, ::recipes::CueRecipe::ShiftCues { offset_ms: 500 });
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].field, "InMsec");
        assert_eq!(edits[0].after, serde_json::json!(1500));
    }

    #[test]
    fn a_position_edit_stages_a_number_not_a_string() {
        // `djmdCue.InMsec` is an integer column; staging "1500" would land as
        // text and the applier's json_to_sql has no way to know better.
        let cues = vec![rcue("a", 1000, None, None)];
        let (edits, _) = diff_of(&cues, ::recipes::CueRecipe::ShiftCues { offset_ms: 500 });
        assert!(edits[0].after.is_number());
    }

    #[test]
    fn clearing_a_colour_stages_minus_one_not_null() {
        // Rekordbox spells "no colour" as -1.
        let cues = vec![rcue("a", 1000, None, Some(4))];
        let (edits, _) = diff_of(
            &cues,
            ::recipes::CueRecipe::ChangeColours {
                scheme: ::recipes::ColourScheme::None,
            },
        );
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].field, "Color");
        assert_eq!(edits[0].after, serde_json::json!(-1));
    }

    #[test]
    fn only_a_sort_reassigns_hot_cue_slots() {
        let cues = vec![rcue("a", 2000, None, None), rcue("b", 1000, None, None)];
        let (sorted, _) = diff_of(
            &cues,
            ::recipes::CueRecipe::SortCues {
                order: ::recipes::SortOrder::TimeAsc,
            },
        );
        assert!(sorted.iter().any(|e| e.field == "Kind"));

        let (shifted, _) = diff_of(&cues, ::recipes::CueRecipe::ShiftCues { offset_ms: 1 });
        assert!(shifted.iter().all(|e| e.field != "Kind"));
    }

    #[test]
    fn a_sort_that_changes_nothing_stages_nothing() {
        let cues = vec![rcue("a", 1000, None, None), rcue("b", 2000, None, None)];
        let (edits, _) = diff_of(
            &cues,
            ::recipes::CueRecipe::SortCues {
                order: ::recipes::SortOrder::TimeAsc,
            },
        );
        assert!(edits.is_empty());
    }

    #[test]
    fn memory_cues_never_get_a_slot() {
        // Memory cues have no slot to reassign; writing Kind on one would turn
        // it into a hot cue.
        let cues = vec![
            ::recipes::RecipeCue {
                memory: true,
                ..rcue("m", 2000, None, None)
            },
            rcue("h", 1000, None, None),
        ];
        let (edits, _) = diff_of(
            &cues,
            ::recipes::CueRecipe::SortCues {
                order: ::recipes::SortOrder::TimeAsc,
            },
        );
        assert!(edits.iter().all(|e| e.field != "Kind" || e.cue_id == "h"));
    }

    #[test]
    fn deletions_carry_a_label_from_before_the_recipe_ran() {
        let cues = vec![rcue("a", 65_000, Some("Drop"), None)];
        let (_, deletions) = diff_of(
            &cues,
            ::recipes::CueRecipe::RemoveCuesByLabel {
                text: "drop".into(),
            },
        );
        assert_eq!(deletions.len(), 1);
        assert_eq!(deletions[0].cue_label, "1:05 Drop");
    }

    #[test]
    fn skip_reasons_read_as_sentences_not_enum_names() {
        assert_eq!(
            describe(&::recipes::Skipped::SourceEmpty {
                field: "remixer".into()
            }),
            "remixer is empty"
        );
        assert!(describe(&::recipes::Skipped::NotANumber {
            field: "title".into(),
            value: "abc".into()
        })
        .contains("not a number"));
    }

    // ── Imported Tags category ───────────────────────────────────────────────

    #[test]
    fn imported_tags_go_to_their_own_category_not_the_first_one() {
        // The old behaviour dropped invented tags into whichever category came
        // first, quietly filling a real category like Genre with unsorted
        // imports — and leaving no way to tell afterwards which tags the user
        // put there and which the importer did.
        let cache = cache::CacheDb::open_in_memory().unwrap();
        cache.create_tag_category("Genre").unwrap();

        let id = ensure_imported_category(&cache).unwrap();
        let category = cache
            .list_tag_categories()
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(category.name, IMPORTED_TAGS_CATEGORY);
    }

    #[test]
    fn an_existing_imported_tags_category_is_reused() {
        let cache = cache::CacheDb::open_in_memory().unwrap();
        let made = cache.create_tag_category(IMPORTED_TAGS_CATEGORY).unwrap();
        assert_eq!(ensure_imported_category(&cache).unwrap(), made.id);
        assert_eq!(cache.list_tag_categories().unwrap().len(), 1);
    }

    #[test]
    fn a_renamed_imported_category_still_matches() {
        // Case-insensitive, so someone who typed it themselves in lower case
        // does not end up with two.
        let cache = cache::CacheDb::open_in_memory().unwrap();
        let made = cache.create_tag_category("imported tags").unwrap();
        assert_eq!(ensure_imported_category(&cache).unwrap(), made.id);
    }

    #[test]
    fn importing_into_an_empty_tag_tree_no_longer_fails() {
        // It used to error with "create a tag category before importing tags",
        // which made the first import of a fresh library impossible.
        let cache = cache::CacheDb::open_in_memory().unwrap();
        assert!(ensure_imported_category(&cache).is_ok());
    }
}
