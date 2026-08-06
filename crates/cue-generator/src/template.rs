//! Cue templates: "a cue 64 beats before the drop", named and coloured.
//!
//! A template is declarative. It says nothing about *where* the drop is — that
//! comes from resolved anchors — only what to place relative to it. The same
//! template therefore works whether the anchors were detected or supplied by
//! the user as custom cue anchors.

use rekordbox_db::quantize;
use rekordbox_db::types::BeatGridEntry;
use serde::{Deserialize, Serialize};

use crate::anchor::{Anchor, Confidence, ResolvedAnchor};

/// Where the Start cue goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartCueBehavior {
    /// The first detected beat.
    #[default]
    FirstBeat,
    /// The track's first existing cue, falling back to the first beat.
    ExistingCue,
    /// Always 0:00.
    Zero,
}

/// One row of a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub anchor: Anchor,
    /// Beats relative to the anchor. Negative is earlier — "64 beats before
    /// the drop" is `-64`.
    #[serde(default)]
    pub offset_beats: i64,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
    /// Unchecked entries are skipped, but **other entries may still depend on
    /// the same anchor** — disabling the drop's own cue does not stop a cue
    /// that sits 64 beats before it.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Emit as a memory cue rather than a hot cue.
    #[serde(default)]
    pub memory_cue: bool,
    /// Loop length in beats. `None` for a plain cue.
    #[serde(default)]
    pub loop_beats: Option<u32>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueTemplate {
    pub name: String,
    pub entries: Vec<TemplateEntry>,
    #[serde(default)]
    pub start_behavior: StartCueBehavior,
    /// Place each cue in its template slot index rather than packing them, so
    /// "the drop is always cue 1" and "the emergency loop is always cue 8"
    /// hold even when earlier entries produce nothing.
    #[serde(default)]
    pub keep_cue_position: bool,
}

/// A cue the generator wants to create.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedCue {
    pub position_ms: i64,
    pub name: String,
    pub color: Option<i64>,
    /// 1–8 for hot cues, 0 for memory cues.
    pub slot: u8,
    pub memory_cue: bool,
    pub loop_end_ms: Option<i64>,
    /// Inherited from the anchor this cue was placed against, so the UI can
    /// mark provisional results instead of presenting guesses as facts.
    pub confidence: Confidence,
    /// Which template row produced this, for "keep cue position".
    pub template_index: usize,
}

