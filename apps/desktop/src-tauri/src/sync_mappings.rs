//! Field Mappings applied to the Rekordbox library, not just to file tags.
//!
//! Per `docs/lexicon/01-interop.md §Field Mappings`: mappings are configured
//! **per DJ app**, and Lexicon applies them on sync. `decks` has had the
//! projection engine and the ID3 profile since Epic 4; this is the Rekordbox
//! profile and the path that gets it into `master.db`.
//!
//! **Preview, then stage — never write directly.** A mapping rewrites Comment
//! or Genre across the whole library, which is the single most destructive
//! shape of edit this app can make. Every other bulk operation here goes
//! through the staged-change pipeline so the user sees the diff and can reject
//! rows, and there is no argument for this one being the exception. The result
//! is that "apply mappings on sync" means *stage the edits sync will write*,
//! which the review table then shows like anything else.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use changes::field_mappings::{FieldMappings, MappingInput};
use serde::Serialize;

use crate::cache_db;

/// The mapping profile that targets the Rekordbox database.
///
/// Separate from `ID3_PROFILE` because the two destinations genuinely differ:
/// an audio file has no Rating frame worth writing and `djmdContent` has no
/// album-art column, so a single shared list would offer targets that silently
/// do nothing on one side or the other.
pub const REKORDBOX_PROFILE: &str = "rekordbox";

/// One edit a mapping would make, before it makes it.
#[derive(Debug, Clone, Serialize)]
pub struct MappingProposal {
    /// Stable within a preview so the UI can deselect rows by id.
    pub id: String,
    pub track_id: String,
    pub track_title: String,
    /// The `djmdContent` column, in the applier's vocabulary.
    pub target: String,
    pub before: Option<String>,
    pub after: String,
}

#[derive(Debug, Default, Serialize)]
pub struct MappingPreview {
    pub proposals: Vec<MappingProposal>,
    /// Targets the applier will not write, named rather than dropped. A mapping
    /// onto a column that vanishes at sync time looks like data loss.
    pub unwritable_targets: Vec<String>,
    /// Tracks a mapping produced nothing for. Reported so "0 proposals" reads
    /// as "nothing to change" rather than as a broken configuration.
    pub unchanged: usize,
}

/// Read the current value of a mapping target from a track.
///
/// Only the columns a mapping can sensibly append to. A target we cannot read
/// is treated as empty, which is right for an overwriting mapping and makes an
/// appending one behave as if the field were blank — visible in the preview
/// either way.
fn existing_values(t: &decks_core::rekordbox_db::Track) -> BTreeMap<String, String> {
    [
        ("Title", Some(t.title.clone())),
        ("Commnt", t.comment.clone()),
        ("Genre", t.genre.clone()),
        ("Album", t.album.clone()),
        ("Artist", t.artist.clone()),
        ("Label", t.label.clone()),
    ]
    .into_iter()
    .filter_map(|(k, v)| {
        v.filter(|s| !s.trim().is_empty())
            .map(|v| (k.to_string(), v))
    })
    .collect()
}

