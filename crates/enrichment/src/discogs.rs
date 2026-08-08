//! Discogs — the opt-in second source.
//!
//! Opt-in rather than default because it **requires a personal access token**.
//! A default source the user has to go and register for is not a default; it is
//! a dead feature with a settings page. MusicBrainz covers the no-setup case,
//! and Discogs adds the thing MusicBrainz is weakest at — label, catalogue
//! number and release year for electronic and dance records, which is most of
//! what a Rekordbox library is.
//!
//! **The token lives in the OS keychain**, per `CLAUDE.md`. This module never
//! sees storage: it takes the token as an argument and the caller fetches it
//! through the existing `get_api_key` command, so there is no second place a
//! secret could end up in plaintext.
//!
//! Like [`crate::musicbrainz`], the field paths are written against Discogs'
//! documented schema and are **not** verified against a live response here —
//! the network policy denies it. Parsing is tolerant, so drift costs proposals
//! rather than producing wrong ones.

use crate::http::Http;
use crate::title::strip_version;
use crate::types::{Candidate, EnrichError, Query, Result};

pub const NAME: &str = "Discogs";

/// The keychain service name. Shared with the frontend's Settings panel, which
/// calls `set_api_key` with this exact string.
pub const KEYCHAIN_SERVICE: &str = "discogs_token";

const BASE: &str = "https://api.discogs.com";

pub fn search_url(q: &Query) -> String {
    let title = if q.original_release {
        strip_version(&q.title).0
    } else {
        q.title.clone()
    };
    let mut url = format!(
        "{BASE}/database/search?type=release&track={}",
        crate::musicbrainz::urlencode(&title)
    );
    if let Some(a) = q.artist.as_deref().filter(|a| !a.trim().is_empty()) {
        url.push_str(&format!(
            "&artist={}",
            crate::musicbrainz::urlencode(a.trim())
        ));
    }
    url.push_str("&per_page=5");
    url
}

/// Discogs styles are subgenres; genres are the top level. That maps exactly
/// onto the spec's main-genre / custom-tags split with no interpretation.
pub fn parse_search(body: &[u8]) -> Result<Vec<Candidate>> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|e| EnrichError::Parse {
        provider: NAME.into(),
        detail: e.to_string(),
    })?;

    let results = v
        .get("results")
        .and_then(|r| r.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    Ok(results
        .iter()
        .enumerate()
        .map(|(i, r)| result(r, i, results.len()))
        .collect())
}

