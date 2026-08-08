//! Cue recipes.
//!
//! The spec calls this category the most valuable and the furthest from
//! anything `decks` had. It operates on a track's cue list rather than its text
//! fields, so it gets its own model — but the same shape as everything else
//! here: a pure function from (recipe, cues) to a new cue list.
//!
//! Beat-grid-dependent operations take the grid as an argument rather than
//! reading it, so the whole category stays testable without an ANLZ file.
//!
//! See `docs/lexicon/10-recipes.md §Cue point recipes`.

use serde::{Deserialize, Serialize};

/// The subset of a cue these operations need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeCue {
    pub id: String,
    pub position_ms: i64,
    /// Present when the cue is a loop.
    #[serde(default)]
    pub loop_end_ms: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
    /// Memory cues are the Rekordbox-only kind; hot cues occupy slots.
    #[serde(default)]
    pub memory: bool,
}

impl RecipeCue {
    pub fn is_loop(&self) -> bool {
        self.loop_end_ms.is_some()
    }

    fn has_text(&self) -> bool {
        self.name
            .as_deref()
            .map(str::trim)
            .is_some_and(|n| !n.is_empty())
    }
}

/// Which cues `DeleteCues` removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    /// Everything — cues and loops alike.
    All,
    First,
    Last,
    /// Delete everything *except* the first.
    KeepFirst,
    KeepLast,
    LoopsOnly,
    WithoutColour,
    WithoutText,
    MemoryCues,
}

/// Colour schemes for `ChangeColours`.
///
/// The palettes are deliberately short and cycle: Rekordbox has a fixed set of
/// cue colours, and a scheme's job is to make a track's cues visually
/// distinguishable rather than to be a full colour space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColourScheme {
    Basic,
    Grayscale,
    Cold,
    Warm,
    /// Cycles the full palette, so no two adjacent cues repeat — the spec's
    /// "never repeats a colour". Deterministic rather than actually random, so
    /// a preview matches what apply does.
    Cycle,
    /// Strip all colours.
    None,
    /// Every cue takes the first cue's colour.
    FirstCueColour,
}

impl ColourScheme {
    fn palette(self) -> &'static [i64] {
        match self {
            ColourScheme::Basic => &[1, 2, 3, 4],
            ColourScheme::Grayscale => &[10, 11, 12],
            ColourScheme::Cold => &[5, 6, 7],
            ColourScheme::Warm => &[1, 2, 8],
            ColourScheme::Cycle => &[1, 2, 3, 4, 5, 6, 7, 8],
            ColourScheme::None | ColourScheme::FirstCueColour => &[],
        }
    }
}

/// Orderings for `SortCues`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    TimeAsc,
    TimeDesc,
    LabelAsc,
    LabelDesc,
    EmptyLabelsFirst,
    EmptyLabelsLast,
    CuesBeforeLoops,
    LoopsBeforeCues,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CueRecipe {
    DeleteCues {
        mode: DeleteMode,
    },
    ChangeColours {
        scheme: ColourScheme,
    },
    /// Empty match text matches *untitled* cues; `*` matches any text. Empty
    /// match colour matches *uncoloured* cues. An empty replacement keeps the
    /// existing value — all four rules verbatim from the spec.
    FindAndReplace {
        #[serde(default)]
        match_text: Option<String>,
        #[serde(default)]
        match_colour: Option<i64>,
        #[serde(default)]
        new_text: Option<String>,
        #[serde(default)]
        new_colour: Option<i64>,
    },
    SortCues {
        order: SortOrder,
    },
    ReplaceCueText {
        find: String,
        replace: String,
        #[serde(default)]
        case_insensitive: bool,
    },
    RemoveCueText,
    RemoveCuesByLabel {
        text: String,
    },
    /// The manual remedy for beatshift. Loops move with their cue.
    ShiftCues {
        offset_ms: i64,
    },
    /// Snap to the nearest grid marker. A no-op without a grid, reported as
    /// such rather than silently moving nothing.
    QuantizeCues {
        /// Beats per snap: 1, 2, 4 (a bar), 16, 64.
        resolution_beats: u32,
    },
    /// Copy cues into the other Rekordbox cue kind.
    ///
    /// Per `docs/lexicon/01-interop.md §Cue Destination`: the sync options
    /// `All to hot cue` / `All to memory cue` / `All to hot and memory cue`,
    /// "which is how you copy hot cues into memory cues wholesale". This is the
    /// half of Cue Destination that `decks` can act on — see the doc comment on
    /// [`MirrorTarget`] for the half it does not need.
    MirrorCues {
        target: MirrorTarget,
    },
}