/// `MappingInput` for one track, given the tag index built once for the batch.
fn input_for(
    t: &decks_core::rekordbox_db::Track,
    tags_by_track: &HashMap<String, Vec<String>>,
    tag_index: &HashMap<String, (String, String)>,
) -> MappingInput {
    MappingInput {
        // The UI shows energy 0–10; the cache stores 0.0–1.0.
        energy: t.energy.map(|e| (e * 10.0).round() as u8),
        tags: tags_by_track
            .get(&t.id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| tag_index.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default(),
        colour_name: t.color.clone(),
        // Danceability / Popularity / Happiness are blocked upstream, not
        // merely unpopulated — see ADR-0012. Left absent so a mapping on one
        // contributes nothing rather than writing a zero we did not measure.
        ..Default::default()
    }
}

struct Batch {
    mappings: FieldMappings,
    tracks: Vec<decks_core::rekordbox_db::Track>,
    tags_by_track: HashMap<String, Vec<String>>,
    tag_index: HashMap<String, (String, String)>,
}

fn load(app: &tauri::AppHandle, library_path: &str) -> Result<Batch, String> {
    let cache = cache_db(app)?;
    let mappings = cache
        .list_field_mappings(REKORDBOX_PROFILE)
        .map_err(|e| e.to_string())?;

    let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(library_path))
        .map_err(|e| e.to_string())?;
    let tracks = db.tracks().map_err(|e| e.to_string())?;

    // Resolved once for the whole batch: a per-track tag lookup over a few
    // thousand tracks is the difference between an instant preview and one the
    // user waits on.
    let tags_by_track = cache
        .list_track_tags_map(library_path)
        .map_err(|e| e.to_string())?;
    let categories: HashMap<String, String> = cache
        .list_tag_categories()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let tag_index = cache
        .list_tags(None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| {
            let category = categories.get(&t.category_id).cloned().unwrap_or_default();
            (t.id, (category, t.name))
        })
        .collect();

    Ok(Batch {
        mappings,
        tracks,
        tags_by_track,
        tag_index,
    })
}

fn build_preview(batch: &Batch) -> MappingPreview {
    let mut preview = MappingPreview::default();
    if batch.mappings.is_empty() {
        return preview;
    }

    for target in batch.mappings.targets() {
        if !changes::applier::writes_field(target) {
            preview.unwritable_targets.push(target.to_string());
        }
    }

    for track in &batch.tracks {
        let existing = existing_values(track);
        let input = input_for(track, &batch.tags_by_track, &batch.tag_index);
        let projected = batch.mappings.project(&input, &existing);

        let mut changed = false;
        for (target, after) in projected {
            if !changes::applier::writes_field(&target) {
                continue;
            }
            let before = existing.get(&target).cloned();
            // A mapping that reproduces what is already there is not a change.
            // Staging it would put the whole library in the review table on
            // every sync and bury the edits that matter.
            if before.as_deref() == Some(after.as_str()) {
                continue;
            }
            changed = true;
            preview.proposals.push(MappingProposal {
                id: format!("{}:{}", track.id, target),
                track_id: track.id.clone(),
                track_title: track.title.clone(),
                target,
                before,
                after,
            });
        }
        if !changed {
            preview.unchanged += 1;
        }
    }
    preview
}

/// What the Rekordbox field mappings would write, without writing it.
#[tauri::command]
pub async fn preview_sync_mappings(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<MappingPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let batch = load(&app, &library_path)?;
        Ok(build_preview(&batch))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Default, Serialize)]
pub struct StageResult {
    pub staged: usize,
}

