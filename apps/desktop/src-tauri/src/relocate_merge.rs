//! Relocating a missing track onto a file another track already claims.
//!
//! Per `docs/lexicon/07-health.md §Find Lost Tracks / Relocate`: picking a
//! replacement file that is **already in the library** offers a
//! choose-which-to-keep merge — the other entry is removed and replaced across
//! every playlist, so nothing breaks.
//!
//! The spec's own constraint is that you may only relocate to a path not
//! already in the library. This is the escape hatch from that constraint, not
//! an exception to it: rather than pointing two rows at one file, the two rows
//! become one.
//!
//! **The playlist rewriting is `duplicates`', not a second copy.** Merging two
//! entries into one is precisely what resolving a duplicate group does, and a
//! parallel implementation would drift — most likely on the case that took the
//! longest to get right there, which is a playlist already holding the keeper.

use serde::{Deserialize, Serialize};

/// What relocating onto an occupied path would mean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelocateTarget {
    /// Nothing else claims this path — an ordinary relocate.
    Free,
    /// Another track already points here, so this is a merge.
    Occupied(OccupiedBy),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OccupiedBy {
    pub track_id: String,
    pub title: String,
    pub artist: Option<String>,
}

/// Compare two paths the way the filesystem the library sits on would.
///
/// Separators are normalised, and comparison is case-insensitive. Rekordbox
/// stores whatever the OS handed it, so the same file reached through a
/// different separator or a different case is still the same file — and
/// treating it as free would create exactly the two-rows-one-file state this
/// command exists to prevent.
fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().replace('\\', "/").to_lowercase();
    !a.trim().is_empty() && norm(a) == norm(b)
}

/// Which track, if any, already claims `path`.
///
/// `moving_id` is excluded: relocating a track onto its own current path is a
/// no-op, not a merge with itself.
pub fn classify(
    path: &str,
    moving_id: &str,
    tracks: &[decks_core::rekordbox_db::Track],
) -> RelocateTarget {
    match tracks
        .iter()
        .filter(|t| t.id != moving_id)
        .find(|t| t.folder_path.as_deref().is_some_and(|p| same_path(p, path)))
    {
        Some(t) => RelocateTarget::Occupied(OccupiedBy {
            track_id: t.id.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
        }),
        None => RelocateTarget::Free,
    }
}