/// Which kind a track's cues should exist as after `MirrorCues`.
///
/// **`decks` needs no hidden-duplicate model.** Lexicon collapses memory cues
/// into hot cues on import and must remember what it hid so it can restore them
/// on sync back. `decks` never imports — it reads `djmdCue` live and shows both
/// kinds as they are — so there is nothing hidden and nothing to restore. The
/// round-trip guarantee is one we get by not having the problem, and the
/// buildable half of the feature is this bulk copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorTarget {
    /// Every cue becomes a hot cue. Memory cues are converted, not duplicated.
    Hot,
    /// Every cue becomes a memory cue.
    Memory,
    /// Every cue exists as **both**. This is the one people actually want:
    /// hot cues do not show on every player, memory cues do.
    Both,
}

/// What a cue recipe produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueEdits {
    /// The cue list after the recipe, in its new order.
    pub cues: Vec<RecipeCue>,
    /// Ids the recipe removed.
    pub deleted: Vec<String>,
    /// Set when the recipe could not run.
    pub skipped: Option<String>,
}

fn sort_key_label(cue: &RecipeCue) -> String {
    cue.name.clone().unwrap_or_default().to_lowercase()
}

/// Snap a position to the nearest grid marker at the given beat resolution.
///
/// The grid is a list of beat times in milliseconds. Only every `resolution`th
/// marker is a candidate, measured from the first — snapping to "4 beats" means
/// bar lines, not any beat.
fn snap(position_ms: i64, grid: &[i64], resolution_beats: u32) -> i64 {
    let step = resolution_beats.max(1) as usize;
    let candidates: Vec<i64> = grid.iter().copied().step_by(step).collect();
    candidates
        .into_iter()
        .min_by_key(|marker| (marker - position_ms).abs())
        .unwrap_or(position_ms)
}