/// Stage the mapping edits as ordinary `TrackMetadataEdit` changes.
///
/// `proposal_ids` empty means "all of them"; otherwise only the rows the user
/// left ticked. Staged as `Proposed`, so they still go through the review table
/// and the `WriteGuard` before touching `master.db` — this command writes
/// nothing to the library itself.
#[tauri::command]
pub async fn stage_sync_mappings(
    app: tauri::AppHandle,
    library_path: String,
    proposal_ids: Vec<String>,
) -> Result<StageResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let batch = load(&app, &library_path)?;
        let preview = build_preview(&batch);
        let cache = cache_db(&app)?;

        let wanted: std::collections::HashSet<String> = proposal_ids.into_iter().collect();
        let mut result = StageResult::default();
        for p in preview.proposals {
            if !wanted.is_empty() && !wanted.contains(&p.id) {
                continue;
            }
            cache
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackMetadataEdit,
                    target_id: Some(p.track_id),
                    field: Some(p.target),
                    old_value: Some(match p.before {
                        Some(v) => serde_json::json!(v),
                        None => serde_json::Value::Null,
                    }),
                    new_value: Some(serde_json::json!(p.after)),
                    reason: Some("Field mapping".to_string()),
                    confidence: Some(1.0),
                })
                .map_err(|e| e.to_string())?;
            result.staged += 1;
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use changes::field_mappings::{FieldMapping, MappingSource};
    use decks_core::rekordbox_db::Track;

    fn track(id: &str, comment: Option<&str>, energy: Option<f32>) -> Track {
        Track {
            id: id.into(),
            title: format!("Track {id}"),
            artist: None,
            album: None,
            genre: None,
            musical_key: None,
            bpm: None,
            duration_secs: None,
            rating: None,
            comment: comment.map(str::to_string),
            folder_path: None,
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
            energy,
        }
    }

    fn batch(mappings: Vec<FieldMapping>, tracks: Vec<Track>) -> Batch {
        Batch {
            mappings: FieldMappings::new(mappings),
            tracks,
            tags_by_track: HashMap::new(),
            tag_index: HashMap::new(),
        }
    }

    fn energy_to_comment(overwrite: bool) -> FieldMapping {
        FieldMapping {
            source: MappingSource::Energy,
            target: "Commnt".into(),
            overwrite,
        }
    }

    #[test]
    fn no_mappings_proposes_nothing() {
        let p = build_preview(&batch(vec![], vec![track("1", None, Some(0.8))]));
        assert!(p.proposals.is_empty());
    }

    #[test]
    fn an_overwriting_mapping_replaces_the_target() {
        let p = build_preview(&batch(
            vec![energy_to_comment(true)],
            vec![track("1", Some("old note"), Some(0.8))],
        ));
        assert_eq!(p.proposals.len(), 1);
        assert_eq!(p.proposals[0].before.as_deref(), Some("old note"));
        assert_eq!(p.proposals[0].after, "Energy 08");
    }

    #[test]
    fn an_appending_mapping_keeps_what_is_there() {
        let p = build_preview(&batch(
            vec![energy_to_comment(false)],
            vec![track("1", Some("old note"), Some(0.8))],
        ));
        assert_eq!(p.proposals[0].after, "old note, Energy 08");
    }

    #[test]
    fn a_track_with_no_value_for_the_source_is_left_alone() {
        // Not "Energy" with no number, and not a blanked comment.
        let p = build_preview(&batch(
            vec![energy_to_comment(true)],
            vec![track("1", Some("keep me"), None)],
        ));
        assert!(p.proposals.is_empty());
        assert_eq!(p.unchanged, 1);
    }

    /// The guard that keeps the review table usable.
    #[test]
    fn a_mapping_that_reproduces_the_current_value_is_not_a_change() {
        // Otherwise the second sync stages the entire library again and buries
        // the edits that actually matter.
        let p = build_preview(&batch(
            vec![energy_to_comment(true)],
            vec![track("1", Some("Energy 08"), Some(0.8))],
        ));
        assert!(p.proposals.is_empty(), "{:?}", p.proposals);
        assert_eq!(p.unchanged, 1);
    }

    #[test]
    fn an_unwritable_target_is_named_rather_than_dropped() {
        // A mapping onto a column sync will not write looks like data loss if
        // it simply disappears.
        let p = build_preview(&batch(
            vec![FieldMapping {
                source: MappingSource::Energy,
                target: "AlbumArt".into(),
                overwrite: true,
            }],
            vec![track("1", None, Some(0.8))],
        ));
        assert_eq!(p.unwritable_targets, vec!["AlbumArt"]);
        assert!(p.proposals.is_empty());
    }

    #[test]
    fn several_sources_on_one_target_combine() {
        let p = build_preview(&batch(
            vec![
                energy_to_comment(true),
                FieldMapping {
                    source: MappingSource::Colour,
                    target: "Commnt".into(),
                    overwrite: true,
                },
            ],
            vec![Track {
                color: Some("Red".into()),
                ..track("1", None, Some(0.8))
            }],
        ));
        assert_eq!(p.proposals[0].after, "Energy 08, Red");
    }

    #[test]
    fn proposal_ids_are_unique_per_track_and_target() {
        let p = build_preview(&batch(
            vec![energy_to_comment(true)],
            vec![track("1", None, Some(0.8)), track("2", None, Some(0.4))],
        ));
        let ids: Vec<&str> = p.proposals.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["1:Commnt", "2:Commnt"]);
    }
}
