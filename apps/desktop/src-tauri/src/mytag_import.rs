//! Importing Rekordbox MyTags into Custom Tags (Epic 5).
//!
//! Per `docs/lexicon/02-library.md §Custom Tags`: "Rekordbox 6/7 MyTags import
//! automatically." Here it is a **preview-then-apply** flow rather than an
//! automatic one, for the same reason every other bulk operation in `decks` is:
//! this creates categories and tags in the user's own tag tree, and merging a
//! stranger's taxonomy into yours without showing it first is how a tag list
//! becomes unusable.
//!
//! Matching is **by name, case-insensitively**, at both levels. Rekordbox ids
//! are not stored: a MyTag id means nothing outside the database it came from,
//! and the thing the user recognises is the name. The cost is that renaming a
//! category in Rekordbox makes the next import look like a new one — which the
//! preview shows, and which is recoverable, unlike the alternative of silently
//! renaming the user's own category to match.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Serialize;

use crate::cache_db;

/// What an import would do, before it does anything.
#[derive(Debug, Default, Serialize)]
pub struct MyTagImportPreview {
    /// Categories that do not exist here yet, by name.
    pub new_categories: Vec<String>,
    /// `(category, tag)` pairs that do not exist here yet.
    pub new_tags: Vec<(String, String)>,
    /// Tags that already exist and will be reused rather than duplicated.
    pub existing_tags: usize,
    /// Track↔tag links that would be added. Links already present are not
    /// counted — re-importing must read as "nothing to do", not as work.
    pub new_links: usize,
    /// Links whose track is not in this library. Reported rather than hidden:
    /// a large number here means the MyTag data is from a different collection.
    pub unmatched_links: usize,
}

/// Lower-case, trimmed — the key both levels of matching use.
fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

struct Plan {
    /// Rekordbox tag id → `(category name, tag name)`.
    tag_names: HashMap<String, (String, String)>,
    /// Rekordbox category name → its tag names, in Rekordbox's order.
    categories: Vec<(String, Vec<String>)>,
    links: Vec<(String, String)>,
}

fn read_plan(library_path: &str) -> Result<Plan, String> {
    let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(library_path))
        .map_err(|e| e.to_string())?;
    let categories = db.my_tags().map_err(|e| e.to_string())?;
    let links = db.my_tags_by_track().map_err(|e| e.to_string())?;

    let mut tag_names = HashMap::new();
    for cat in &categories {
        for tag in &cat.tags {
            tag_names.insert(tag.id.clone(), (cat.name.clone(), tag.name.clone()));
        }
    }

    Ok(Plan {
        tag_names,
        categories: categories
            .into_iter()
            .map(|c| (c.name, c.tags.into_iter().map(|t| t.name).collect()))
            .collect(),
        links,
    })
}

/// What the import would create, without creating it.
#[tauri::command]
pub async fn preview_mytag_import(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<MyTagImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = read_plan(&library_path)?;
        let cache = cache_db(&app)?;

        let existing_categories: HashMap<String, String> = cache
            .list_tag_categories()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| (key(&c.name), c.id))
            .collect();
        let existing_tags: HashSet<(String, String)> = cache
            .list_tags(None)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|t| {
                let cat = existing_categories
                    .iter()
                    .find(|(_, id)| **id == t.category_id)?;
                Some((cat.0.clone(), key(&t.name)))
            })
            .collect();

        let mut preview = MyTagImportPreview::default();
        for (cat_name, tags) in &plan.categories {
            if !existing_categories.contains_key(&key(cat_name)) {
                preview.new_categories.push(cat_name.clone());
            }
            for tag_name in tags {
                if existing_tags.contains(&(key(cat_name), key(tag_name))) {
                    preview.existing_tags += 1;
                } else {
                    preview.new_tags.push((cat_name.clone(), tag_name.clone()));
                }
            }
        }

        // Links: a track that is not in this library cannot be tagged, and a
        // link that already exists is not work.
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let library_ids: HashSet<String> = db
            .tracks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| t.id)
            .collect();
        let already: HashMap<String, Vec<String>> = cache
            .list_track_tags_map(&library_path)
            .map_err(|e| e.to_string())?;

        for (track_id, my_tag_id) in &plan.links {
            if !library_ids.contains(track_id) {
                preview.unmatched_links += 1;
                continue;
            }
            let Some((cat_name, tag_name)) = plan.tag_names.get(my_tag_id) else {
                continue;
            };
            // An existing link can only be recognised once the tag exists here,
            // so a first import counts every link as new — which is true.
            let existing_tag_id = cache
                .list_tags(None)
                .ok()
                .and_then(|tags| {
                    tags.into_iter().find(|t| {
                        key(&t.name) == key(tag_name)
                            && existing_categories.get(&key(cat_name)) == Some(&t.category_id)
                    })
                })
                .map(|t| t.id);
            let linked = existing_tag_id
                .as_ref()
                .is_some_and(|id| already.get(track_id).is_some_and(|ids| ids.contains(id)));
            if !linked {
                preview.new_links += 1;
            }
        }

        Ok(preview)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Default, Serialize)]
