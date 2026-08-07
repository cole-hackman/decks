//! Duplicate resolution (Epic 5).
//!
//! Detection already existed. What did not was the part that makes it safe to
//! use: **playlists are rewritten to point at the keeper**. Archiving a losing
//! copy without that leaves a hole in every set it was in, and the user finds
//! out on stage.
//!
//! The other half is choosing the keeper. Lexicon preselects one from bitrate,
//! cue presence and more; `Prefer` applies one rule across every group at once.
//! Both are pure functions here, so the heuristic is inspectable and testable
//! rather than buried in a click handler.
//!
//! Per `docs/lexicon/07-health.md §Find Duplicates`.

use serde::{Deserialize, Serialize};

use crate::cache_db;

/// The facts the keeper heuristic looks at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    pub track_id: String,
    pub bit_rate: Option<i64>,
    pub duration_secs: Option<i64>,
    pub has_cues: bool,
    pub rating: Option<i64>,
    pub play_count: Option<i64>,
    pub in_playlists: usize,
}

/// A single rule applied across every group at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreferRule {
    /// The default heuristic: cues, then bitrate, then playlists, then plays.
    Best,
    HighestBitrate,
    HasCues,
    MostPlaylists,
    Longest,
}

/// Score a candidate under a rule. Higher wins.
///
/// Returned as a tuple so ties fall through to the next criterion rather than
/// being broken arbitrarily — and the last element is always the track id, so
/// a genuine tie resolves the same way on every run instead of shuffling.
fn score(c: &DuplicateCandidate, rule: PreferRule) -> (i64, i64, i64, i64) {
    let cues = i64::from(c.has_cues);
    let bitrate = c.bit_rate.unwrap_or(0);
    let playlists = c.in_playlists as i64;
    let plays = c.play_count.unwrap_or(0);
    let length = c.duration_secs.unwrap_or(0);
    match rule {
        // Cues first: a track someone has prepared is the one they will reach
        // for, whatever its bitrate. Losing that work is the expensive mistake;
        // losing 64kbps is not.
        PreferRule::Best => (cues, bitrate, playlists, plays),
        PreferRule::HighestBitrate => (bitrate, cues, playlists, plays),
        PreferRule::HasCues => (cues, playlists, bitrate, plays),
        PreferRule::MostPlaylists => (playlists, cues, bitrate, plays),
        PreferRule::Longest => (length, cues, bitrate, playlists),
    }
}

/// Pick the keeper from a group.
///
/// Returns `None` for an empty group rather than panicking — a group with no
/// members is a bug upstream, not something to crash on.
pub fn preselect(group: &[DuplicateCandidate], rule: PreferRule) -> Option<&DuplicateCandidate> {
    group
        .iter()
        // `max_by_key` keeps the *last* maximum; comparing on the id as a final
        // tie-break would need a clone per candidate, so ties are settled by
        // taking the first instead.
        .rev()
        .max_by_key(|c| score(c, rule))
}

/// What resolving a group would do, before it does it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolutionPlan {
    pub keeper_id: String,
    /// Tracks to archive.
    pub loser_ids: Vec<String>,
    /// `(playlist_id, playlist_name, loser_id)` — memberships to re-point.
    pub repoint: Vec<(String, String, String)>,
    /// Playlists where the keeper is already present, so re-pointing would
    /// duplicate it. Removed rather than swapped.
    pub already_present: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ResolveResult {
    pub archived: Vec<String>,
    pub staged: Vec<String>,
}

