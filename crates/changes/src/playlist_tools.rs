//! Playlist Tools — Merge, Sort, Cross Reference, Prefix, Rewrite Order.
//!
//! Per `docs/lexicon/02-library.md §Playlist Tools`. All five are pure
//! planning: each returns what *would* change, and the caller turns that into
//! staged changes. Nothing here touches a database.
//!
//! **Rewrite Order is the reason this module matters most.** It has no visible
//! effect inside `decks` either — its entire purpose is that CDJs can only sort
//! by a handful of columns and know nothing about Energy. Sort by Energy here,
//! rewrite the order, and the playlist arrives on the gear in that order.

use serde::{Deserialize, Serialize};

/// One playlist, as the tools need to see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    /// `None` for a root-level playlist.
    pub parent_id: Option<String>,
    /// Track ids **in stored order**. Order matters to Merge and Rewrite Order.
    pub track_ids: Vec<String>,
}

// ── Merge ────────────────────────────────────────────────────────────────────

/// Combine playlists into one track list, dropping duplicates.
///
/// First occurrence wins, so the merged order follows the order the playlists
/// were given and then each playlist's own order. Lexicon says "duplicates
/// dropped" without saying which copy survives; keeping the first is the only
/// choice that makes the result a stable function of the input.
pub fn merge(playlists: &[PlaylistSummary]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in playlists {
        for id in &p.track_ids {
            if seen.insert(id.clone()) {
                out.push(id.clone());
            }
        }
    }
    out
}

// ── Sort ─────────────────────────────────────────────────────────────────────

/// How to order the playlists themselves — **not** the tracks inside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistSortMode {
    NameAsc,
    NameDesc,
    /// Most tracks first, with name as the tie-break so the result is stable.
    TrackCountDesc,
}

/// Order `playlists` by `mode`, returning their ids in the new order.
///
/// Name comparison is case-insensitive: a tree that puts `apple` after `Zebra`
/// is sorted by byte value, not alphabetically, and no one reading it will
/// believe the button worked.
pub fn sort_playlists(playlists: &[PlaylistSummary], mode: PlaylistSortMode) -> Vec<String> {
    let mut sorted: Vec<&PlaylistSummary> = playlists.iter().collect();
    let key = |p: &PlaylistSummary| p.name.to_lowercase();
    match mode {
        PlaylistSortMode::NameAsc => sorted.sort_by_key(|p| key(p)),
        PlaylistSortMode::NameDesc => sorted.sort_by_key(|p| std::cmp::Reverse(key(p))),
        PlaylistSortMode::TrackCountDesc => {
            sorted.sort_by_key(|p| (std::cmp::Reverse(p.track_ids.len()), key(p)))
        }
    }
    sorted.into_iter().map(|p| p.id.clone()).collect()
}

// ── Cross Reference ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossReferenceMode {
    /// Tracks present in every selected playlist.
    InAll,
    /// Library tracks in none of them. The spec warns this can be huge, and it
    /// is the caller's job to say so before running it.
    InNone,
}

/// Cross-reference `playlists`, against `library` when the mode needs it.
///
/// `InAll` over an empty selection returns nothing rather than everything: the
/// vacuous-truth reading ("in all zero playlists") is technically right and
/// useless, and would hand back the whole library to someone who selected
/// nothing.
///
/// Results follow `library` order so two runs agree; when `library` is not
/// consulted (`InAll`), they follow the first playlist's order.
pub fn cross_reference(
    playlists: &[PlaylistSummary],
    library: &[String],
    mode: CrossReferenceMode,
) -> Vec<String> {
    if playlists.is_empty() {
        return Vec::new();
    }
    let sets: Vec<std::collections::HashSet<&String>> = playlists
        .iter()
        .map(|p| p.track_ids.iter().collect())
        .collect();

    match mode {
        CrossReferenceMode::InAll => playlists[0]
            .track_ids
            .iter()
            .filter(|id| sets.iter().all(|s| s.contains(id)))
            .cloned()
            .collect(),
        CrossReferenceMode::InNone => library
            .iter()
            .filter(|id| !sets.iter().any(|s| s.contains(id)))
            .cloned()
            .collect(),
    }
}

// ── Prefix ───────────────────────────────────────────────────────────────────