pub struct MyTagImportResult {
    pub categories_created: usize,
    pub tags_created: usize,
    pub links_created: usize,
}

/// Merge Rekordbox's MyTags into the Custom Tags tree.
///
/// **Idempotent.** Re-running matches existing categories and tags by name and
/// reuses them, and `add_track_tag` ignores a link that is already there — so a
/// second import creates nothing and reports zeroes. That is what makes this
/// safe to run after every Rekordbox session.
///
/// Nothing here touches `master.db`; tags live entirely in the local cache.
#[tauri::command]
pub async fn import_mytags(
    app: tauri::AppHandle,
    library_path: String,
) -> Result<MyTagImportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = read_plan(&library_path)?;
        let cache = cache_db(&app)?;
        let mut result = MyTagImportResult::default();

        // Categories first, so tags have somewhere to go.
        let mut category_ids: HashMap<String, String> = cache
            .list_tag_categories()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| (key(&c.name), c.id))
            .collect();

        for (cat_name, _) in &plan.categories {
            if category_ids.contains_key(&key(cat_name)) {
                continue;
            }
            let created = cache
                .create_tag_category(cat_name)
                .map_err(|e| e.to_string())?;
            category_ids.insert(key(cat_name), created.id);
            result.categories_created += 1;
        }

        // Then tags, keyed by (category, name) so two categories may hold the
        // same tag name without colliding — "Deep" under Genre and under Mood
        // are different tags.
        let mut tag_ids: HashMap<(String, String), String> = cache
            .list_tags(None)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|t| {
                let cat_key = category_ids
                    .iter()
                    .find(|(_, id)| **id == t.category_id)
                    .map(|(k, _)| k.clone())?;
                Some(((cat_key, key(&t.name)), t.id))
            })
            .collect();

        for (cat_name, tags) in &plan.categories {
            let Some(category_id) = category_ids.get(&key(cat_name)) else {
                continue;
            };
            for tag_name in tags {
                let map_key = (key(cat_name), key(tag_name));
                if tag_ids.contains_key(&map_key) {
                    continue;
                }
                let created = cache
                    .create_tag(category_id, tag_name)
                    .map_err(|e| e.to_string())?;
                tag_ids.insert(map_key, created.id);
                result.tags_created += 1;
            }
        }

        // Finally the links. Tracks absent from this library are skipped —
        // `preview` has already reported how many that is.
        let db = decks_core::rekordbox_db::RekordboxDb::open(Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let library_ids: HashSet<String> = db
            .tracks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| t.id)
            .collect();
        let already = cache
            .list_track_tags_map(&library_path)
            .map_err(|e| e.to_string())?;

        for (track_id, my_tag_id) in &plan.links {
            if !library_ids.contains(track_id) {
                continue;
            }
            let Some((cat_name, tag_name)) = plan.tag_names.get(my_tag_id) else {
                continue;
            };
            let Some(tag_id) = tag_ids.get(&(key(cat_name), key(tag_name))) else {
                continue;
            };
            if already
                .get(track_id)
                .is_some_and(|ids| ids.contains(tag_id))
            {
                continue;
            }
            cache
                .add_track_tag(&library_path, track_id, tag_id)
                .map_err(|e| e.to_string())?;
            result.links_created += 1;
        }

        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::key;

    #[test]
    fn matching_ignores_case_and_surrounding_space() {
        // "Techno", "techno" and " Techno " are the same tag to a human, and
        // importing three of them is the failure this prevents.
        assert_eq!(key("Techno"), key("techno"));
        assert_eq!(key(" Techno "), key("Techno"));
    }

    #[test]
    fn matching_does_not_collapse_different_names() {
        assert_ne!(key("Deep House"), key("Deep"));
    }
}