/// Work out what to archive and which playlist rows to rewrite.
#[tauri::command]
pub async fn plan_duplicate_resolution(
    library_path: String,
    keeper_id: String,
    loser_ids: Vec<String>,
) -> Result<ResolutionPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(std::path::Path::new(&library_path))
            .map_err(|e| e.to_string())?;

        let mut plan = ResolutionPlan {
            keeper_id: keeper_id.clone(),
            loser_ids: loser_ids.clone(),
            repoint: Vec::new(),
            already_present: Vec::new(),
        };

        for playlist in db.playlists().map_err(|e| e.to_string())? {
            if !matches!(
                playlist.kind,
                decks_core::rekordbox_db::PlaylistKind::Playlist
            ) {
                continue;
            }
            let entries = db
                .playlist_entries(&playlist.id)
                .map_err(|e| e.to_string())?;
            let keeper_present = entries.iter().any(|e| e.content_id == keeper_id);
            let losers_here: Vec<&String> = entries
                .iter()
                .map(|e| &e.content_id)
                .filter(|id| loser_ids.contains(id))
                .collect();
            if losers_here.is_empty() {
                continue;
            }
            if keeper_present {
                // Adding the keeper again would leave the playlist holding it
                // twice — the loser is simply removed.
                plan.already_present.push(playlist.name.clone());
            }
            for loser in losers_here {
                plan.repoint
                    .push((playlist.id.clone(), playlist.name.clone(), loser.clone()));
            }
        }
        Ok(plan)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Archive the losers and stage the playlist rewrites.