/// Incrementing-number half of the Prefix tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Numbering {
    pub start: u32,
    /// Zero-pad to this width. `2` gives `01`, `02`, …
    #[serde(default)]
    pub pad: usize,
    /// Strip a number already at the front of the name before prefixing.
    /// Without this, running Prefix twice gives `02 01 House`.
    #[serde(default)]
    pub replace_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefixSpec {
    /// Literal text between the number and the name. Carries its own separator
    /// (`" - "`), which is why there is no separate separator field.
    #[serde(default)]
    pub text: String,
    pub numbering: Option<Numbering>,
}

/// One planned rename.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistRename {
    pub id: String,
    pub from: String,
    pub to: String,
}

/// Strip a leading run of digits and the separator that follows it.
///
/// `"01 - House"` → `"House"`; `"2024 Recap"` → `"Recap"`; `"7empest"` is left
/// alone, because the digits are not followed by a separator and taking them
/// would mangle a title.
fn strip_number_prefix(name: &str) -> &str {
    let digits = name.len() - name.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return name;
    }
    let rest = &name[digits..];
    let trimmed = rest.trim_start_matches([' ', '-', '.', '_', ')']);
    if trimmed.len() == rest.len() {
        // Digits ran straight into a letter — part of the name, not a prefix.
        return name;
    }
    if trimmed.is_empty() {
        // The whole name was a number; keep it rather than renaming to "".
        return name;
    }
    trimmed
}

/// Plan the renames, in the order given — the numbering follows that order.
///
/// A rename that would change nothing is dropped, so an empty result means
/// "nothing to do" rather than "N no-ops staged".
pub fn prefix_names(playlists: &[PlaylistSummary], spec: &PrefixSpec) -> Vec<PlaylistRename> {
    playlists
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let base = match &spec.numbering {
                Some(n) if n.replace_existing => strip_number_prefix(&p.name),
                _ => p.name.as_str(),
            };
            let number = match &spec.numbering {
                Some(n) => {
                    let value = n.start as u64 + i as u64;
                    format!("{:0width$}", value, width = n.pad)
                }
                None => String::new(),
            };
            let to = format!("{}{}{}", number, spec.text, base);
            (to != p.name).then(|| PlaylistRename {
                id: p.id.clone(),
                from: p.name.clone(),
                to,
            })
        })
        .collect()
}

// ── Rewrite Order ────────────────────────────────────────────────────────────

/// What a Rewrite Order would do, or the reason it would do nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewriteOrderPlan {
    pub playlist_id: String,
    /// Track ids in the order to store.
    pub order: Vec<String>,
    /// Ids in the requested order that are not in the playlist. The applier
    /// rejects the whole change if any survive, so they are surfaced here
    /// rather than discovered at Sync.
    pub unknown: Vec<String>,
    /// Ids in the playlist that the requested order left out. Appended to the
    /// end in their existing order — a visible sort that hides rows (a filter
    /// is active) must not silently drop them from the playlist.
    pub appended: Vec<String>,
    /// `true` when the stored order already matches.
    pub unchanged: bool,
}