/// Why a cue was dropped, so the caller can tell the user rather than silently
/// producing fewer cues than the template describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum SkippedCue {
    /// The anchor was never resolved — e.g. no second drop was found.
    AnchorMissing { name: String, anchor: Anchor },
    /// The offset put the cue outside the track.
    OutOfRange { name: String, position_ms: i64 },
    /// More cues than the target app can hold.
    Overflow { name: String },
    /// Rekordbox refuses two memory cues at the same position.
    DuplicateMemoryCue { name: String, position_ms: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerationResult {
    pub cues: Vec<GeneratedCue>,
    pub skipped: Vec<SkippedCue>,
}

/// Milliseconds per beat, from the grid entry nearest a position.
fn ms_per_beat_at(grid: &[BeatGridEntry], position_ms: i64) -> Option<f64> {
    let idx = quantize::nearest_beat_index(grid, position_ms)?;
    let bpm = grid[idx].bpm();
    if bpm <= 0.0 {
        return None;
    }
    Some(60_000.0 / bpm)
}

/// Offset a position by whole beats, walking the grid where possible.
///
/// Walking the grid rather than multiplying by a constant tempo means offsets
/// stay correct across tempo changes; the arithmetic fallback only applies when
/// the offset runs past the end of the grid.
fn offset_by_beats(grid: &[BeatGridEntry], position_ms: i64, beats: i64) -> Option<i64> {
    if beats == 0 {
        return Some(position_ms);
    }
    let idx = quantize::nearest_beat_index(grid, position_ms)?;
    let target = idx as i64 + beats;
    if target >= 0 && (target as usize) < grid.len() {
        return Some(grid[target as usize].time_ms as i64);
    }
    let mpb = ms_per_beat_at(grid, position_ms)?;
    Some(position_ms + (beats as f64 * mpb).round() as i64)
}

/// Apply a template to a set of resolved anchors.
///
/// `max_hot_cues` is the target app's capacity. When the template yields more,
/// the **least interesting** cues are dropped first: lowest confidence, then
/// latest in the track, on the reasoning that a certain early landmark is worth
/// more to a DJ than a speculative late one.
pub fn apply_template(
    template: &CueTemplate,
    anchors: &[ResolvedAnchor],
    grid: &[BeatGridEntry],
    track_duration_ms: i64,
    max_hot_cues: usize,
) -> GenerationResult {
    let mut cues: Vec<GeneratedCue> = Vec::new();
    let mut skipped: Vec<SkippedCue> = Vec::new();

    for (index, entry) in template.entries.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let Some(anchor) = anchors.iter().find(|a| a.anchor == entry.anchor) else {
            skipped.push(SkippedCue::AnchorMissing {
                name: entry.name.clone(),
                anchor: entry.anchor,
            });
            continue;
        };

        let Some(position) = offset_by_beats(grid, anchor.position_ms, entry.offset_beats) else {
            skipped.push(SkippedCue::OutOfRange {
                name: entry.name.clone(),
                position_ms: anchor.position_ms,
            });
            continue;
        };

        if position < 0 || (track_duration_ms > 0 && position > track_duration_ms) {
            skipped.push(SkippedCue::OutOfRange {
                name: entry.name.clone(),
                position_ms: position,
            });
            continue;
        }

        let loop_end = entry.loop_beats.and_then(|b| {
            let mpb = ms_per_beat_at(grid, position)?;
            Some(position + (b as f64 * mpb).round() as i64)
        });

        cues.push(GeneratedCue {
            position_ms: position,
            name: entry.name.clone(),
            color: entry.color,
            slot: 0, // assigned below
            memory_cue: entry.memory_cue,
            loop_end_ms: loop_end,
            confidence: anchor.confidence,
            template_index: index,
        });
    }

    // Rekordbox refuses two memory cues at the same position and silently drops
    // one. Doing it ourselves means the user is told which, instead of finding
    // out when a cue is missing in Rekordbox.
    let mut seen_memory: Vec<i64> = Vec::new();
    cues.retain(|c| {
        if !c.memory_cue {
            return true;
        }
        if seen_memory.contains(&c.position_ms) {
            skipped.push(SkippedCue::DuplicateMemoryCue {
                name: c.name.clone(),
                position_ms: c.position_ms,
            });
            false
        } else {
            seen_memory.push(c.position_ms);
            true
        }
    });

    // Overflow trimming applies to hot cues only — memory cues do not consume
    // the 1–8 slots.
    let hot_count = cues.iter().filter(|c| !c.memory_cue).count();
    if hot_count > max_hot_cues {
        let mut ranked: Vec<usize> = cues
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.memory_cue)
            .map(|(i, _)| i)
            .collect();
        // Least interesting last: lower confidence first, then later position.
        ranked.sort_by(|&a, &b| {
            let ca = cues[a].confidence.score();
            let cb = cues[b].confidence.score();
            cb.partial_cmp(&ca)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(cues[a].position_ms.cmp(&cues[b].position_ms))
        });
        let doomed: Vec<usize> = ranked.into_iter().skip(max_hot_cues).collect();
        for &i in &doomed {
            skipped.push(SkippedCue::Overflow {
                name: cues[i].name.clone(),
            });
        }
        let mut keep = vec![true; cues.len()];
        for &i in &doomed {
            keep[i] = false;
        }
        let mut it = keep.iter();
        cues.retain(|_| *it.next().unwrap_or(&true));
    }

    assign_slots(&mut cues, template.keep_cue_position, max_hot_cues);
    cues.sort_by_key(|c| c.position_ms);

    GenerationResult { cues, skipped }
}

