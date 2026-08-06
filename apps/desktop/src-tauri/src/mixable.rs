//! Mixable Tracks (Epic 6) — the IPC surface for `scoring::mixable`.
//!
//! Per `docs/lexicon/04-analysis.md §Mixable Tracks`. The engine is a pure
//! function over `&[Track]` plus a context; this module owns opening the two
//! databases the context comes from (`master.db` for cues, the cache for tags
//! and archived tracks) and the template store.
//!
//! `scoring::score_transition` and the old `suggest_next_tracks` command have
//! existed since long before this epic with **no UI caller** — flagged as
//! "partial, and stranded" in the spec. This is what reaches them.

use std::collections::HashSet;
use std::path::Path;

use decks_core::rekordbox_db::{RekordboxDb, Track};
use scoring::{
    compatible_keys, find_mixable, KeyMixingMode, MixableContext, MixableMatch, MixableOptions,
};
use serde::Serialize;

use crate::{cache_db, hydrate_energy, read_config, write_config};

/// Where the global Key Mixing Mode lives in `config.json`.
const KEY_MIXING_MODE: &str = "key_mixing_mode";

/// The result list plus the facts the panel needs to explain it.
#[derive(Debug, Serialize)]
pub struct MixableResult {
    pub source: Track,
    pub matches: Vec<MixableMatch>,
    /// How many tracks were considered before the rules ran, so an empty list
    /// can say "0 of 4,213" rather than just being blank.
    pub considered: usize,
    /// The keys that mix out of the source under the active mode. Empty when
    /// the source has no parseable key — which is itself worth showing.
    pub compatible_keys: Vec<String>,
}

/// A saved option set, with its rules already parsed.
#[derive(Debug, Serialize)]
pub struct MixableTemplateView {
    pub id: String,
    pub name: String,
    pub options: MixableOptions,
    pub created_at: i64,
}

fn open_db(path: &str) -> Result<RekordboxDb, String> {
    RekordboxDb::open(Path::new(path)).map_err(|e| e.to_string())
}

/// Read the global Key Mixing Mode, defaulting to Harmonically Compatible.
pub fn stored_key_mixing_mode(app: &tauri::AppHandle) -> KeyMixingMode {
    match read_config(app)
        .ok()
        .and_then(|c| c[KEY_MIXING_MODE].as_str().map(str::to_owned))
        .as_deref()
    {
        Some("fuzzy") => KeyMixingMode::Fuzzy,
        _ => KeyMixingMode::HarmonicallyCompatible,
    }
}

#[tauri::command]
pub fn get_key_mixing_mode(app: tauri::AppHandle) -> Result<KeyMixingMode, String> {
    Ok(stored_key_mixing_mode(&app))
}

/// The spec's basic mode, as the panel's starting rule set.
///
/// Served rather than hardcoded in the renderer so there is one definition of
/// "basic mode" — a second copy in TypeScript would drift the first time a
/// default changed, and nothing would fail.
#[tauri::command]
pub fn mixable_default_options(app: tauri::AppHandle) -> Result<MixableOptions, String> {
    Ok(MixableOptions {
        key_mixing_mode: stored_key_mixing_mode(&app),
        ..MixableOptions::basic()
    })
}

#[tauri::command]
pub fn set_key_mixing_mode(app: tauri::AppHandle, mode: KeyMixingMode) -> Result<(), String> {
    let mut config = read_config(&app)?;
    config[KEY_MIXING_MODE] = serde_json::to_value(mode).map_err(|e| e.to_string())?;
    write_config(&app, &config)
}

