//! Tauri commands for the Cue Point Generator (Epic 3, part 1).
//!
//! Custom cue anchors only for now: the user names which of their existing cues
//! is the drop, the breakdown and so on, and the template is applied to those.
//! Detection lands later and will feed the same `ResolvedAnchor` values into the
//! same template engine.

use std::path::Path;

use cue_generator::{
    apply_template, resolve_custom_anchors, Anchor, Confidence, CueRef, CueTemplate,
    CustomAnchorRule, GenerationResult, ResolvedAnchor, StartCueBehavior,
};
use decks_core::rekordbox_db::{anlz, BeatGridEntry, CueKind, RekordboxDb};
use serde::Serialize;

use crate::cache_db;

/// Rekordbox holds eight hot cues.
const MAX_HOT_CUES: usize = 8;

#[derive(Debug, Serialize)]
pub struct GeneratePreview {
    pub cues: Vec<cue_generator::GeneratedCue>,
    pub skipped: Vec<cue_generator::SkippedCue>,
    /// Anchors that were resolved, so the UI can show what the template hung off.
    pub anchors: Vec<ResolvedAnchor>,
}

fn load(
    library_path: &str,
    track_id: &str,
) -> Result<(Vec<CueRef>, Vec<BeatGridEntry>, i64), String> {
    let db = RekordboxDb::open(Path::new(library_path)).map_err(|e| e.to_string())?;
    let track = db
        .track_by_id(track_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("track {track_id} not found"))?;

    let mut cues: Vec<CueRef> = db
        .hot_cues_for_track(track_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|c| {
            c.in_msec.map(|pos| CueRef {
                position_ms: pos,
                name: c.comment,
                color: c.color.filter(|v| *v >= 0),
            })
        })
        .collect();
    // "First cue with that name" means first in track order.
    cues.sort_by_key(|c| c.position_ms);

    let grid = track
        .analysis_data_path
        .as_deref()
        .and_then(|p| {
            let lib_dir = Path::new(library_path).parent().unwrap_or(Path::new(""));
            anlz::resolve_anlz_path(lib_dir, p)
        })
        .map(|resolved| anlz::read_beat_grid(&resolved).unwrap_or_default())
        .unwrap_or_default();

    let duration_ms = track.duration_secs.unwrap_or(0) * 1000;
    Ok((cues, grid, duration_ms))
}

/// Resolve the Start anchor, which is the one anchor that does not come from a
/// detection or a mapping rule.
fn start_anchor(
    behavior: StartCueBehavior,
    cues: &[CueRef],
    grid: &[BeatGridEntry],
) -> Option<ResolvedAnchor> {
    let position = match behavior {
        StartCueBehavior::Zero => 0,
        StartCueBehavior::FirstBeat => grid.first().map(|b| b.time_ms as i64)?,
        StartCueBehavior::ExistingCue => cues
            .first()
            .map(|c| c.position_ms)
            .or_else(|| grid.first().map(|b| b.time_ms as i64))?,
    };
    Some(ResolvedAnchor {
        anchor: Anchor::Start,
        position_ms: position,
        confidence: Confidence::Certain,
    })
}

/// Preview what a template would produce, without staging anything.
///
/// Preview and apply share this function so what the user reviews is exactly
/// what gets staged.
fn generate(
    library_path: &str,
    track_id: &str,
    template: &CueTemplate,
    rules: &[CustomAnchorRule],
) -> Result<GeneratePreview, String> {
    let (cues, grid, duration_ms) = load(library_path, track_id)?;

    let mut anchors = resolve_custom_anchors(&cues, rules);
    if let Some(start) = start_anchor(template.start_behavior, &cues, &grid) {
        anchors.push(start);
    }

    let GenerationResult { cues, skipped } =
        apply_template(template, &anchors, &grid, duration_ms, MAX_HOT_CUES);

    Ok(GeneratePreview {
        cues,
        skipped,
        anchors,
    })
}

#[tauri::command]
pub async fn preview_generated_cues(
    library_path: String,
    track_id: String,
    template: CueTemplate,
    anchor_rules: Vec<CustomAnchorRule>,
) -> Result<GeneratePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        generate(&library_path, &track_id, &template, &anchor_rules)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stage the generated cues as `TrackAddCue` changes.
///
/// Nothing is written to `master.db` here — generated cues are exactly the kind
/// of thing that should be reviewable before it touches a performing library,
/// and low-confidence anchors make that doubly true.
#[tauri::command]
pub async fn apply_generated_cues(
    app: tauri::AppHandle,
    library_path: String,
    track_id: String,
    template: CueTemplate,
    anchor_rules: Vec<CustomAnchorRule>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let preview = generate(&library_path, &track_id, &template, &anchor_rules)?;
        let cache = cache_db(&app)?;

        let mut staged = Vec::new();
        for cue in preview.cues {
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackAddCue,
                    target_id: Some(track_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(serde_json::json!({
                        "in_msec": cue.position_ms,
                        "out_msec": cue.loop_end_ms,
                        "kind": if cue.memory_cue { 0 } else { cue.slot as i64 },
                        "color": cue.color.unwrap_or(-1),
                        "commnt": cue.name,
                    })),
                    reason: Some(format!(
                        "Cue Point Generator — template \"{}\"{}",
                        template.name,
                        if cue.confidence.is_provisional() {
                            format!(
                                " (provisional, confidence {:.0}%)",
                                cue.confidence.score() * 100.0
                            )
                        } else {
                            String::new()
                        }
                    )),
                    confidence: Some(cue.confidence.score() as f64),
                })
                .map_err(|e| e.to_string())?;
            staged.push(record.id);
        }
        Ok(staged)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Suggest anchor rules from a track's existing cues, so the user has something
/// to edit rather than an empty form. Purely a convenience — it guesses from
/// common cue names and never invents an anchor the names do not support.
#[tauri::command]
pub async fn suggest_anchor_rules(
    library_path: String,
    track_id: String,
) -> Result<Vec<CustomAnchorRule>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = RekordboxDb::open(Path::new(&library_path)).map_err(|e| e.to_string())?;
        let cues = db
            .hot_cues_for_track(&track_id)
            .map_err(|e| e.to_string())?;

        let mut rules = Vec::new();
        let mut drops = 0u8;
        let mut breakdowns = 0u8;
        let mut sorted: Vec<_> = cues
            .into_iter()
            .filter(|c| matches!(c.kind, CueKind::HotCue(_)) || c.in_msec.is_some())
            .collect();
        sorted.sort_by_key(|c| c.in_msec.unwrap_or(0));

        for cue in sorted {
            let Some(name) = cue.comment.as_deref() else {
                continue;
            };
            let lower = name.trim().to_lowercase();
            let anchor = if lower.contains("drop") {
                drops += 1;
                Some(Anchor::drop(drops))
            } else if lower.contains("break") {
                breakdowns += 1;
                Some(Anchor::breakdown(breakdowns))
            } else if lower.contains("outro") || lower.contains("fade") {
                Some(Anchor::FadeOut)
            } else {
                None
            };
            if let Some(anchor) = anchor {
                rules.push(CustomAnchorRule {
                    anchor,
                    name: Some(name.to_string()),
                    color: None,
                });
            }
        }
        Ok(rules)
    })
    .await
    .map_err(|e| e.to_string())?
}