/// Assign hot-cue slots. Memory cues always take slot 0.
///
/// With `keep_cue_position`, a cue's slot is its template row index + 1, so
/// gaps left by skipped rows are preserved. Otherwise slots are packed in
/// template order.
fn assign_slots(cues: &mut [GeneratedCue], keep_position: bool, max_hot_cues: usize) {
    let mut next = 1u8;
    for cue in cues.iter_mut() {
        if cue.memory_cue {
            cue.slot = 0;
            continue;
        }
        if keep_position {
            let slot = cue.template_index + 1;
            cue.slot = slot.min(max_hot_cues.max(1)) as u8;
        } else {
            cue.slot = next;
            next = next.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_120(beats: usize) -> Vec<BeatGridEntry> {
        // 120 BPM → exactly 500ms per beat, which keeps the arithmetic in these
        // tests readable.
        (0..beats)
            .map(|i| BeatGridEntry {
                beat_number: (i % 4) as u16 + 1,
                tempo_bpm_x100: 12000,
                time_ms: (i * 500) as u32,
            })
            .collect()
    }

    fn anchor(a: Anchor, pos: i64) -> ResolvedAnchor {
        ResolvedAnchor {
            anchor: a,
            position_ms: pos,
            confidence: Confidence::Certain,
        }
    }

    fn entry(a: Anchor, offset: i64, name: &str) -> TemplateEntry {
        TemplateEntry {
            anchor: a,
            offset_beats: offset,
            name: name.into(),
            color: None,
            enabled: true,
            memory_cue: false,
            loop_beats: None,
        }
    }

    fn template(entries: Vec<TemplateEntry>) -> CueTemplate {
        CueTemplate {
            name: "T".into(),
            entries,
            start_behavior: StartCueBehavior::FirstBeat,
            keep_cue_position: false,
        }
    }

    #[test]
    fn places_a_cue_a_number_of_beats_before_an_anchor() {
        let grid = grid_120(200);
        // Drop at beat 64 (32000ms); 64 beats earlier is beat 0.
        let t = template(vec![entry(Anchor::drop(1), -64, "Build")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 32_000)], &grid, 120_000, 8);
        assert_eq!(out.cues.len(), 1);
        assert_eq!(out.cues[0].position_ms, 0);
    }

    #[test]
    fn a_zero_offset_lands_on_the_anchor() {
        let grid = grid_120(200);
        let t = template(vec![entry(Anchor::drop(1), 0, "Drop")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 32_000)], &grid, 120_000, 8);
        assert_eq!(out.cues[0].position_ms, 32_000);
    }

    #[test]
    fn a_missing_anchor_is_reported_not_silently_dropped() {
        let grid = grid_120(200);
        let t = template(vec![entry(Anchor::drop(2), 0, "Second drop")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 32_000)], &grid, 120_000, 8);
        assert!(out.cues.is_empty());
        assert_eq!(
            out.skipped,
            vec![SkippedCue::AnchorMissing {
                name: "Second drop".into(),
                anchor: Anchor::drop(2)
            }]
        );
    }

    #[test]
    fn a_disabled_entry_is_skipped_but_others_still_use_its_anchor() {
        let grid = grid_120(200);
        let mut drop_entry = entry(Anchor::drop(1), 0, "Drop");
        drop_entry.enabled = false;
        let t = template(vec![drop_entry, entry(Anchor::drop(1), -64, "Build")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 32_000)], &grid, 120_000, 8);
        // The drop's own cue is gone, the cue that depends on it survives.
        assert_eq!(out.cues.len(), 1);
        assert_eq!(out.cues[0].name, "Build");
    }

    #[test]
    fn cues_outside_the_track_are_reported() {
        let grid = grid_120(200);
        let t = template(vec![entry(Anchor::drop(1), -1000, "Way early")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 1_000)], &grid, 120_000, 8);
        assert!(out.cues.is_empty());
        assert!(matches!(out.skipped[0], SkippedCue::OutOfRange { .. }));
    }

    #[test]
    fn loop_length_is_computed_from_the_grid_tempo() {
        let grid = grid_120(200);
        let mut e = entry(Anchor::drop(1), 0, "Emergency");
        e.loop_beats = Some(8);
        let t = template(vec![e]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 10_000)], &grid, 120_000, 8);
        // 8 beats at 120 BPM = 4000ms.
        assert_eq!(out.cues[0].loop_end_ms, Some(14_000));
    }

    #[test]
    fn rekordbox_duplicate_memory_cue_guard() {
        let grid = grid_120(200);
        let mut a = entry(Anchor::FadeOut, 0, "Fade out");
        a.memory_cue = true;
        let mut b = entry(Anchor::breakdown(2), 0, "Second breakdown");
        b.memory_cue = true;
        let t = template(vec![a, b]);
        // Both anchors land on the same position — Rekordbox would drop one.
        let out = apply_template(
            &t,
            &[
                anchor(Anchor::FadeOut, 60_000),
                anchor(Anchor::breakdown(2), 60_000),
            ],
            &grid,
            120_000,
            8,
        );
        assert_eq!(out.cues.len(), 1);
        assert_eq!(
            out.skipped,
            vec![SkippedCue::DuplicateMemoryCue {
                name: "Second breakdown".into(),
                position_ms: 60_000
            }]
        );
    }

    #[test]
    fn two_hot_cues_at_the_same_position_are_allowed() {
        let grid = grid_120(200);
        let t = template(vec![
            entry(Anchor::drop(1), 0, "A"),
            entry(Anchor::breakdown(1), 0, "B"),
        ]);
        let out = apply_template(
            &t,
            &[
                anchor(Anchor::drop(1), 10_000),
                anchor(Anchor::breakdown(1), 10_000),
            ],
            &grid,
            120_000,
            8,
        );
        assert_eq!(out.cues.len(), 2);
    }

    #[test]
    fn overflow_drops_the_least_confident_first() {
        let grid = grid_120(400);
        let entries: Vec<TemplateEntry> = (0..3)
            .map(|i| entry(Anchor::drop(i as u8 + 1), 0, &format!("D{i}")))
            .collect();
        let t = template(entries);
        let anchors = vec![
            ResolvedAnchor {
                anchor: Anchor::drop(1),
                position_ms: 10_000,
                confidence: Confidence::Certain,
            },
            ResolvedAnchor {
                anchor: Anchor::drop(2),
                position_ms: 20_000,
                confidence: Confidence::Detected(0.3),
            },
            ResolvedAnchor {
                anchor: Anchor::drop(3),
                position_ms: 30_000,
                confidence: Confidence::Detected(0.9),
            },
        ];
        let out = apply_template(&t, &anchors, &grid, 200_000, 2);
        assert_eq!(out.cues.len(), 2);
        // D1 hangs off the 0.3-confidence anchor, so it is the one that goes —
        // the certain and the 0.9 survive.
        let names: Vec<&str> = out.cues.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["D0", "D2"]);
        assert_eq!(
            out.skipped,
            vec![SkippedCue::Overflow { name: "D1".into() }]
        );
    }

    #[test]
    fn memory_cues_do_not_consume_hot_cue_slots() {
        let grid = grid_120(400);
        let mut mem = entry(Anchor::FadeOut, 0, "Fade");
        mem.memory_cue = true;
        let t = template(vec![
            entry(Anchor::drop(1), 0, "A"),
            entry(Anchor::drop(2), 0, "B"),
            mem,
        ]);
        let out = apply_template(
            &t,
            &[
                anchor(Anchor::drop(1), 10_000),
                anchor(Anchor::drop(2), 20_000),
                anchor(Anchor::FadeOut, 30_000),
            ],
            &grid,
            200_000,
            2,
        );
        // Two hot cues fill the capacity; the memory cue survives regardless.
        assert_eq!(out.cues.len(), 3);
        assert_eq!(out.cues.iter().filter(|c| c.memory_cue).count(), 1);
    }

    #[test]
    fn slots_pack_in_template_order_by_default() {
        let grid = grid_120(400);
        let t = template(vec![
            entry(Anchor::drop(1), 0, "A"),
            entry(Anchor::drop(2), 0, "B"),
        ]);
        let out = apply_template(
            &t,
            &[
                anchor(Anchor::drop(1), 10_000),
                anchor(Anchor::drop(2), 20_000),
            ],
            &grid,
            200_000,
            8,
        );
        let slots: Vec<u8> = out.cues.iter().map(|c| c.slot).collect();
        assert_eq!(slots, vec![1, 2]);
    }

    #[test]
    fn keep_cue_position_preserves_gaps_from_skipped_rows() {
        let grid = grid_120(400);
        let mut t = template(vec![
            entry(Anchor::drop(1), 0, "Drop"),
            entry(Anchor::drop(2), 0, "Second drop"),
            entry(Anchor::FadeOut, 0, "Emergency"),
        ]);
        t.keep_cue_position = true;
        // No second drop, so row 2 produces nothing.
        let out = apply_template(
            &t,
            &[
                anchor(Anchor::drop(1), 10_000),
                anchor(Anchor::FadeOut, 30_000),
            ],
            &grid,
            200_000,
            8,
        );
        let slots: Vec<u8> = out.cues.iter().map(|c| c.slot).collect();
        // Emergency stays in slot 3 rather than sliding into 2.
        assert_eq!(slots, vec![1, 3]);
    }

    #[test]
    fn results_come_back_in_track_order() {
        let grid = grid_120(400);
        let t = template(vec![
            entry(Anchor::drop(1), 0, "Late"),
            entry(Anchor::Start, 0, "Early"),
        ]);
        let out = apply_template(
            &t,
            &[anchor(Anchor::drop(1), 30_000), anchor(Anchor::Start, 0)],
            &grid,
            200_000,
            8,
        );
        assert_eq!(out.cues[0].name, "Early");
        assert_eq!(out.cues[1].name, "Late");
    }

    #[test]
    fn confidence_is_inherited_from_the_anchor() {
        let grid = grid_120(200);
        let t = template(vec![entry(Anchor::drop(1), 0, "Drop")]);
        let out = apply_template(
            &t,
            &[ResolvedAnchor {
                anchor: Anchor::drop(1),
                position_ms: 10_000,
                confidence: Confidence::Detected(0.42),
            }],
            &grid,
            200_000,
            8,
        );
        assert_eq!(out.cues[0].confidence, Confidence::Detected(0.42));
        assert!(out.cues[0].confidence.is_provisional());
    }

    #[test]
    fn an_empty_grid_produces_nothing_rather_than_guessing() {
        let t = template(vec![entry(Anchor::drop(1), -64, "Build")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 32_000)], &[], 120_000, 8);
        assert!(out.cues.is_empty());
        assert!(matches!(out.skipped[0], SkippedCue::OutOfRange { .. }));
    }

    #[test]
    fn offsets_past_the_end_of_the_grid_fall_back_to_tempo_arithmetic() {
        // Grid covers 8 beats; asking for +16 runs off it.
        let grid = grid_120(8);
        let t = template(vec![entry(Anchor::drop(1), 16, "Later")]);
        let out = apply_template(&t, &[anchor(Anchor::drop(1), 0)], &grid, 200_000, 8);
        // 16 beats at 120 BPM = 8000ms.
        assert_eq!(out.cues[0].position_ms, 8_000);
    }

    #[test]
    fn templates_round_trip_through_json() {
        let t = template(vec![entry(Anchor::drop(1), -64, "Build")]);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<CueTemplate>(&json).unwrap(), t);
    }
}
