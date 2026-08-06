//! Tauri commands for cue editing and beat-grid arithmetic (Epic 2).
//!
//! Every mutation goes through the staged-change pipeline rather than writing
//! `master.db` directly — cue edits are exactly the kind of change a DJ wants
//! to review before it touches a library they perform from.

use std::path::Path;

use decks_core::rekordbox_db::{
    anlz, quantize, quantize::QuantizeResolution, BeatGridEntry, RekordboxDb,
};
use serde::{Deserialize, Serialize};

use crate::cache_db;

/// The cue payload the editor sends. Mirrors the `new_value` shape
/// `changes::applier::cues::apply_add_cue` expects.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CueInput {
    pub in_msec: i64,
    /// Present when the cue is a loop. `djmdCue.OutMsec`.
    #[serde(default)]
    pub out_msec: Option<i64>,
    /// 0 = memory cue, 1–8 = hot cue slot.
    #[serde(default)]
    pub kind: i64,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub comment: Option<String>,
}

fn beat_grid_for(library_path: &str, track_id: &str) -> Result<Vec<BeatGridEntry>, String> {
    let db = RekordboxDb::open(Path::new(library_path)).map_err(|e| e.to_string())?;
    let track = db
        .track_by_id(track_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("track {track_id} not found"))?;
    let Some(analysis_path) = track.analysis_data_path else {
        return Ok(Vec::new());
    };
    let lib_dir = Path::new(library_path).parent().unwrap_or(Path::new(""));
    let Some(resolved) = anlz::resolve_anlz_path(lib_dir, &analysis_path) else {
        return Ok(Vec::new());
    };
    Ok(anlz::read_beat_grid(&resolved).unwrap_or_default())
}

/// The track's beat grid, for the waveform to draw and for client-side snapping.
#[tauri::command]
pub async fn get_beat_grid(
    library_path: String,
    track_id: String,
) -> Result<Vec<BeatGridEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || beat_grid_for(&library_path, &track_id))
        .await
        .map_err(|e| e.to_string())?
}