/// Does another track already point at this file?
///
/// Called before offering a relocate so the UI can ask which entry to keep
/// rather than refusing, which is what the spec's constraint would otherwise
/// force.
#[tauri::command]
pub async fn classify_relocate_target(
    library_path: String,
    track_id: String,
    new_path: String,
) -> Result<RelocateTarget, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(std::path::Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let tracks = db.tracks().map_err(|e| e.to_string())?;
        Ok(classify(&new_path, &track_id, &tracks))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What a merge relocate would do, in full.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRelocatePlan {
    /// The entry that survives.
    pub keeper_id: String,
    /// The entry that is archived and re-pointed away from.
    pub loser_id: String,
    /// Set when the keeper's path has to change — i.e. the user kept the
    /// *missing* track and it takes over the found file's path.
    pub relocate_to: Option<String>,
    /// Delegated wholesale to `duplicates`, so the playlist rewriting has one
    /// implementation rather than two that drift.
    pub resolution: crate::duplicates::ResolutionPlan,
}

/// Plan a merge: which entry survives, and what happens to the playlists.
///
/// `keep_moving` chooses. Keeping the moving (missing) track means it takes the
/// found file's path and the other entry goes; keeping the existing track means
/// the missing one goes and no path changes at all — the file was already
/// correctly attached to a library entry, and the missing row was the mistake.
#[tauri::command]
pub async fn plan_merge_relocate(
    library_path: String,
    moving_id: String,
    existing_id: String,
    new_path: String,
    keep_moving: bool,
) -> Result<MergeRelocatePlan, String> {
    let (keeper_id, loser_id) = if keep_moving {
        (moving_id.clone(), existing_id.clone())
    } else {
        (existing_id.clone(), moving_id.clone())
    };
    let resolution = crate::duplicates::plan_duplicate_resolution(
        library_path,
        keeper_id.clone(),
        vec![loser_id.clone()],
    )
    .await?;

    Ok(MergeRelocatePlan {
        keeper_id,
        loser_id,
        // Only the keep-the-missing-track branch moves a path. Keeping the
        // existing entry needs no relocate: it already points at the file.
        relocate_to: keep_moving.then_some(new_path),
        resolution,
    })
}

#[derive(Debug, Default, Serialize)]
pub struct MergeRelocateResult {
    pub archived: Vec<String>,
    pub staged: Vec<String>,
}

/// Execute the plan: stage the relocate, then hand the rest to `duplicates`.
///
/// Takes the plan back rather than recomputing it, so what happens is what the
/// user was shown — the same contract `resolve_duplicates` has.
#[tauri::command]
pub async fn apply_merge_relocate(
    app: tauri::AppHandle,
    library_path: String,
    plan: MergeRelocatePlan,
) -> Result<MergeRelocateResult, String> {
    let mut result = MergeRelocateResult::default();

    if let Some(to) = plan.relocate_to.clone() {
        let cache = crate::cache_db(&app)?;
        let record = cache
            .stage_change(changes::NewChange {
                library_path: Some(library_path.clone()),
                kind: changes::ChangeKind::TrackRelocate,
                target_id: Some(plan.keeper_id.clone()),
                field: None,
                // The old path is not recorded: the track is missing, so its
                // stored path points at nothing, and writing it into the change
                // would make the undo entry restore a path known to be broken.
                old_value: None,
                new_value: Some(serde_json::json!(to)),
                reason: Some("Relocate (merged with existing entry)".to_string()),
                confidence: Some(1.0),
            })
            .map_err(|e| e.to_string())?;
        result.staged.push(record.id);
    }

    let resolved =
        crate::duplicates::resolve_duplicates(app, library_path, plan.resolution.clone()).await?;
    result.archived = resolved.archived;
    result.staged.extend(resolved.staged);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use decks_core::rekordbox_db::Track;

    fn track(id: &str, path: Option<&str>) -> Track {
        Track {
            id: id.into(),
            title: format!("Track {id}"),
            artist: Some("Someone".into()),
            album: None,
            genre: None,
            musical_key: None,
            bpm: None,
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: path.map(str::to_string),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: None,
            dj_play_count: None,
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
            energy: None,
        }
    }

    #[test]
    fn an_unclaimed_path_is_free() {
        let tracks = vec![track("a", Some("/music/a.mp3"))];
        assert_eq!(
            classify("/music/new.mp3", "missing", &tracks),
            RelocateTarget::Free
        );
    }

    #[test]
    fn a_claimed_path_names_the_track_that_holds_it() {
        let tracks = vec![track("b", Some("/music/b.mp3"))];
        match classify("/music/b.mp3", "missing", &tracks) {
            RelocateTarget::Occupied(by) => {
                assert_eq!(by.track_id, "b");
                assert_eq!(by.title, "Track b");
            }
            other => panic!("expected Occupied, got {other:?}"),
        }
    }

    #[test]
    fn separators_and_case_do_not_hide_a_collision() {
        // Rekordbox stores whatever the OS handed it. Treating a
        // differently-spelled path as free is how you end up with two rows
        // pointing at one file — the state this command exists to prevent.
        let tracks = vec![track("b", Some("D:\\Music\\B.mp3"))];
        assert!(matches!(
            classify("d:/music/b.mp3", "missing", &tracks),
            RelocateTarget::Occupied(_)
        ));
    }

    #[test]
    fn a_track_never_collides_with_itself() {
        // Relocating a track onto its own path is a no-op, not a merge.
        let tracks = vec![track("a", Some("/music/a.mp3"))];
        assert_eq!(classify("/music/a.mp3", "a", &tracks), RelocateTarget::Free);
    }

    #[test]
    fn a_track_with_no_path_claims_nothing() {
        let tracks = vec![track("a", None)];
        assert_eq!(classify("/music/x.mp3", "b", &tracks), RelocateTarget::Free);
    }

    #[test]
    fn an_empty_requested_path_matches_nothing() {
        // Otherwise it would collide with every track whose path is also blank.
        let tracks = vec![track("a", Some("")), track("b", Some("/music/b.mp3"))];
        assert_eq!(classify("", "z", &tracks), RelocateTarget::Free);
    }
}
