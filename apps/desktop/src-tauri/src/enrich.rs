//! Find Tags & Album Art — the desktop wiring.
//!
//! Implements `docs/lexicon/07-health.md §Find Tags & Album Art` on top of
//! `crates/enrichment`. Three things live here and nowhere else:
//!
//! 1. **The cache adapter.** `enrichment` declares a `ResponseCache` trait and
//!    knows nothing about SQLite; [`DbCache`] binds it to `enrichment_cache`.
//! 2. **Preview then stage.** Nothing is written by looking. The preview
//!    returns proposals, the user picks, and only then do staged changes exist
//!    — the same shape as every other write path in this app.

use enrichment::reqwest_http::ReqwestHttp;
use enrichment::{Candidate, Existing, Providers, Query, Service};
use serde::{Deserialize, Serialize};

/// How long a cached provider answer stays current.
///
/// Thirty days. Release metadata is close to immutable — a 1997 record does not
/// change label next month — and the cost of being a month stale is that a
/// newly-added MusicBrainz tag is missed, against the benefit of never spending
/// a rate-limited request twice on the same question. `enrichment_clear` is the
/// escape hatch when a user knows the data has moved on.
const CACHE_MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

/// Binds `enrichment`'s cache trait to the `enrichment_cache` table.
/// `CacheDb` wraps a rusqlite `Connection`, which is `Send` but not `Sync`,
/// while `ResponseCache` must be `Sync` because the `Service` is held across
/// awaits. A mutex is the whole difference — contention is nil, since the
/// lookups it guards are serialised by the rate limiter anyway.
pub struct DbCache {
    db: std::sync::Mutex<cache::CacheDb>,
}

impl enrichment::ResponseCache for DbCache {
    fn get(&self, key: &str) -> Option<Vec<Candidate>> {
        // `key` is `provider\u{1}artist\u{1}title\u{1}original`, built by
        // `Query::cache_key`. Splitting the provider back out keeps the table's
        // primary key meaningful — one provider's answer can be cleared or
        // inspected without the other's.
        let (provider, rest) = key.split_once('\u{1}')?;
        let json = self
            .db
            .lock()
            .ok()?
            .enrichment_cached(provider, rest, CACHE_MAX_AGE_SECS)
            .ok()??;
        // A row that will not parse is treated as a miss rather than an error:
        // the shape of `Candidate` can change between releases, and a stale
        // cache should cost a re-fetch, not break the feature.
        serde_json::from_str(&json).ok()
    }

    fn put(&self, key: &str, candidates: &[Candidate]) {
        let Some((provider, rest)) = key.split_once('\u{1}') else {
            return;
        };
        if let (Ok(json), Ok(db)) = (serde_json::to_string(candidates), self.db.lock()) {
            let _ = db.enrichment_put(provider, rest, &json);
        }
    }
}

