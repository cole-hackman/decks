//! Beat-grid arithmetic: snapping cues to the grid, and jumping by beats.
//!
//! Pure functions over a parsed ANLZ beat grid (`Vec<BeatGridEntry>`, ordered by
//! `time_ms`). Both the cue editor and Beat Jump need this, and keeping it out
//! of the UI means the behaviour is unit-testable rather than only reachable
//! through a running app.
//!
//! Everything here treats the grid as the authority — it never extrapolates a
//! tempo beyond the last marker, because a grid that stops early usually means
//! the analysis stopped early, and inventing beats past that point would place
//! cues in silence.

use crate::types::BeatGridEntry;

/// Quantise resolutions Lexicon offers, in beats.
///
/// `Bar` is 4 beats and `FourBars` is 16 — the values that matter for
/// electronic music, where phrases are built on 4- and 16-bar boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizeResolution {
    Beat,
    TwoBeats,
    Bar,
    FourBars,
    SixteenBars,
}

impl QuantizeResolution {
    pub fn beats(self) -> usize {
        match self {
            QuantizeResolution::Beat => 1,
            QuantizeResolution::TwoBeats => 2,
            QuantizeResolution::Bar => 4,
            QuantizeResolution::FourBars => 16,
            QuantizeResolution::SixteenBars => 64,
        }
    }
}

/// Index of the grid entry nearest `position_ms`.
///
/// Returns `None` for an empty grid. Ties break towards the earlier beat, which
/// keeps behaviour deterministic when a cue sits exactly between two beats.
pub fn nearest_beat_index(grid: &[BeatGridEntry], position_ms: i64) -> Option<usize> {
    if grid.is_empty() {
        return None;
    }
    let mut best = 0usize;
    let mut best_delta = i64::MAX;
    for (i, entry) in grid.iter().enumerate() {
        let delta = (entry.time_ms as i64 - position_ms).abs();
        if delta < best_delta {
            best_delta = delta;
            best = i;
        }
    }
    Some(best)
}

/// Snap a position onto the grid at the given resolution.
///
/// Resolutions coarser than one beat are measured from the **first grid
/// marker**, so a 4-beat snap lands on a downbeat as the grid defines it rather
/// than on an arbitrary beat that happens to be a multiple of four away from
/// the cue.
///
/// Returns `position_ms` unchanged when there is no grid — a track without
/// analysis should not have its cues silently moved.
pub fn snap_to_grid(
    grid: &[BeatGridEntry],
    position_ms: i64,
    resolution: QuantizeResolution,
) -> i64 {
    let Some(nearest) = nearest_beat_index(grid, position_ms) else {
        return position_ms;
    };
    let step = resolution.beats();
    if step <= 1 {
        return grid[nearest].time_ms as i64;
    }

    // Round the nearest beat index to the closest multiple of `step`, clamped
    // into the grid so we never index past the end.
    let lower = nearest - (nearest % step);
    let upper = (lower + step).min(grid.len().saturating_sub(1));
    let lower_delta = (grid[lower].time_ms as i64 - position_ms).abs();
    let upper_delta = (grid[upper].time_ms as i64 - position_ms).abs();
    if upper_delta < lower_delta {
        grid[upper].time_ms as i64
    } else {
        grid[lower].time_ms as i64
    }
}

/// Whether a position already sits on a grid marker, within `tolerance_ms`.
///
/// This is the test that decides which cues move when the beatgrid moves:
/// cues already on the grid follow it, cues deliberately placed off-grid stay
/// where the user put them.
pub fn is_on_grid(grid: &[BeatGridEntry], position_ms: i64, tolerance_ms: i64) -> bool {
    match nearest_beat_index(grid, position_ms) {
        Some(i) => (grid[i].time_ms as i64 - position_ms).abs() <= tolerance_ms,
        None => false,
    }
}

/// Move `position_ms` by `beats` along the grid. Negative jumps backwards.
///
/// Clamps at both ends rather than running off the grid, so holding the beat-jump
/// key parks the playhead at the first or last beat instead of seeking to a
/// nonsense position.
pub fn beat_jump(grid: &[BeatGridEntry], position_ms: i64, beats: i64) -> i64 {
    let Some(nearest) = nearest_beat_index(grid, position_ms) else {
        return position_ms;
    };
    let target = nearest as i64 + beats;
    let clamped = target.clamp(0, grid.len() as i64 - 1) as usize;
    grid[clamped].time_ms as i64
}

/// Shift every cue by a fixed millisecond offset, clamping at zero.
///
/// Used by the beatshift correction path and the "Shift Cues/Beatgrid" recipe.
pub fn shift_positions(positions: &[i64], offset_ms: i64) -> Vec<i64> {
    positions.iter().map(|p| (p + offset_ms).max(0)).collect()
}