/// Snap a position onto the grid. Returns the input unchanged when the track
/// has no analysis, so an un-analysed track never has its cues silently moved.
#[tauri::command]
pub async fn quantize_position(
    library_path: String,
    track_id: String,
    position_ms: i64,
    resolution: QuantizeResolution,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let grid = beat_grid_for(&library_path, &track_id)?;
        Ok(quantize::snap_to_grid(&grid, position_ms, resolution))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Move a position by whole beats along the grid, clamped to its ends.
#[tauri::command]
pub async fn beat_jump_position(
    library_path: String,
    track_id: String,
    position_ms: i64,
    beats: i64,
) -> Result<i64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let grid = beat_grid_for(&library_path, &track_id)?;
        Ok(quantize::beat_jump(&grid, position_ms, beats))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage a new cue or loop. `quantize` snaps the in-point (and the out-point,
/// preserving loop length in beats) before staging.
#[tauri::command]
pub async fn stage_cue_add(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    cue: CueInput,
    quantize_to: Option<QuantizeResolution>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut cue = cue;
        if let Some(res) = quantize_to {
            let grid = beat_grid_for(&library_path, &track_id)?;
            let snapped_in = quantize::snap_to_grid(&grid, cue.in_msec, res);
            // Shift the out-point by the same delta so quantising an existing
            // loop preserves its length rather than stretching it.
            if let Some(out) = cue.out_msec {
                cue.out_msec = Some((out + (snapped_in - cue.in_msec)).max(snapped_in));
            }
            cue.in_msec = snapped_in;
        }

        let db = cache_db(&app)?;
        let record = db
            .stage_change(changes::NewChange {
                library_path: Some(library_path.clone()),
                kind: changes::ChangeKind::TrackAddCue,
                target_id: Some(track_id),
                field: None,
                old_value: None,
                new_value: Some(serde_json::json!({
                    "in_msec": cue.in_msec,
                    "out_msec": cue.out_msec,
                    "kind": cue.kind,
                    "color": cue.color.unwrap_or(-1),
                    "commnt": cue.comment,
                })),
                reason: Some(if cue.out_msec.is_some() {
                    "Loop added in the cue editor".to_string()
                } else {
                    "Cue added in the cue editor".to_string()
                }),
                confidence: None,
            })
            .map_err(|e| e.to_string())?;
        Ok(record.id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn stage_cue_delete(
    app: tauri::AppHandle,
    library_path: String,
    cue_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        let record = db
            .stage_change(changes::NewChange {
                library_path: Some(library_path),
                kind: changes::ChangeKind::TrackDeleteCue,
                target_id: Some(cue_id),
                field: None,
                old_value: None,
                new_value: None,
                reason: Some("Cue deleted in the cue editor".to_string()),
                confidence: None,
            })
            .map_err(|e| e.to_string())?;
        Ok(record.id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Edit one column of an existing cue. `field` must be in the applier's
/// allowlist (`InMsec`, `OutMsec`, `Kind`, `Color`, `Commnt`).
#[tauri::command]
pub async fn stage_cue_edit(
    app: tauri::AppHandle,
    library_path: String,
    cue_id: String,
    field: String,
    old_value: Option<serde_json::Value>,
    new_value: serde_json::Value,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = cache_db(&app)?;
        let record = db
            .stage_change(changes::NewChange {
                library_path: Some(library_path),
                kind: changes::ChangeKind::CueMetadataEdit,
                target_id: Some(cue_id),
                field: Some(field.clone()),
                old_value,
                new_value: Some(new_value),
                reason: Some(format!("Cue {field} changed in the cue editor")),
                confidence: None,
            })
            .map_err(|e| e.to_string())?;
        Ok(record.id)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage `InMsec` edits for every cue that follows a beat-grid move.
///
/// Cues already sitting on the grid move with it; cues deliberately placed
/// off-grid are left alone. Returns the staged change IDs.
#[tauri::command]
pub async fn stage_grid_shift(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    offset_ms: i64,
    tolerance_ms: Option<i64>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let grid = beat_grid_for(&library_path, &track_id)?;
        let db = RekordboxDb::open(Path::new(&library_path)).map_err(|e| e.to_string())?;
        let cues = db
            .hot_cues_for_track(&track_id)
            .map_err(|e| e.to_string())?;

        let positions: Vec<i64> = cues.iter().filter_map(|c| c.in_msec).collect();
        let moved =
            quantize::cues_following_grid(&grid, &positions, offset_ms, tolerance_ms.unwrap_or(5));

        let cache = cache_db(&app)?;
        let mut staged = Vec::new();
        for (from, to) in moved {
            // Multiple cues can share a position; move all of them.
            for cue in cues.iter().filter(|c| c.in_msec == Some(from)) {
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::CueMetadataEdit,
                        target_id: Some(cue.id.clone()),
                        field: Some("InMsec".to_string()),
                        old_value: Some(serde_json::json!(from)),
                        new_value: Some(serde_json::json!(to)),
                        reason: Some("Cue followed a beat-grid move".to_string()),
                        confidence: None,
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

// ── Cue presets (Epic 2) ─────────────────────────────────────────────────────

/// A saved name+colour pair, as the player sees it.
///
/// Deliberately **not** called a template: `crates/cue-generator` already owns
/// `CueTemplate` for its bulk-generation rule sets. Two things called
/// "template" in one player would be unreadable. See
/// `docs/lexicon/05-cues-player.md §Cue templates`.
#[derive(Debug, Clone, Serialize)]
pub struct CuePresetView {
    pub id: String,
    pub name: String,
    pub color: Option<i64>,
    /// `1`–`8` for the presets that carry a hotkey, `None` beyond that. Served
    /// rather than derived in the renderer so the hotkey and the badge cannot
    /// disagree about which preset a number means.
    pub hotkey: Option<u8>,
}

/// How many presets get a number key. The spec binds the first eight.
const HOTKEY_PRESETS: usize = 8;

#[tauri::command]
pub fn list_cue_presets(app: tauri::AppHandle) -> Result<Vec<CuePresetView>, String> {
    let cache = cache_db(&app)?;
    Ok(cache
        .list_cue_presets()
        .map_err(|e| e.to_string())?
        .into_iter()
        .enumerate()
        .map(|(index, p)| CuePresetView {
            id: p.id,
            name: p.name,
            color: p.color,
            hotkey: (index < HOTKEY_PRESETS).then(|| (index + 1) as u8),
        })
        .collect())
}

/// Promote a name+colour into a preset.
///
/// The spec creates these by right-clicking an existing cue, which is why this
/// takes the two values rather than a cue id — the caller has the cue, and a
/// preset that remembered *which* cue it came from would be a different, more
/// confusing object.
#[tauri::command]
pub fn create_cue_preset(
    app: tauri::AppHandle,
    name: String,
    color: Option<i64>,
) -> Result<String, String> {
    cache_db(&app)?
        .create_cue_preset(&name, color)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_cue_preset(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    cache_db(&app)?
        .delete_cue_preset(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_cue_preset_order(app: tauri::AppHandle, ids: Vec<String>) -> Result<(), String> {
    cache_db(&app)?
        .set_cue_preset_order(&ids)
        .map_err(|e| e.to_string())
}

/// Stamp a preset onto an existing cue.
///
/// Stages one `CueMetadataEdit` per field that actually changes — nothing is
/// written to `master.db` here. A preset with no colour stages only the name,
/// which is what "leave the colour alone" has to mean; and re-applying the same
/// preset stages nothing at all, so the review panel does not fill with rows
/// that do nothing.
#[tauri::command]
pub async fn apply_cue_preset(
    app: tauri::AppHandle,
    library_path: String,
    cue_id: String,
    preset_id: String,
    current_name: Option<String>,
    current_color: Option<i64>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let preset = cache
            .list_cue_presets()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|p| p.id == preset_id)
            .ok_or_else(|| format!("no cue preset {preset_id}"))?;

        let mut staged = Vec::new();
        let mut stage =
            |field: &str, old: serde_json::Value, new: serde_json::Value| -> Result<(), String> {
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::CueMetadataEdit,
                        target_id: Some(cue_id.clone()),
                        field: Some(field.to_string()),
                        old_value: Some(old),
                        new_value: Some(new),
                        reason: Some(format!("Applied cue preset “{}”", preset.name)),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| e.to_string())?;
                staged.push(record.id);
                Ok(())
            };

        if current_name.as_deref() != Some(preset.name.as_str()) {
            stage(
                "Commnt",
                match &current_name {
                    Some(v) => serde_json::json!(v),
                    None => serde_json::Value::Null,
                },
                serde_json::json!(preset.name),
            )?;
        }
        if let Some(color) = preset.color {
            if current_color != Some(color) {
                stage(
                    "Color",
                    match current_color {
                        Some(v) => serde_json::json!(v),
                        None => serde_json::Value::Null,
                    },
                    serde_json::json!(color),
                )?;
            }
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod preset_tests {
    use super::HOTKEY_PRESETS;

    #[test]
    fn the_first_eight_presets_carry_hotkeys() {
        // The badge the UI draws and the key the action registry binds are both
        // derived from this, so it is worth pinning.
        let hotkey = |index: usize| (index < HOTKEY_PRESETS).then(|| (index + 1) as u8);
        assert_eq!(hotkey(0), Some(1));
        assert_eq!(hotkey(7), Some(8));
        assert_eq!(hotkey(8), None);
    }
}