/// What the UI asks for.
#[derive(Debug, Clone, Deserialize)]
pub struct EnrichRequest {
    pub library_path: String,
    pub track_ids: Vec<String>,
    /// Per the spec's `Original release` option: strip remix/remaster text and
    /// resolve to the earliest release. The manual calls this best practice for
    /// older tracks.
    #[serde(default)]
    pub original_release: bool,
    /// Off unless the user turned Discogs on; the token comes from the OS
    /// keychain, never from this payload.
    #[serde(default)]
    pub use_discogs: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EnrichPreview {
    pub tracks: Vec<enrichment::TrackProposal>,
    /// Tracks whose tags were too thin to search with at all. Surfaced rather
    /// than silently skipped — the manual points these users at Smart Fixes,
    /// and they cannot act on advice they never see.
    pub unsearchable: Vec<String>,
    /// Provider failures, verbatim. One source being down must not look like
    /// "no match found".
    pub errors: Vec<String>,
}

/// Build the search query for a track from whatever its tags actually hold.
///
/// A track whose artist is blank and whose title is the whole filename is the
/// normal state of a downloaded library, and searching for that string as a
/// title matches nothing. Splitting it is the difference between the feature
/// working on a real collection and not.
pub fn query_for(
    title: Option<&str>,
    artist: Option<&str>,
    original_release: bool,
) -> Option<Query> {
    let title = title.map(str::trim).filter(|s| !s.is_empty())?;
    let artist = artist.map(str::trim).filter(|s| !s.is_empty());

    let q = match artist {
        Some(a) => Query::new(Some(a), title),
        None => match enrichment::title::split_artist_title(title) {
            Some((a, t)) => Query::new(Some(&a), &t),
            // No artist and nothing to split: still worth a title-only search,
            // which MusicBrainz supports.
            None => Query::new(None, title),
        },
    };
    Some(if original_release {
        q.original_release()
    } else {
        q
    })
}

fn existing_of(t: &decks_core::rekordbox_db::Track) -> Existing {
    Existing {
        genre: t.genre.clone(),
        year: t.release_year.map(|y| y as i32),
        label: t.label.clone(),
        album: t.album.clone(),
    }
}

/// Look up metadata for a selection. Writes nothing.
#[tauri::command]
pub async fn enrich_preview(
    app: tauri::AppHandle,
    req: EnrichRequest,
) -> Result<EnrichPreview, String> {
    let discogs_token = if req.use_discogs {
        let t = crate::read_keychain(enrichment::discogs::KEYCHAIN_SERVICE).await?;
        if t.as_deref().map(str::trim).unwrap_or("").is_empty() {
            return Err(format!(
                "{} is switched on but has no token. Add one in Settings.",
                enrichment::discogs::NAME
            ));
        }
        t
    } else {
        None
    };

    let library_path = req.library_path.clone();
    let ids = req.track_ids.clone();
    let tracks = tauri::async_runtime::spawn_blocking(move || {
        let db = decks_core::rekordbox_db::RekordboxDb::open(std::path::Path::new(&library_path))
            .map_err(|e| e.to_string())?;
        let all = db.tracks().map_err(|e| e.to_string())?;
        Ok::<_, String>(
            all.into_iter()
                .filter(|t| ids.contains(&t.id))
                .collect::<Vec<_>>(),
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    let db = crate::cache_db(&app)?;
    let svc = Service::new(
        ReqwestHttp::new()?,
        DbCache {
            db: std::sync::Mutex::new(db),
        },
    );
    // `album_art` stays false here on purpose. `enrichment::cover_art` can
    // fetch and identify a cover, but nothing can yet write one into a file —
    // `crates/audio-tags` has no picture support. Offering the option would
    // download an image and discard it, which is the stub logic `CLAUDE.md`
    // forbids in production paths. See ADR-0016.
    let providers = Providers {
        discogs_token,
        album_art: false,
    };

    let mut out = EnrichPreview::default();
    for t in &tracks {
        let Some(q) = query_for(
            Some(t.title.as_str()),
            t.artist.as_deref(),
            req.original_release,
        ) else {
            out.unsearchable.push(t.id.clone());
            continue;
        };
        let (candidates, errs) = svc.lookup(&q, &providers).await;
        for e in errs {
            if !out.errors.contains(&e) {
                out.errors.push(e);
            }
        }
        out.tracks.push(enrichment::merge::build(
            &t.id,
            &existing_of(t),
            &candidates,
        ));
    }
    Ok(out)
}

/// Stage the proposals the user accepted.
///
/// Takes the proposals back rather than re-running the lookup, so what is
/// staged is what the user was shown — the same contract `resolve_duplicates`
/// and `apply_merge_relocate` have, and the reason a second lookup returning a
/// different answer cannot silently change what gets written.
#[tauri::command]
pub async fn enrich_stage(
    app: tauri::AppHandle,
    library_path: String,
    accepted: Vec<enrichment::TrackProposal>,
) -> Result<Vec<String>, String> {
    let db = crate::cache_db(&app)?;
    let mut staged = Vec::new();

    for track in &accepted {
        for p in &track.proposals {
            let record = db
                .stage_change(changes::NewChange {
                    library_path: Some(library_path.clone()),
                    kind: changes::ChangeKind::TrackMetadataEdit,
                    target_id: Some(track.track_id.clone()),
                    field: Some(p.field.clone()),
                    old_value: p.before.clone().map(|b| serde_json::json!(b)),
                    new_value: Some(serde_json::json!(p.after)),
                    // ADR-0008: the reason names the source, so a user reviewing
                    // the diff later can see who claimed this and decide
                    // whether they still believe it.
                    reason: Some(format!("Found on {}", p.source)),
                    confidence: Some(p.confidence as f64),
                })
                .map_err(|e| e.to_string())?;
            staged.push(record.id);
        }
    }
    Ok(staged)
}

/// Forget every cached provider answer.
#[tauri::command]
pub async fn enrich_clear_cache(app: tauri::AppHandle) -> Result<usize, String> {
    crate::cache_db(&app)?
        .enrichment_clear()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_with_both_tags_searches_with_both() {
        let q = query_for(Some("Around the World"), Some("Daft Punk"), false).unwrap();
        assert_eq!(q.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(q.title, "Around the World");
        assert!(!q.original_release);
    }

    #[test]
    fn a_filename_shaped_title_is_split_into_artist_and_title() {
        // The normal state of a downloaded library. Searching for the whole
        // string as a title matches nothing at all.
        let q = query_for(Some("Daft Punk - Around the World"), None, false).unwrap();
        assert_eq!(q.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(q.title, "Around the World");
    }

    #[test]
    fn a_real_artist_tag_is_never_overridden_by_a_split() {
        // "Re-Wired" would split wrongly if we tried; we do not try, because
        // the artist tag is already there.
        let q = query_for(Some("Something - Else"), Some("Real Artist"), false).unwrap();
        assert_eq!(q.artist.as_deref(), Some("Real Artist"));
        assert_eq!(q.title, "Something - Else");
    }

    #[test]
    fn a_title_with_no_artist_and_no_separator_still_searches() {
        let q = query_for(Some("Windowlicker"), None, false).unwrap();
        assert_eq!(q.artist, None);
        assert_eq!(q.title, "Windowlicker");
    }

    #[test]
    fn a_blank_artist_tag_counts_as_absent() {
        let q = query_for(Some("A - B"), Some("   "), false).unwrap();
        assert_eq!(q.artist.as_deref(), Some("A"));
    }

    #[test]
    fn a_track_with_no_title_is_unsearchable() {
        // Nothing to search with. The caller surfaces these rather than
        // silently skipping them, so the user can go and fix their tags.
        assert!(query_for(None, Some("Daft Punk"), false).is_none());
        assert!(query_for(Some("   "), Some("Daft Punk"), false).is_none());
    }

    #[test]
    fn the_original_release_flag_reaches_the_query() {
        let q = query_for(Some("Heroes (2017 Remaster)"), Some("Bowie"), true).unwrap();
        assert!(q.original_release);
        // The title is stripped at search time, not here — the query keeps what
        // the library actually holds so the cache key stays honest.
        assert_eq!(q.title, "Heroes (2017 Remaster)");
    }

    #[test]
    fn the_cache_key_splits_back_into_provider_and_rest() {
        // DbCache relies on this shape; if `Query::cache_key` changed its
        // separator, every lookup would silently miss.
        let key = Query::new(Some("A"), "T").cache_key("MusicBrainz");
        let (provider, rest) = key.split_once('\u{1}').unwrap();
        assert_eq!(provider, "MusicBrainz");
        assert!(rest.contains('\u{1}'));
    }

    #[test]
    fn the_cache_window_is_long_because_release_data_does_not_move() {
        assert_eq!(CACHE_MAX_AGE_SECS, 2_592_000);
    }
}