/// New positions for cues after the beatgrid is moved by `offset_ms`.
///
/// Returns `(original, moved)` pairs **only for cues that were on the grid**;
/// off-grid cues are omitted, since the caller should leave them alone. This is
/// the selective behaviour Lexicon documents for quantised grid edits, and it
/// is easy to get wrong by moving everything.
pub fn cues_following_grid(
    grid: &[BeatGridEntry],
    cue_positions: &[i64],
    offset_ms: i64,
    tolerance_ms: i64,
) -> Vec<(i64, i64)> {
    cue_positions
        .iter()
        .filter(|p| is_on_grid(grid, **p, tolerance_ms))
        .map(|p| (*p, (*p + offset_ms).max(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 128 BPM → 468.75 ms per beat. Rounded to whole ms, as ANLZ stores them.
    fn grid_128(beats: usize) -> Vec<BeatGridEntry> {
        (0..beats)
            .map(|i| BeatGridEntry {
                beat_number: (i % 4) as u16 + 1,
                tempo_bpm_x100: 12800,
                time_ms: (i as f64 * 468.75).round() as u32,
            })
            .collect()
    }

    #[test]
    fn nearest_beat_on_empty_grid_is_none() {
        assert_eq!(nearest_beat_index(&[], 1000), None);
    }

    #[test]
    fn nearest_beat_picks_the_closest_marker() {
        let g = grid_128(8);
        assert_eq!(nearest_beat_index(&g, 0), Some(0));
        assert_eq!(nearest_beat_index(&g, 460), Some(1)); // 469 is closer than 0
        assert_eq!(nearest_beat_index(&g, 1000), Some(2)); // 938 vs 1406
    }

    #[test]
    fn nearest_beat_breaks_ties_towards_the_earlier_beat() {
        let g = vec![
            BeatGridEntry {
                beat_number: 1,
                tempo_bpm_x100: 12000,
                time_ms: 0,
            },
            BeatGridEntry {
                beat_number: 2,
                tempo_bpm_x100: 12000,
                time_ms: 1000,
            },
        ];
        assert_eq!(nearest_beat_index(&g, 500), Some(0));
    }

    #[test]
    fn snap_without_a_grid_leaves_the_position_alone() {
        assert_eq!(snap_to_grid(&[], 1234, QuantizeResolution::Bar), 1234);
    }

    #[test]
    fn snap_to_one_beat_lands_on_the_nearest_marker() {
        let g = grid_128(16);
        assert_eq!(snap_to_grid(&g, 460, QuantizeResolution::Beat), 469);
        assert_eq!(snap_to_grid(&g, 10, QuantizeResolution::Beat), 0);
    }

    #[test]
    fn snap_to_a_bar_lands_on_a_downbeat_measured_from_the_first_marker() {
        let g = grid_128(16);
        // Beat 5 sits at 1875ms; the nearest bar boundary is beat 4 (1406) —
        // index 4, a multiple of 4 from the first marker.
        assert_eq!(snap_to_grid(&g, 1800, QuantizeResolution::Bar), 1875);
        // Just after the start snaps back to the very first marker.
        assert_eq!(snap_to_grid(&g, 300, QuantizeResolution::Bar), 0);
    }

    #[test]
    fn snap_to_four_bars_uses_sixteen_beat_boundaries() {
        let g = grid_128(64);
        // 16 beats in = 7500ms. A cue near it snaps there rather than to a
        // nearer single beat.
        let snapped = snap_to_grid(&g, 7400, QuantizeResolution::FourBars);
        assert_eq!(snapped, 7500);
    }

    #[test]
    fn snap_clamps_at_the_end_of_a_short_grid() {
        let g = grid_128(6);
        // Asking for a 16-beat snap on a 6-beat grid must not index past the end.
        let snapped = snap_to_grid(&g, 2000, QuantizeResolution::FourBars);
        assert!(snapped <= g.last().unwrap().time_ms as i64);
    }

    #[test]
    fn resolutions_map_to_beat_counts() {
        assert_eq!(QuantizeResolution::Beat.beats(), 1);
        assert_eq!(QuantizeResolution::Bar.beats(), 4);
        assert_eq!(QuantizeResolution::FourBars.beats(), 16);
        assert_eq!(QuantizeResolution::SixteenBars.beats(), 64);
    }

    #[test]
    fn is_on_grid_respects_tolerance() {
        let g = grid_128(8);
        assert!(is_on_grid(&g, 469, 5));
        assert!(is_on_grid(&g, 472, 5));
        assert!(!is_on_grid(&g, 500, 5));
        assert!(!is_on_grid(&[], 100, 5));
    }

    #[test]
    fn beat_jump_moves_by_whole_beats_in_both_directions() {
        let g = grid_128(16);
        assert_eq!(beat_jump(&g, 0, 4), 1875);
        assert_eq!(beat_jump(&g, 1875, -4), 0);
    }

    #[test]
    fn beat_jump_clamps_at_both_ends() {
        let g = grid_128(8);
        assert_eq!(beat_jump(&g, 0, -16), 0);
        assert_eq!(beat_jump(&g, 0, 100), g.last().unwrap().time_ms as i64);
    }

    #[test]
    fn beat_jump_without_a_grid_is_a_no_op() {
        assert_eq!(beat_jump(&[], 5000, 16), 5000);
    }

    #[test]
    fn shift_clamps_at_zero() {
        assert_eq!(shift_positions(&[100, 2000], -500), vec![0, 1500]);
        assert_eq!(shift_positions(&[100], 50), vec![150]);
    }

    #[test]
    fn only_on_grid_cues_follow_a_grid_move() {
        let g = grid_128(8);
        // 469 is on the grid; 600 was deliberately placed off it.
        let moved = cues_following_grid(&g, &[469, 600], 20, 5);
        assert_eq!(moved, vec![(469, 489)]);
    }

    #[test]
    fn a_grid_move_of_zero_still_reports_on_grid_cues() {
        let g = grid_128(8);
        let moved = cues_following_grid(&g, &[0, 469], 0, 5);
        assert_eq!(moved, vec![(0, 0), (469, 469)]);
    }
}