/// Run a cue recipe.
///
/// `grid` is only consulted by `QuantizeCues`; pass an empty slice otherwise.
pub fn apply_cue_recipe(recipe: &CueRecipe, cues: &[RecipeCue], grid: &[i64]) -> CueEdits {
    let mut out = cues.to_vec();
    let mut deleted = Vec::new();
    let mut skipped = None;

    // Time order is the reference for "first" and "last" — the stored order is
    // an implementation detail and users mean the track's timeline.
    let mut by_time: Vec<usize> = (0..out.len()).collect();
    by_time.sort_by_key(|&i| out[i].position_ms);

    match recipe {
        CueRecipe::DeleteCues { mode } => {
            let doomed: Vec<String> = match mode {
                DeleteMode::All => out.iter().map(|c| c.id.clone()).collect(),
                DeleteMode::First => by_time
                    .first()
                    .map(|&i| out[i].id.clone())
                    .into_iter()
                    .collect(),
                DeleteMode::Last => by_time
                    .last()
                    .map(|&i| out[i].id.clone())
                    .into_iter()
                    .collect(),
                DeleteMode::KeepFirst => {
                    by_time.iter().skip(1).map(|&i| out[i].id.clone()).collect()
                }
                DeleteMode::KeepLast => by_time
                    .iter()
                    .take(by_time.len().saturating_sub(1))
                    .map(|&i| out[i].id.clone())
                    .collect(),
                DeleteMode::LoopsOnly => out
                    .iter()
                    .filter(|c| c.is_loop())
                    .map(|c| c.id.clone())
                    .collect(),
                DeleteMode::WithoutColour => out
                    .iter()
                    .filter(|c| c.color.is_none())
                    .map(|c| c.id.clone())
                    .collect(),
                DeleteMode::WithoutText => out
                    .iter()
                    .filter(|c| !c.has_text())
                    .map(|c| c.id.clone())
                    .collect(),
                DeleteMode::MemoryCues => out
                    .iter()
                    .filter(|c| c.memory)
                    .map(|c| c.id.clone())
                    .collect(),
            };
            out.retain(|c| !doomed.contains(&c.id));
            deleted = doomed;
        }

        CueRecipe::ChangeColours { scheme } => match scheme {
            ColourScheme::None => {
                for cue in &mut out {
                    cue.color = None;
                }
            }
            ColourScheme::FirstCueColour => {
                let first = by_time.first().and_then(|&i| out[i].color);
                for cue in &mut out {
                    cue.color = first;
                }
            }
            other => {
                let palette = other.palette();
                // Assigned in time order, so the colours read left to right on
                // the waveform rather than in storage order.
                for (n, &i) in by_time.iter().enumerate() {
                    out[i].color = Some(palette[n % palette.len()]);
                }
            }
        },

        CueRecipe::FindAndReplace {
            match_text,
            match_colour,
            new_text,
            new_colour,
        } => {
            for cue in &mut out {
                let text_ok = match match_text.as_deref() {
                    None => true,
                    Some("*") => true,
                    // Empty match text means "untitled cues".
                    Some("") => !cue.has_text(),
                    Some(want) => cue
                        .name
                        .as_deref()
                        .is_some_and(|n| n.trim().eq_ignore_ascii_case(want.trim())),
                };
                // A colour of None in the rule means "uncoloured cues", which
                // is why this is Option<i64> rather than a sentinel.
                let colour_ok = match match_colour {
                    None => true,
                    Some(want) => cue.color == Some(*want),
                };
                if !(text_ok && colour_ok) {
                    continue;
                }
                // An empty replacement keeps the existing value.
                if let Some(text) = new_text.as_deref().filter(|t| !t.is_empty()) {
                    cue.name = Some(text.to_string());
                }
                if let Some(colour) = new_colour {
                    cue.color = Some(*colour);
                }
            }
        }

        CueRecipe::SortCues { order } => match order {
            SortOrder::TimeAsc => out.sort_by_key(|c| c.position_ms),
            SortOrder::TimeDesc => out.sort_by_key(|c| std::cmp::Reverse(c.position_ms)),
            SortOrder::LabelAsc => out.sort_by_key(sort_key_label),
            SortOrder::LabelDesc => out.sort_by_key(|c| std::cmp::Reverse(sort_key_label(c))),
            // Secondary sort by time throughout, so the result is stable and
            // reads sensibly within each group.
            SortOrder::EmptyLabelsFirst => out.sort_by_key(|c| (c.has_text(), c.position_ms)),
            SortOrder::EmptyLabelsLast => out.sort_by_key(|c| (!c.has_text(), c.position_ms)),
            SortOrder::CuesBeforeLoops => out.sort_by_key(|c| (c.is_loop(), c.position_ms)),
            SortOrder::LoopsBeforeCues => out.sort_by_key(|c| (!c.is_loop(), c.position_ms)),
        },

        CueRecipe::ReplaceCueText {
            find,
            replace,
            case_insensitive,
        } => {
            for cue in &mut out {
                if let Some(name) = cue.name.clone() {
                    let next = crate::text::replace_text(&name, find, replace, *case_insensitive);
                    cue.name = if next.trim().is_empty() {
                        None
                    } else {
                        Some(next)
                    };
                }
            }
        }

        CueRecipe::RemoveCueText => {
            for cue in &mut out {
                cue.name = None;
            }
        }

        CueRecipe::RemoveCuesByLabel { text } => {
            if text.trim().is_empty() {
                // An empty needle would match every cue with a name, which is
                // Remove Cue Text's job, not this one.
                skipped = Some("a label to match is required".into());
            } else {
                let needle = text.to_lowercase();
                let doomed: Vec<String> = out
                    .iter()
                    .filter(|c| {
                        c.name
                            .as_deref()
                            .is_some_and(|n| n.to_lowercase().contains(&needle))
                    })
                    .map(|c| c.id.clone())
                    .collect();
                out.retain(|c| !doomed.contains(&c.id));
                deleted = doomed;
            }
        }

        CueRecipe::ShiftCues { offset_ms } => {
            for cue in &mut out {
                // Clamped at zero: a cue before the start of the track is not
                // representable, and wrapping negative would be worse.
                cue.position_ms = (cue.position_ms + offset_ms).max(0);
                // A loop moves whole — shifting only its start would silently
                // change its length.
                if let Some(end) = cue.loop_end_ms {
                    cue.loop_end_ms = Some((end + offset_ms).max(0));
                }
            }
        }

        CueRecipe::QuantizeCues { resolution_beats } => {
            if grid.is_empty() {
                skipped = Some("this track has no beat grid".into());
            } else {
                for cue in &mut out {
                    let before = cue.position_ms;
                    cue.position_ms = snap(before, grid, *resolution_beats);
                    // Preserve loop length rather than snapping both ends
                    // independently, which would stretch the loop.
                    if let Some(end) = cue.loop_end_ms {
                        cue.loop_end_ms = Some(end + (cue.position_ms - before));
                    }
                }
            }
        }

        CueRecipe::MirrorCues { target } => match target {
            MirrorTarget::Hot => out.iter_mut().for_each(|c| c.memory = false),
            MirrorTarget::Memory => out.iter_mut().for_each(|c| c.memory = true),
            MirrorTarget::Both => {
                // A position that already exists as both kinds is left alone.
                // Without this, running the recipe twice doubles the cue list —
                // and a bulk operation people run after every session has to be
                // safe to run again.
                let mut added = Vec::new();
                for cue in &out {
                    let twin_exists = out.iter().any(|other| {
                        other.position_ms == cue.position_ms && other.memory != cue.memory
                    });
                    if twin_exists {
                        continue;
                    }
                    added.push(RecipeCue {
                        // A new row, so a new id. The applier inserts anything
                        // it has not seen; reusing the id would make this an
                        // edit of the original instead of a copy.
                        id: format!("{}-mirror", cue.id),
                        memory: !cue.memory,
                        ..cue.clone()
                    });
                }
                if added.is_empty() {
                    skipped = Some("every cue already exists as both kinds".into());
                }
                out.extend(added);
            }
        },
    }

    CueEdits {
        cues: out,
        deleted,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(id: &str, pos: i64, name: Option<&str>, color: Option<i64>) -> RecipeCue {
        RecipeCue {
            id: id.into(),
            position_ms: pos,
            loop_end_ms: None,
            name: name.map(String::from),
            color,
            memory: false,
        }
    }

    fn looped(id: &str, pos: i64, end: i64) -> RecipeCue {
        RecipeCue {
            loop_end_ms: Some(end),
            ..cue(id, pos, None, None)
        }
    }

    fn three() -> Vec<RecipeCue> {
        vec![
            cue("a", 1000, Some("Intro"), Some(1)),
            cue("b", 2000, None, None),
            cue("c", 3000, Some("Drop"), Some(4)),
        ]
    }

    fn run(recipe: CueRecipe, cues: &[RecipeCue]) -> CueEdits {
        apply_cue_recipe(&recipe, cues, &[])
    }

    fn ids(edits: &CueEdits) -> Vec<&str> {
        edits.cues.iter().map(|c| c.id.as_str()).collect()
    }

    // ── delete ──────────────────────────────────────────────────────────────

    #[test]
    fn delete_modes_pick_the_right_cues() {
        let c = three();
        assert!(run(
            CueRecipe::DeleteCues {
                mode: DeleteMode::All
            },
            &c
        )
        .cues
        .is_empty());
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::First
                },
                &c
            )),
            vec!["b", "c"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::Last
                },
                &c
            )),
            vec!["a", "b"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::KeepFirst
                },
                &c
            )),
            vec!["a"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::KeepLast
                },
                &c
            )),
            vec!["c"]
        );
    }

    #[test]
    fn first_and_last_mean_first_and_last_in_the_track_not_in_storage() {
        // Stored out of order on purpose.
        let c = vec![cue("late", 9000, None, None), cue("early", 100, None, None)];
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::First
                },
                &c
            )),
            vec!["late"]
        );
    }

    #[test]
    fn delete_without_colour_or_text_targets_the_right_ones() {
        let c = three();
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::WithoutColour
                },
                &c
            )),
            vec!["a", "c"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::WithoutText
                },
                &c
            )),
            vec!["a", "c"]
        );
    }

    #[test]
    fn a_whitespace_only_name_counts_as_no_text() {
        let c = vec![cue("a", 1, Some("   "), None)];
        assert!(run(
            CueRecipe::DeleteCues {
                mode: DeleteMode::WithoutText
            },
            &c
        )
        .cues
        .is_empty());
    }

    #[test]
    fn delete_loops_only_leaves_plain_cues() {
        let c = vec![cue("a", 1000, None, None), looped("b", 2000, 4000)];
        assert_eq!(
            ids(&run(
                CueRecipe::DeleteCues {
                    mode: DeleteMode::LoopsOnly
                },
                &c
            )),
            vec!["a"]
        );
    }

    #[test]
    fn deleting_from_an_empty_cue_list_is_a_no_op() {
        let got = run(
            CueRecipe::DeleteCues {
                mode: DeleteMode::First,
            },
            &[],
        );
        assert!(got.cues.is_empty());
        assert!(got.deleted.is_empty());
    }

    // ── colours ─────────────────────────────────────────────────────────────

    #[test]
    fn a_scheme_assigns_in_time_order() {
        let got = run(
            CueRecipe::ChangeColours {
                scheme: ColourScheme::Basic,
            },
            &three(),
        );
        let colours: Vec<_> = got.cues.iter().map(|c| c.color).collect();
        assert_eq!(colours, vec![Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn cycle_never_repeats_until_the_palette_runs_out() {
        let cues: Vec<_> = (0..8)
            .map(|i| cue(&format!("c{i}"), i * 1000, None, None))
            .collect();
        let got = run(
            CueRecipe::ChangeColours {
                scheme: ColourScheme::Cycle,
            },
            &cues,
        );
        let colours: Vec<_> = got.cues.iter().filter_map(|c| c.color).collect();
        let unique: std::collections::HashSet<_> = colours.iter().collect();
        assert_eq!(unique.len(), colours.len());
    }

    #[test]
    fn no_colours_strips_them_all() {
        let got = run(
            CueRecipe::ChangeColours {
                scheme: ColourScheme::None,
            },
            &three(),
        );
        assert!(got.cues.iter().all(|c| c.color.is_none()));
    }

    #[test]
    fn first_cue_colour_paints_everything_with_it() {
        let got = run(
            CueRecipe::ChangeColours {
                scheme: ColourScheme::FirstCueColour,
            },
            &three(),
        );
        assert!(got.cues.iter().all(|c| c.color == Some(1)));
    }

    // ── find & replace ──────────────────────────────────────────────────────

    #[test]
    fn empty_match_text_means_untitled_cues() {
        let got = run(
            CueRecipe::FindAndReplace {
                match_text: Some(String::new()),
                match_colour: None,
                new_text: Some("Unnamed".into()),
                new_colour: None,
            },
            &three(),
        );
        assert_eq!(got.cues[1].name.as_deref(), Some("Unnamed"));
        assert_eq!(got.cues[0].name.as_deref(), Some("Intro"));
    }

    #[test]
    fn a_star_matches_any_text() {
        let got = run(
            CueRecipe::FindAndReplace {
                match_text: Some("*".into()),
                match_colour: None,
                new_text: None,
                new_colour: Some(9),
            },
            &three(),
        );
        assert!(got.cues.iter().all(|c| c.color == Some(9)));
    }

    #[test]
    fn an_empty_replacement_keeps_the_existing_value() {
        let got = run(
            CueRecipe::FindAndReplace {
                match_text: Some("Intro".into()),
                match_colour: None,
                new_text: Some(String::new()),
                new_colour: Some(7),
            },
            &three(),
        );
        assert_eq!(got.cues[0].name.as_deref(), Some("Intro"));
        assert_eq!(got.cues[0].color, Some(7));
    }

    #[test]
    fn matching_on_colour_narrows_the_set() {
        let got = run(
            CueRecipe::FindAndReplace {
                match_text: Some("*".into()),
                match_colour: Some(4),
                new_text: Some("Renamed".into()),
                new_colour: None,
            },
            &three(),
        );
        assert_eq!(got.cues[2].name.as_deref(), Some("Renamed"));
        assert_eq!(got.cues[0].name.as_deref(), Some("Intro"));
    }

    // ── sort ────────────────────────────────────────────────────────────────

    #[test]
    fn sorting_by_time_works_both_ways() {
        let c = vec![cue("b", 2000, None, None), cue("a", 1000, None, None)];
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::TimeAsc
                },
                &c
            )),
            vec!["a", "b"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::TimeDesc
                },
                &c
            )),
            vec!["b", "a"]
        );
    }

    #[test]
    fn sorting_by_label_is_case_insensitive() {
        let c = vec![
            cue("b", 1, Some("beta"), None),
            cue("a", 2, Some("Alpha"), None),
        ];
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::LabelAsc
                },
                &c
            )),
            vec!["a", "b"]
        );
    }

    #[test]
    fn empty_labels_can_go_first_or_last_and_stay_time_ordered_within_a_group() {
        let c = three();
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::EmptyLabelsFirst
                },
                &c
            )),
            vec!["b", "a", "c"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::EmptyLabelsLast
                },
                &c
            )),
            vec!["a", "c", "b"]
        );
    }

    #[test]
    fn cues_and_loops_can_be_grouped_either_way() {
        let c = vec![looped("loop", 500, 1500), cue("cue", 1000, None, None)];
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::CuesBeforeLoops
                },
                &c
            )),
            vec!["cue", "loop"]
        );
        assert_eq!(
            ids(&run(
                CueRecipe::SortCues {
                    order: SortOrder::LoopsBeforeCues
                },
                &c
            )),
            vec!["loop", "cue"]
        );
    }

    // ── text ────────────────────────────────────────────────────────────────

    #[test]
    fn replacing_cue_text_reuses_the_shared_text_engine() {
        let got = run(
            CueRecipe::ReplaceCueText {
                find: "intro".into(),
                replace: "Start".into(),
                case_insensitive: true,
            },
            &three(),
        );
        assert_eq!(got.cues[0].name.as_deref(), Some("Start"));
    }

    #[test]
    fn replacing_a_whole_name_with_nothing_clears_it_rather_than_leaving_blank() {
        let got = run(
            CueRecipe::ReplaceCueText {
                find: "Intro".into(),
                replace: String::new(),
                case_insensitive: false,
            },
            &three(),
        );
        assert!(got.cues[0].name.is_none());
    }

    #[test]
    fn remove_cue_text_strips_every_name() {
        let got = run(CueRecipe::RemoveCueText, &three());
        assert!(got.cues.iter().all(|c| c.name.is_none()));
    }

    #[test]
    fn remove_by_label_matches_a_substring_case_insensitively() {
        let got = run(
            CueRecipe::RemoveCuesByLabel { text: "dro".into() },
            &three(),
        );
        assert_eq!(ids(&got), vec!["a", "b"]);
        assert_eq!(got.deleted, vec!["c"]);
    }

    #[test]
    fn remove_by_label_with_an_empty_needle_is_refused() {
        // It would match every named cue, which is Remove Cue Text's job.
        let got = run(CueRecipe::RemoveCuesByLabel { text: "  ".into() }, &three());
        assert!(got.skipped.is_some());
        assert_eq!(got.cues.len(), 3);
    }

    // ── shift and quantize ──────────────────────────────────────────────────

    #[test]
    fn shifting_moves_every_cue_and_takes_loops_whole() {
        let c = vec![looped("l", 1000, 3000)];
        let got = run(CueRecipe::ShiftCues { offset_ms: 250 }, &c);
        assert_eq!(got.cues[0].position_ms, 1250);
        // Length preserved: shifting only the start would silently resize it.
        assert_eq!(got.cues[0].loop_end_ms, Some(3250));
    }

    #[test]
    fn shifting_before_the_start_of_the_track_clamps_at_zero() {
        let c = vec![cue("a", 100, None, None)];
        let got = run(CueRecipe::ShiftCues { offset_ms: -500 }, &c);
        assert_eq!(got.cues[0].position_ms, 0);
    }

    #[test]
    fn quantizing_snaps_to_the_nearest_marker() {
        let grid: Vec<i64> = (0..8).map(|i| i * 500).collect();
        let c = vec![cue("a", 1100, None, None)];
        let got = apply_cue_recipe(
            &CueRecipe::QuantizeCues {
                resolution_beats: 1,
            },
            &c,
            &grid,
        );
        assert_eq!(got.cues[0].position_ms, 1000);
    }

    #[test]
    fn a_four_beat_resolution_snaps_to_bars_not_beats() {
        let grid: Vec<i64> = (0..9).map(|i| i * 500).collect();
        let c = vec![cue("a", 1100, None, None)];
        let got = apply_cue_recipe(
            &CueRecipe::QuantizeCues {
                resolution_beats: 4,
            },
            &c,
            &grid,
        );
        // Bars are at 0 and 2000; 1100 is nearer 2000.
        assert_eq!(got.cues[0].position_ms, 2000);
    }

    #[test]
    fn quantizing_preserves_loop_length_rather_than_stretching_it() {
        let grid: Vec<i64> = (0..8).map(|i| i * 500).collect();
        let c = vec![looped("l", 1100, 2100)];
        let got = apply_cue_recipe(
            &CueRecipe::QuantizeCues {
                resolution_beats: 1,
            },
            &c,
            &grid,
        );
        assert_eq!(got.cues[0].position_ms, 1000);
        assert_eq!(got.cues[0].loop_end_ms, Some(2000));
    }

    #[test]
    fn quantizing_without_a_grid_says_so_rather_than_doing_nothing() {
        let got = run(
            CueRecipe::QuantizeCues {
                resolution_beats: 4,
            },
            &three(),
        );
        assert_eq!(got.skipped.as_deref(), Some("this track has no beat grid"));
        assert_eq!(got.cues, three());
    }

    #[test]
    fn cue_recipes_round_trip_through_json() {
        let recipes = vec![
            CueRecipe::DeleteCues {
                mode: DeleteMode::KeepFirst,
            },
            CueRecipe::ChangeColours {
                scheme: ColourScheme::Cycle,
            },
            CueRecipe::SortCues {
                order: SortOrder::EmptyLabelsLast,
            },
            CueRecipe::ShiftCues { offset_ms: -12 },
            CueRecipe::QuantizeCues {
                resolution_beats: 16,
            },
        ];
        let json = serde_json::to_string(&recipes).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<CueRecipe>>(&json).unwrap(),
            recipes
        );
    }

    fn hot(id: &str, ms: i64) -> RecipeCue {
        RecipeCue {
            id: id.into(),
            position_ms: ms,
            loop_end_ms: None,
            name: Some("Drop".into()),
            color: Some(4),
            memory: false,
        }
    }

    fn mem(id: &str, ms: i64) -> RecipeCue {
        RecipeCue {
            memory: true,
            ..hot(id, ms)
        }
    }

    fn mirror(target: MirrorTarget, cues: &[RecipeCue]) -> CueEdits {
        apply_cue_recipe(&CueRecipe::MirrorCues { target }, cues, &[])
    }

    #[test]
    fn mirroring_to_memory_converts_rather_than_duplicating() {
        let out = mirror(MirrorTarget::Memory, &[hot("a", 1000), mem("b", 2000)]);
        assert_eq!(out.cues.len(), 2);
        assert!(out.cues.iter().all(|c| c.memory));
    }

    #[test]
    fn mirroring_to_hot_converts_rather_than_duplicating() {
        let out = mirror(MirrorTarget::Hot, &[hot("a", 1000), mem("b", 2000)]);
        assert_eq!(out.cues.len(), 2);
        assert!(out.cues.iter().all(|c| !c.memory));
    }

    #[test]
    fn mirroring_to_both_copies_each_cue_into_the_other_kind() {
        // The one people actually want: hot cues do not show on every player,
        // memory cues do.
        let out = mirror(MirrorTarget::Both, &[hot("a", 1000)]);
        assert_eq!(out.cues.len(), 2);
        assert!(out.cues.iter().any(|c| c.memory));
        assert!(out.cues.iter().any(|c| !c.memory));
        // The copy keeps everything but the kind and the id.
        let copy = out.cues.iter().find(|c| c.memory).unwrap();
        assert_eq!(copy.position_ms, 1000);
        assert_eq!(copy.name.as_deref(), Some("Drop"));
        assert_eq!(copy.color, Some(4));
        assert_ne!(copy.id, "a");
    }

    /// The guard that makes this safe to run after every session.
    #[test]
    fn mirroring_to_both_is_idempotent() {
        // Without it, a second run doubles the cue list — and this is a bulk
        // operation people run repeatedly.
        let once = mirror(MirrorTarget::Both, &[hot("a", 1000)]);
        let twice = mirror(MirrorTarget::Both, &once.cues);
        assert_eq!(twice.cues.len(), once.cues.len());
    }

    #[test]
    fn a_position_that_already_has_both_kinds_is_left_alone() {
        let out = mirror(MirrorTarget::Both, &[hot("a", 1000), mem("b", 1000)]);
        assert_eq!(out.cues.len(), 2);
        assert_eq!(
            out.skipped.as_deref(),
            Some("every cue already exists as both kinds")
        );
    }

    #[test]
    fn mirroring_carries_a_loop_across() {
        let mut loop_cue = hot("a", 1000);
        loop_cue.loop_end_ms = Some(3000);
        let out = mirror(MirrorTarget::Both, &[loop_cue]);
        let copy = out.cues.iter().find(|c| c.memory).unwrap();
        assert_eq!(copy.loop_end_ms, Some(3000));
    }

    #[test]
    fn mirroring_an_empty_cue_list_deletes_nothing_and_says_so() {
        let out = mirror(MirrorTarget::Both, &[]);
        assert!(out.cues.is_empty());
        assert!(out.deleted.is_empty());
        assert!(out.skipped.is_some());
    }
}
