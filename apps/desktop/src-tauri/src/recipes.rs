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
fn column_for(field: &str) -> Option<&'static str> {
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
/// is what the user reviewed. A tag name with no existing tag is created in the
/// first category — importing `#Techno` from a comment is useless if it fails
/// because nobody made a Techno tag first.
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

        let default_category = cache
            .list_tag_categories()
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .map(|c| c.id);

        for p in proposals {
            let mut touched = false;

            for name in &p.added {
                let key = name.to_lowercase();
                let id = match by_name.get(&key) {
                    Some(id) => id.clone(),
                    None => {
                        let Some(category) = default_category.clone() else {
                            // No categories at all — creating a tag would have
                            // nowhere to live. Report rather than guess.
                            return Err("create a tag category before importing tags".to_string());
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
}