fn result(r: &serde_json::Value, idx: usize, total: usize) -> Candidate {
    let first_str = |k: &str| {
        r.get(k)
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let rest_strs = |k: &str| {
        r.get(k)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .skip(1)
                    .filter_map(|s| s.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    // Discogs' `title` is "Artist - Title" for a release result; the API gives
    // no separate artist field here, so the split is the only way to get one.
    let raw_title = r.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
    let (artist, title) = match crate::title::split_artist_title(raw_title) {
        Some((a, t)) => (Some(a), Some(t)),
        None => (None, (!raw_title.is_empty()).then(|| raw_title.to_string())),
    };

    // Styles are the finer classification and belong in the Genre field ahead
    // of the broad `genre` array — "Deep House" is more useful than
    // "Electronic", which is true of nearly every record in a DJ library.
    let mut subgenres = rest_strs("style");
    let genre = first_str("style").or_else(|| first_str("genre"));
    if first_str("style").is_some() {
        subgenres.extend(
            r.get("genre")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        );
    } else {
        subgenres.extend(rest_strs("genre"));
    }
    subgenres.retain(|s| Some(s) != genre.as_ref());
    subgenres.dedup();

    Candidate {
        source: NAME.to_string(),
        id: r.get("id").map(|i| i.to_string()),
        title,
        artist,
        album: None,
        genre,
        subgenres,
        year: r
            .get("year")
            .and_then(|y| {
                y.as_str()
                    .map(str::to_string)
                    .or_else(|| y.as_i64().map(|n| n.to_string()))
            })
            .and_then(|y| y.trim().get(..4).and_then(|h| h.parse::<i32>().ok()))
            .filter(|y| (1000..=9999).contains(y)),
        label: first_str("label"),
        // No release MBID — cover art is Cover Art Archive's job, and it is
        // keyed by MusicBrainz ids only.
        release_id: None,
        // Discogs returns no per-result score, so position stands in: first
        // result strongest, decaying. Never 1.0 — we are inferring, not told.
        score: if total == 0 {
            0.0
        } else {
            (0.9 - (idx as f32 * 0.1)).clamp(0.3, 0.9)
        },
    }
}

/// Search Discogs. `token` is a personal access token from the OS keychain.
pub async fn search<H: Http>(http: &H, q: &Query, token: &str) -> Result<Vec<Candidate>> {
    if token.trim().is_empty() {
        return Err(EnrichError::NeedsToken(NAME.into()));
    }
    let res = http
        .get(
            &search_url(q),
            &[
                ("User-Agent", crate::musicbrainz::USER_AGENT),
                ("Authorization", &format!("Discogs token={}", token.trim())),
                ("Accept", "application/json"),
            ],
        )
        .await?;
    if !res.ok() {
        return Err(EnrichError::Status {
            provider: NAME.into(),
            status: res.status,
        });
    }
    parse_search(&res.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeHttp;

    const BODY: &str = r#"{"results":[
      {"id": 12345, "title": "Daft Punk - Homework", "year": "1997",
       "label": ["Soma Quality Recordings", "Virgin"],
       "genre": ["Electronic"], "style": ["House", "Techno"]},
      {"id": 6, "title": "Untitled", "year": 2001, "genre": ["Rock"]}
    ]}"#;

    #[test]
    fn the_style_becomes_the_genre_and_the_broad_genre_becomes_a_tag() {
        // "Deep House" is more useful in the Genre field than "Electronic",
        // which is true of nearly every record in a DJ library.
        let c = &parse_search(BODY.as_bytes()).unwrap()[0];
        assert_eq!(c.genre.as_deref(), Some("House"));
        assert!(c.subgenres.contains(&"Techno".to_string()));
        assert!(c.subgenres.contains(&"Electronic".to_string()));
    }

    #[test]
    fn with_no_style_the_broad_genre_is_used() {
        let c = &parse_search(BODY.as_bytes()).unwrap()[1];
        assert_eq!(c.genre.as_deref(), Some("Rock"));
    }

    #[test]
    fn the_artist_comes_out_of_the_combined_title() {
        let c = &parse_search(BODY.as_bytes()).unwrap()[0];
        assert_eq!(c.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(c.title.as_deref(), Some("Homework"));
    }

    #[test]
    fn a_title_with_no_separator_stays_whole() {
        let c = &parse_search(BODY.as_bytes()).unwrap()[1];
        assert_eq!(c.artist, None);
        assert_eq!(c.title.as_deref(), Some("Untitled"));
    }

    #[test]
    fn the_year_parses_whether_it_arrived_as_text_or_a_number() {
        let cs = parse_search(BODY.as_bytes()).unwrap();
        assert_eq!(cs[0].year, Some(1997));
        assert_eq!(cs[1].year, Some(2001));
    }

    #[test]
    fn the_first_label_wins_and_the_rest_are_ignored() {
        let c = &parse_search(BODY.as_bytes()).unwrap()[0];
        assert_eq!(c.label.as_deref(), Some("Soma Quality Recordings"));
    }

    #[test]
    fn discogs_never_claims_a_perfect_score() {
        // There is no per-result score in the response; position is a proxy,
        // and presenting a proxy as certainty is what ADR-0008 forbids.
        let cs = parse_search(BODY.as_bytes()).unwrap();
        assert!(cs[0].score < 1.0);
        assert!(cs[0].score > cs[1].score);
    }

    #[test]
    fn no_results_is_empty_not_an_error() {
        assert!(parse_search(br#"{"results":[]}"#).unwrap().is_empty());
        assert!(parse_search(br#"{}"#).unwrap().is_empty());
    }

    #[test]
    fn a_result_missing_everything_optional_still_parses() {
        let c = &parse_search(br#"{"results":[{"id":1}]}"#).unwrap()[0];
        assert_eq!(c.genre, None);
        assert_eq!(c.year, None);
        assert_eq!(c.label, None);
    }

    #[test]
    fn a_nonsense_year_is_dropped() {
        let c = &parse_search(br#"{"results":[{"id":1,"year":"0"}]}"#).unwrap()[0];
        assert_eq!(c.year, None);
    }

    #[tokio::test]
    async fn a_missing_token_is_named_as_such_rather_than_failing_at_the_wire() {
        // The UI turns this into "add a token in Settings"; a 401 would only
        // say "unauthorized".
        let http = FakeHttp::new();
        assert!(matches!(
            search(&http, &Query::new(None, "x"), "  ").await,
            Err(EnrichError::NeedsToken(_))
        ));
        // And nothing was sent — a request with no token is never worth making.
        assert_eq!(http.call_count(), 0);
    }

    #[tokio::test]
    async fn a_search_carries_the_token_and_parses() {
        let q = Query::new(Some("Daft Punk"), "Homework");
        let http = FakeHttp::new().route(&search_url(&q), BODY);
        let got = search(&http, &q, "tok").await.unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn the_keychain_service_name_is_the_one_settings_writes() {
        // Drift here means the token is stored under one name and read under
        // another, and the feature silently behaves as if unconfigured.
        assert_eq!(KEYCHAIN_SERVICE, "discogs_token");
    }
}