/// Rank the library against one track.
///
/// `options.key_mixing_mode` is **overwritten** with the stored global mode:
/// the spec makes it a global setting shared with the browser's compatible-key
/// indicator, so a template that carried a stale mode must not quietly override
/// what the user set in Settings.
#[tauri::command]
pub async fn find_mixable_tracks(
    app: tauri::AppHandle,
    path: String,
    track_id: String,
    options: Option<MixableOptions>,
) -> Result<MixableResult, String> {
    let mode = stored_key_mixing_mode(&app);
    let cache = cache_db(&app).ok();

    tauri::async_runtime::spawn_blocking(move || {
        let mut opts = options.unwrap_or_default();
        opts.key_mixing_mode = mode;

        let db = open_db(&path)?;
        let Some(source) = db.track_by_id(&track_id).map_err(|e| e.to_string())? else {
            return Err(format!("Source track not found: {track_id}"));
        };

        let mut tracks = db.tracks().map_err(|e| e.to_string())?;

        let mut ctx = MixableContext {
            tracks_with_cues: db
                .track_ids_with_cues()
                .map_err(|e| e.to_string())?
                .into_iter()
                .collect(),
            ..Default::default()
        };

        if let Some(cache) = &cache {
            hydrate_energy(&mut tracks, cache);
            if let Ok(ids) = cache.list_archived(&path) {
                ctx.archived_tracks = ids.into_iter().collect();
            }
            if let Ok(map) = cache.list_track_tags_map(&path) {
                ctx.tags_by_track = map
                    .into_iter()
                    .map(|(k, v)| (k, v.into_iter().collect::<HashSet<String>>()))
                    .collect();
            }
        }

        // The source is hydrated too — a `NearSource` energy rule reads its
        // value, and `db.track_by_id` does not know about the cache.
        let source = tracks
            .iter()
            .find(|t| t.id == source.id)
            .cloned()
            .unwrap_or(source);

        let compatible = source
            .musical_key
            .as_deref()
            .map(|k| compatible_keys(k, mode))
            .unwrap_or_default();

        let considered = tracks.len().saturating_sub(1);
        let matches = find_mixable(&source, &tracks, &opts, &ctx);

        Ok(MixableResult {
            source,
            matches,
            considered,
            compatible_keys: compatible,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ── Templates ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_mixable_templates(app: tauri::AppHandle) -> Result<Vec<MixableTemplateView>, String> {
    let cache = cache_db(&app)?;
    let rows = cache.list_mixable_templates().map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter_map(|t| {
            // A template whose JSON no longer parses is skipped rather than
            // failing the whole list — one bad row must not hide the others.
            let options = serde_json::from_str::<MixableOptions>(&t.options).ok()?;
            Some(MixableTemplateView {
                id: t.id,
                name: t.name,
                options,
                created_at: t.created_at,
            })
        })
        .collect())
}

#[tauri::command]
pub fn save_mixable_template(
    app: tauri::AppHandle,
    name: String,
    options: MixableOptions,
) -> Result<String, String> {
    let cache = cache_db(&app)?;
    let json = serde_json::to_string(&options).map_err(|e| e.to_string())?;
    cache
        .save_mixable_template(&name, &json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_mixable_template(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    let cache = cache_db(&app)?;
    cache
        .delete_mixable_template(&id)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use scoring::{KeyMixingMode, MixableOptions, NumericRule};

    #[test]
    fn default_options_are_basic_mode() {
        assert_eq!(MixableOptions::default(), MixableOptions::basic());
    }

    #[test]
    fn the_key_mixing_mode_serialises_as_the_config_string_we_read_back() {
        // `stored_key_mixing_mode` matches on "fuzzy"; if the serde rename ever
        // changed, the setting would silently stop round-tripping.
        assert_eq!(
            serde_json::to_value(KeyMixingMode::Fuzzy).unwrap(),
            serde_json::json!("fuzzy")
        );
        assert_eq!(
            serde_json::to_value(KeyMixingMode::HarmonicallyCompatible).unwrap(),
            serde_json::json!("harmonically_compatible")
        );
    }

    #[test]
    fn templates_round_trip_through_the_stored_json() {
        let opts = MixableOptions {
            must_have_cues: true,
            energy: NumericRule::NearSource,
            ..MixableOptions::basic()
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: MixableOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back, opts);
    }
}