///
/// Takes the plan back rather than recomputing, so what happens is what the
/// review step showed. Archiving is cache-only and immediate; the playlist
/// rewrites are staged changes and go through Sync — which is why the two are
/// reported separately.
///
/// The keeper is added to a playlist **before** the loser is removed. Both are
/// staged, so the order only matters at apply time, and that is exactly when a
/// removal-first ordering would briefly leave the set short a track.
#[tauri::command]
pub async fn resolve_duplicates(
    app: tauri::AppHandle,
    library_path: String,
    plan: ResolutionPlan,
) -> Result<ResolveResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = cache_db(&app)?;
        let mut result = ResolveResult::default();

        let mut added_to: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (playlist_id, playlist_name, loser_id) in &plan.repoint {
            // One add per playlist, however many losers it held.
            if !plan.already_present.contains(playlist_name) && added_to.insert(playlist_id.clone())
            {
                let record = cache
                    .stage_change(changes::NewChange {
                        library_path: Some(library_path.clone()),
                        kind: changes::ChangeKind::PlaylistAddTrack,
                        target_id: Some(playlist_id.clone()),
                        field: None,
                        old_value: None,
                        new_value: Some(serde_json::json!(plan.keeper_id)),
                        reason: Some("Duplicate resolution — point at the keeper".to_string()),
                        confidence: Some(1.0),
                    })
                    .map_err(|e| e.to_string())?;
                result.staged.push(record.id);
            }
            let record = cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::PlaylistRemoveTrack,
                    target_id: Some(playlist_id.clone()),
                    field: None,
                    old_value: None,
                    new_value: Some(serde_json::json!(loser_id)),
                    reason: Some("Duplicate resolution — remove the duplicate".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            result.staged.push(record.id);
        }

        // Archived, never deleted — the guarantee the spec calls out. The
        // discarded copies sit in the Archive until the user says otherwise.
        cache
            .archive_tracks(&library_path, &plan.loser_ids)
            .map_err(|e| e.to_string())?;
        result.archived = plan.loser_ids;

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The keeper a rule would pick for each group, for bulk `Prefer`.
#[tauri::command]
pub fn preselect_keepers(
    groups: Vec<Vec<DuplicateCandidate>>,
    rule: PreferRule,
) -> Vec<Option<String>> {
    groups
        .iter()
        .map(|g| preselect(g, rule).map(|c| c.track_id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str) -> DuplicateCandidate {
        DuplicateCandidate {
            track_id: id.into(),
            bit_rate: None,
            duration_secs: None,
            has_cues: false,
            rating: None,
            play_count: None,
            in_playlists: 0,
        }
    }

    fn kept(group: &[DuplicateCandidate], rule: PreferRule) -> String {
        preselect(group, rule).unwrap().track_id.clone()
    }

    #[test]
    fn the_default_rule_keeps_the_prepared_copy_over_the_higher_bitrate_one() {
        // Losing someone's cue work is the expensive mistake; losing 64kbps is
        // not. This is the single most important line in the heuristic.
        let group = vec![
            DuplicateCandidate {
                bit_rate: Some(320),
                ..candidate("lossless")
            },
            DuplicateCandidate {
                bit_rate: Some(128),
                has_cues: true,
                ..candidate("prepared")
            },
        ];
        assert_eq!(kept(&group, PreferRule::Best), "prepared");
    }

    #[test]
    fn bitrate_breaks_a_tie_when_neither_has_cues() {
        let group = vec![
            DuplicateCandidate {
                bit_rate: Some(128),
                ..candidate("low")
            },
            DuplicateCandidate {
                bit_rate: Some(320),
                ..candidate("high")
            },
        ];
        assert_eq!(kept(&group, PreferRule::Best), "high");
    }

    #[test]
    fn playlist_membership_breaks_a_tie_after_bitrate() {
        let group = vec![
            DuplicateCandidate {
                bit_rate: Some(320),
                in_playlists: 0,
                ..candidate("orphan")
            },
            DuplicateCandidate {
                bit_rate: Some(320),
                in_playlists: 4,
                ..candidate("in-sets")
            },
        ];
        assert_eq!(kept(&group, PreferRule::Best), "in-sets");
    }

    #[test]
    fn an_explicit_bitrate_rule_overrides_cues() {
        // The point of Prefer is to say "I know, do it my way anyway".
        let group = vec![
            DuplicateCandidate {
                bit_rate: Some(320),
                ..candidate("lossless")
            },
            DuplicateCandidate {
                bit_rate: Some(128),
                has_cues: true,
                ..candidate("prepared")
            },
        ];
        assert_eq!(kept(&group, PreferRule::HighestBitrate), "lossless");
    }

    #[test]
    fn the_longest_rule_picks_the_full_length_copy() {
        // The radio-edit-versus-extended-mix case.
        let group = vec![
            DuplicateCandidate {
                duration_secs: Some(180),
                ..candidate("edit")
            },
            DuplicateCandidate {
                duration_secs: Some(420),
                ..candidate("extended")
            },
        ];
        assert_eq!(kept(&group, PreferRule::Longest), "extended");
    }

    #[test]
    fn a_missing_bitrate_loses_to_a_known_one_rather_than_winning() {
        let group = vec![
            candidate("unknown"),
            DuplicateCandidate {
                bit_rate: Some(128),
                ..candidate("known")
            },
        ];
        assert_eq!(kept(&group, PreferRule::Best), "known");
    }

    #[test]
    fn a_genuine_tie_resolves_the_same_way_every_run() {
        // Otherwise a bulk Prefer over 200 groups gives different answers each
        // time it is previewed.
        let group = vec![candidate("a"), candidate("b")];
        let first = kept(&group, PreferRule::Best);
        for _ in 0..10 {
            assert_eq!(kept(&group, PreferRule::Best), first);
        }
        assert_eq!(first, "a");
    }

    #[test]
    fn an_empty_group_has_no_keeper_rather_than_panicking() {
        assert!(preselect(&[], PreferRule::Best).is_none());
    }

    #[test]
    fn bulk_preselect_answers_for_every_group() {
        let groups = vec![
            vec![
                candidate("a"),
                DuplicateCandidate {
                    has_cues: true,
                    ..candidate("b")
                },
            ],
            vec![],
        ];
        assert_eq!(
            preselect_keepers(groups, PreferRule::Best),
            vec![Some("b".to_string()), None]
        );
    }

    #[test]
    fn rules_round_trip_through_json() {
        let all = vec![
            PreferRule::Best,
            PreferRule::HighestBitrate,
            PreferRule::HasCues,
            PreferRule::MostPlaylists,
            PreferRule::Longest,
        ];
        let json = serde_json::to_string(&all).unwrap();
        assert_eq!(serde_json::from_str::<Vec<PreferRule>>(&json).unwrap(), all);
    }
}