/// Plan persisting `visible_order` as `playlist`'s stored order.
pub fn plan_rewrite_order(
    playlist: &PlaylistSummary,
    visible_order: &[String],
) -> RewriteOrderPlan {
    let members: std::collections::HashSet<&String> = playlist.track_ids.iter().collect();
    let mut unknown = Vec::new();
    let mut order = Vec::new();
    let mut placed = std::collections::HashSet::new();

    for id in visible_order {
        if !members.contains(id) {
            unknown.push(id.clone());
            continue;
        }
        if placed.insert(id.clone()) {
            order.push(id.clone());
        }
    }

    let appended: Vec<String> = playlist
        .track_ids
        .iter()
        .filter(|id| !placed.contains(*id))
        .cloned()
        .collect();
    order.extend(appended.iter().cloned());

    RewriteOrderPlan {
        unchanged: order == playlist.track_ids,
        playlist_id: playlist.id.clone(),
        order,
        unknown,
        appended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pl(id: &str, name: &str, tracks: &[&str]) -> PlaylistSummary {
        PlaylistSummary {
            id: id.into(),
            name: name.into(),
            parent_id: None,
            track_ids: tracks.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    // ── Merge ────────────────────────────────────────────────────────────

    #[test]
    fn merge_drops_duplicates_and_keeps_the_first_copy() {
        let got = merge(&[pl("a", "A", &["1", "2"]), pl("b", "B", &["2", "3"])]);
        assert_eq!(got, ["1", "2", "3"]);
    }

    #[test]
    fn merge_of_nothing_is_nothing() {
        assert!(merge(&[]).is_empty());
        assert!(merge(&[pl("a", "A", &[])]).is_empty());
    }

    #[test]
    fn merge_dedupes_within_one_playlist_too() {
        // Rekordbox allows the same track twice in a playlist.
        let got = merge(&[pl("a", "A", &["1", "1", "2"])]);
        assert_eq!(got, ["1", "2"]);
    }

    // ── Sort ─────────────────────────────────────────────────────────────

    #[test]
    fn sorting_by_name_is_case_insensitive() {
        let lists = vec![pl("z", "Zebra", &[]), pl("a", "apple", &[])];
        assert_eq!(
            sort_playlists(&lists, PlaylistSortMode::NameAsc),
            ["a", "z"]
        );
        assert_eq!(
            sort_playlists(&lists, PlaylistSortMode::NameDesc),
            ["z", "a"]
        );
    }

    #[test]
    fn track_count_sorts_descending_with_name_as_the_tie_break() {
        let lists = vec![
            pl("small", "Small", &["1"]),
            pl("b", "Beta", &["1", "2"]),
            pl("a", "Alpha", &["1", "2"]),
        ];
        assert_eq!(
            sort_playlists(&lists, PlaylistSortMode::TrackCountDesc),
            ["a", "b", "small"]
        );
    }

    // ── Cross Reference ──────────────────────────────────────────────────

    #[test]
    fn in_all_returns_the_intersection() {
        let lists = vec![
            pl("a", "A", &["1", "2", "3"]),
            pl("b", "B", &["2", "3", "4"]),
            pl("c", "C", &["3", "2"]),
        ];
        let got = cross_reference(&lists, &[], CrossReferenceMode::InAll);
        assert_eq!(got, ["2", "3"]);
    }

    #[test]
    fn in_none_returns_library_tracks_no_selected_playlist_holds() {
        let lists = vec![pl("a", "A", &["1"]), pl("b", "B", &["2"])];
        let library: Vec<String> = ["1", "2", "3", "4"].iter().map(|s| s.to_string()).collect();
        let got = cross_reference(&lists, &library, CrossReferenceMode::InNone);
        assert_eq!(got, ["3", "4"]);
    }

    #[test]
    fn an_empty_selection_returns_nothing_not_everything() {
        // "In all zero playlists" is vacuously the whole library. That is the
        // technically correct answer and a terrible one to hand someone who
        // selected nothing.
        let library: Vec<String> = ["1", "2"].iter().map(|s| s.to_string()).collect();
        assert!(cross_reference(&[], &library, CrossReferenceMode::InAll).is_empty());
        assert!(cross_reference(&[], &library, CrossReferenceMode::InNone).is_empty());
    }

    #[test]
    fn in_all_of_one_playlist_is_that_playlist() {
        let lists = vec![pl("a", "A", &["1", "2"])];
        assert_eq!(
            cross_reference(&lists, &[], CrossReferenceMode::InAll),
            ["1", "2"]
        );
    }

    // ── Prefix ───────────────────────────────────────────────────────────

    #[test]
    fn text_only_prefixes_every_name() {
        let lists = vec![pl("a", "House", &[]), pl("b", "Techno", &[])];
        let got = prefix_names(
            &lists,
            &PrefixSpec {
                text: "2026 ".into(),
                numbering: None,
            },
        );
        assert_eq!(
            got.iter().map(|r| r.to.as_str()).collect::<Vec<_>>(),
            ["2026 House", "2026 Techno"]
        );
    }

    #[test]
    fn numbering_increments_and_pads() {
        let lists = vec![
            pl("a", "House", &[]),
            pl("b", "Techno", &[]),
            pl("c", "Disco", &[]),
        ];
        let got = prefix_names(
            &lists,
            &PrefixSpec {
                text: " - ".into(),
                numbering: Some(Numbering {
                    start: 9,
                    pad: 2,
                    replace_existing: false,
                }),
            },
        );
        assert_eq!(
            got.iter().map(|r| r.to.as_str()).collect::<Vec<_>>(),
            ["09 - House", "10 - Techno", "11 - Disco"]
        );
    }

    #[test]
    fn replace_existing_stops_prefixes_from_stacking() {
        // Reordering an already-numbered set: without replace_existing this
        // gives "01 - 02 - Techno".
        let lists = vec![pl("b", "02 - Techno", &[]), pl("a", "01 - House", &[])];
        let spec = PrefixSpec {
            text: " - ".into(),
            numbering: Some(Numbering {
                start: 1,
                pad: 2,
                replace_existing: true,
            }),
        };
        assert_eq!(
            prefix_names(&lists, &spec)
                .iter()
                .map(|r| r.to.as_str())
                .collect::<Vec<_>>(),
            ["01 - Techno", "02 - House"]
        );
    }

    #[test]
    fn without_replace_existing_the_old_number_survives() {
        let lists = vec![pl("b", "02 - Techno", &[])];
        let got = prefix_names(
            &lists,
            &PrefixSpec {
                text: " - ".into(),
                numbering: Some(Numbering {
                    start: 1,
                    pad: 2,
                    replace_existing: false,
                }),
            },
        );
        assert_eq!(got[0].to, "01 - 02 - Techno");
    }

    #[test]
    fn renumbering_a_set_that_is_already_right_stages_nothing() {
        // The whole set already carries the numbers it would be given, so
        // there is nothing to review and nothing to sync.
        let lists = vec![pl("a", "01 - House", &[]), pl("b", "02 - Techno", &[])];
        let got = prefix_names(
            &lists,
            &PrefixSpec {
                text: " - ".into(),
                numbering: Some(Numbering {
                    start: 1,
                    pad: 2,
                    replace_existing: true,
                }),
            },
        );
        assert!(got.is_empty(), "expected no renames, got {got:?}");
    }

    #[test]
    fn a_number_that_is_part_of_the_name_is_not_stripped() {
        // "7empest" and "2 Bad Mice" are titles, not prefixes. The signal is
        // the separator: digits running straight into a letter stay.
        assert_eq!(strip_number_prefix("7empest"), "7empest");
        assert_eq!(strip_number_prefix("01 - House"), "House");
        assert_eq!(strip_number_prefix("2024 Recap"), "Recap");
        assert_eq!(strip_number_prefix("House"), "House");
        // A name that is only a number keeps it — renaming to "" is worse.
        assert_eq!(strip_number_prefix("12"), "12");
        assert_eq!(strip_number_prefix("12 - "), "12 - ");
    }

    #[test]
    fn renames_that_change_nothing_are_dropped() {
        let lists = vec![pl("a", "House", &[])];
        let got = prefix_names(
            &lists,
            &PrefixSpec {
                text: String::new(),
                numbering: None,
            },
        );
        assert!(got.is_empty());
    }

    // ── Rewrite Order ────────────────────────────────────────────────────

    #[test]
    fn rewrite_order_stores_the_visible_order() {
        let playlist = pl("p", "Set", &["1", "2", "3"]);
        let plan = plan_rewrite_order(&playlist, &["3".into(), "1".into(), "2".into()]);
        assert_eq!(plan.order, ["3", "1", "2"]);
        assert!(plan.unknown.is_empty());
        assert!(plan.appended.is_empty());
        assert!(!plan.unchanged);
    }

    #[test]
    fn tracks_the_visible_order_left_out_are_appended_not_dropped() {
        // A filter was active, so the sorted view held two of three rows.
        // Storing only those would silently remove the third from the playlist.
        let playlist = pl("p", "Set", &["1", "2", "3"]);
        let plan = plan_rewrite_order(&playlist, &["3".into(), "1".into()]);
        assert_eq!(plan.order, ["3", "1", "2"]);
        assert_eq!(plan.appended, ["2"]);
    }

    #[test]
    fn ids_not_in_the_playlist_are_reported_rather_than_stored() {
        // The applier rejects the whole change on an unknown id, so it has to
        // surface at preview time, not at Sync.
        let playlist = pl("p", "Set", &["1", "2"]);
        let plan = plan_rewrite_order(&playlist, &["1".into(), "99".into(), "2".into()]);
        assert_eq!(plan.order, ["1", "2"]);
        assert_eq!(plan.unknown, ["99"]);
    }

    #[test]
    fn a_repeated_id_in_the_visible_order_is_placed_once() {
        let playlist = pl("p", "Set", &["1", "2"]);
        let plan = plan_rewrite_order(&playlist, &["2".into(), "2".into(), "1".into()]);
        assert_eq!(plan.order, ["2", "1"]);
    }

    #[test]
    fn an_order_that_matches_the_stored_one_is_flagged_unchanged() {
        let playlist = pl("p", "Set", &["1", "2", "3"]);
        let plan = plan_rewrite_order(&playlist, &["1".into(), "2".into(), "3".into()]);
        assert!(plan.unchanged);
    }

    #[test]
    fn an_empty_visible_order_leaves_the_playlist_alone() {
        let playlist = pl("p", "Set", &["1", "2"]);
        let plan = plan_rewrite_order(&playlist, &[]);
        assert_eq!(plan.order, ["1", "2"]);
        assert!(plan.unchanged);
    }
}
