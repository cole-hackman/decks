//! MusicBrainz — the default metadata source.
//!
//! Chosen as the default because it needs no account, no key and no
//! registration, and its data is CC0. That matters more than raw coverage: a
//! default source that requires the user to go and get a token is a default
//! that does not work out of the box.
//!
//! **Their rate limit is a condition of use, not a courtesy.** The service
//! documents one request per second per client and a User-Agent identifying the
//! application with contact information; clients that ignore either get
//! blocked. [`crate::rate_limit`] enforces the first, [`USER_AGENT`] the
//! second, and neither is optional.
//!
//! ## Unverified against a live response
//!
//! The field paths below are written against MusicBrainz's documented web
//! service schema; they have **not** been exercised against the real service,
//! because this container's network policy denies `musicbrainz.org`. Recorded
//! in `docs/lexicon/GAPS.md` §Environment blockers along with the one command
//! that checks them.
//!
//! Every parse is tolerant by construction — an absent or renamed field yields
//! `None` for that value rather than an error — so the failure mode of a schema
//! drift is "fewer proposals", never a wrong value written into a library. That
//! asymmetry is why this ships unverified and ANLZ writing does not.

use crate::http::Http;
use crate::title::{lucene_escape, strip_version};
use crate::types::{Candidate, EnrichError, Query, Result};

pub const NAME: &str = "MusicBrainz";

/// Sent on every request. MusicBrainz requires an application name, a version
/// and a contact URL, and rejects generic agents.
pub const USER_AGENT: &str = "decks/0.1.0 ( https://github.com/cole-hackman/decks )";

const BASE: &str = "https://musicbrainz.org/ws/2";

/// Build the recording-search URL for a query.
///
/// Split out from the fetch so the query construction — which is where the
/// escaping and the original-release handling live — is testable without a
/// transport at all.
pub fn search_url(q: &Query, limit: u8) -> String {
    let title = if q.original_release {
        strip_version(&q.title).0
    } else {
        q.title.clone()
    };

    let mut lucene = format!("recording:\"{}\"", lucene_escape(&title));
    if let Some(artist) = q.artist.as_deref().filter(|a| !a.trim().is_empty()) {
        lucene.push_str(&format!(" AND artist:\"{}\"", lucene_escape(artist)));
    }

    format!(
        "{BASE}/recording?query={}&fmt=json&limit={limit}",
        urlencode(&lucene)
    )
}

/// Percent-encode for a query string.
///
/// Hand-rolled rather than pulling in a URL crate for one function. The
/// unreserved set is RFC 3986's; everything else is encoded, including the
/// space, which must not become `+` here because MusicBrainz reads the value
/// as a path-style component.
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Parse a recording-search response into candidates, best first.
pub fn parse_search(body: &[u8], original_release: bool) -> Result<Vec<Candidate>> {
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|e| EnrichError::Parse {
        provider: NAME.into(),
        detail: e.to_string(),
    })?;

    let recordings = v
        .get("recordings")
        .and_then(|r| r.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let mut out: Vec<Candidate> = recordings
        .iter()
        .map(|r| recording(r, original_release))
        .collect();
    // MusicBrainz returns these ordered by score already, but the
    // original-release option re-ranks by date, so never rely on arrival order.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

fn recording(r: &serde_json::Value, original_release: bool) -> Candidate {
    let str_at = |v: &serde_json::Value, k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    // Artist credit is an array of parts with join phrases; the display name is
    // the concatenation, which is what a DJ's tag actually looks like.
    let artist = r
        .get("artist-credit")
        .and_then(|a| a.as_array())
        .map(|parts| {
            parts
                .iter()
                .map(|p| {
                    let name = p
                        .get("name")
                        .and_then(|n| n.as_str())
                        .or_else(|| {
                            p.get("artist")
                                .and_then(|a| a.get("name"))
                                .and_then(|n| n.as_str())
                        })
                        .unwrap_or_default();
                    let join = p.get("joinphrase").and_then(|j| j.as_str()).unwrap_or("");
                    format!("{name}{join}")
                })
                .collect::<String>()
        });

    let releases = r
        .get("releases")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    // With `original_release`, the oldest dated release wins — that is the
    // whole point of the option. Otherwise take the first, which is
    // MusicBrainz's own best guess.
    let chosen = if original_release {
        releases
            .iter()
            .filter(|rel| release_year(rel).is_some())
            .min_by_key(|rel| release_year(rel).unwrap_or(i32::MAX))
            .or_else(|| releases.first())
    } else {
        releases.first()
    };

    let (album, release_id, label) = match chosen {
        Some(rel) => (
            str_at(rel, "title"),
            str_at(rel, "id"),
            rel.get("label-info")
                .and_then(|l| l.as_array())
                .and_then(|a| a.first())
                .and_then(|li| li.get("label"))
                .and_then(|l| l.get("name"))
                .and_then(|n| n.as_str())
                .map(str::to_string),
        ),
        None => (None, None, None),
    };

    // Prefer the chosen release's date; fall back to the recording's own.
    let year = chosen
        .and_then(release_year)
        .or_else(|| year_of(r.get("first-release-date").and_then(|d| d.as_str())));

    let (genre, subgenres) = genres(r);

    Candidate {
        source: NAME.to_string(),
        id: str_at(r, "id"),
        title: str_at(r, "title"),
        artist: artist
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty()),
        album,
        genre,
        subgenres,
        year,
        label,
        release_id,
        // MusicBrainz scores 0–100; a missing score is treated as a weak match
        // rather than a perfect one.
        score: r
            .get("score")
            .and_then(|s| s.as_f64())
            .map(|s| (s / 100.0).clamp(0.0, 1.0) as f32)
            .unwrap_or(0.5),
    }
}

fn release_year(rel: &serde_json::Value) -> Option<i32> {
    year_of(rel.get("date").and_then(|d| d.as_str()))
        .or_else(|| year_of(rel.get("first-release-date").and_then(|d| d.as_str())))
}

/// The year out of an ISO-ish date. MusicBrainz dates may be `1997`,
/// `1997-02` or `1997-02-17`, and partial dates are common on older releases.
fn year_of(date: Option<&str>) -> Option<i32> {
    let d = date?.trim();
    let head = d.split('-').next()?;
    let y: i32 = head.parse().ok()?;
    // A four-digit sanity range. A zero or a five-digit year is a data error,
    // and writing it into a library is worse than leaving the field alone.
    (1000..=9999).contains(&y).then_some(y)
}

/// Split MusicBrainz's tags into one main genre and the rest.
///
/// Per `docs/lexicon/07-health.md §Find Tags`: the single-value genre field
/// gets the main genre; everything else becomes custom tags. The most-voted tag
/// is the main one, which is as close to "main genre" as a folksonomy gets.
fn genres(r: &serde_json::Value) -> (Option<String>, Vec<String>) {
    let mut tags: Vec<(String, i64)> = r
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    let name = t.get("name")?.as_str()?.trim();
                    (!name.is_empty()).then(|| {
                        (
                            name.to_string(),
                            t.get("count").and_then(|c| c.as_i64()).unwrap_or(0),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // A tag with no votes carries no signal; including it would let one
    // person's typo become somebody's Genre field.
    tags.retain(|(_, count)| *count > 0);
    // Sort by votes, then by name so equal-vote ties are deterministic rather
    // than dependent on the server's ordering.
    tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut names = tags.into_iter().map(|(n, _)| title_case(&n));
    let main = names.next();
    (main, names.collect())
}

/// `deep house` → `Deep House`. MusicBrainz tags are lowercase by convention;
/// a Genre field full of lowercase looks broken next to Rekordbox's own.
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Search MusicBrainz for a track.
pub async fn search<H: Http>(http: &H, q: &Query, limit: u8) -> Result<Vec<Candidate>> {
    let url = search_url(q, limit);
    let res = http
        .get(
            &url,
            &[("User-Agent", USER_AGENT), ("Accept", "application/json")],
        )
        .await?;
    if !res.ok() {
        return Err(EnrichError::Status {
            provider: NAME.into(),
            status: res.status,
        });
    }
    parse_search(&res.body, q.original_release)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeHttp;

    const BODY: &str = r#"{
      "recordings": [{
        "id": "rec-1",
        "score": 100,
        "title": "Around the World",
        "first-release-date": "1997-02-17",
        "artist-credit": [{"name": "Daft Punk", "joinphrase": ""}],
        "tags": [
          {"name": "house", "count": 9},
          {"name": "french house", "count": 4},
          {"name": "typo", "count": 0}
        ],
        "releases": [
          {"id": "rel-new", "title": "Musique Vol. 1", "date": "2006",
           "label-info": [{"label": {"name": "Virgin"}}]},
          {"id": "rel-old", "title": "Homework", "date": "1997-01-20",
           "label-info": [{"label": {"name": "Soma"}}]}
        ]
      }]
    }"#;

    #[test]
    fn the_query_quotes_and_escapes_both_fields() {
        let url = search_url(&Query::new(Some("A:B"), "Hey!"), 5);
        // The colon and the bang must arrive escaped, or the request 400s.
        assert!(url.contains("%5C%3A"), "{url}");
        assert!(url.contains("%5C%21"), "{url}");
        assert!(url.ends_with("&fmt=json&limit=5"));
    }

    #[test]
    fn a_missing_artist_leaves_the_clause_out_entirely() {
        // Not `artist:""`, which matches nothing rather than anything.
        let url = search_url(&Query::new(None, "Windowlicker"), 5);
        assert!(!url.contains("artist"), "{url}");
        let url = search_url(&Query::new(Some("   "), "Windowlicker"), 5);
        assert!(!url.contains("artist"), "{url}");
    }

    #[test]
    fn the_original_release_option_searches_for_the_stripped_title() {
        let q = Query::new(Some("Daft Punk"), "Around the World (Extended Mix)").original_release();
        let url = search_url(&q, 5);
        assert!(!url.to_lowercase().contains("extended"), "{url}");
    }

    #[test]
    fn a_space_encodes_as_percent_20_not_plus() {
        assert_eq!(urlencode("a b"), "a%20b");
    }

    #[test]
    fn a_recording_parses_into_a_candidate() {
        let c = &parse_search(BODY.as_bytes(), false).unwrap()[0];
        assert_eq!(c.source, NAME);
        assert_eq!(c.id.as_deref(), Some("rec-1"));
        assert_eq!(c.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(c.title.as_deref(), Some("Around the World"));
        assert_eq!(c.score, 1.0);
    }

    #[test]
    fn the_top_voted_tag_is_the_genre_and_the_rest_are_subgenres() {
        // The spec's own good idea: keep the single-value field clean, put the
        // detail in custom tags.
        let c = &parse_search(BODY.as_bytes(), false).unwrap()[0];
        assert_eq!(c.genre.as_deref(), Some("House"));
        assert_eq!(c.subgenres, vec!["French House"]);
    }

    #[test]
    fn a_tag_nobody_voted_for_is_dropped() {
        // Otherwise one person's typo becomes somebody's Genre field.
        let c = &parse_search(BODY.as_bytes(), false).unwrap()[0];
        assert!(!c.subgenres.iter().any(|s| s == "Typo"));
    }

    #[test]
    fn without_the_option_the_first_release_is_used() {
        let c = &parse_search(BODY.as_bytes(), false).unwrap()[0];
        assert_eq!(c.album.as_deref(), Some("Musique Vol. 1"));
        assert_eq!(c.year, Some(2006));
        assert_eq!(c.label.as_deref(), Some("Virgin"));
    }

    #[test]
    fn with_the_option_the_oldest_release_wins() {
        // The reason the option exists: a 2006 compilation should not become
        // the release year of a 1997 record.
        let c = &parse_search(BODY.as_bytes(), true).unwrap()[0];
        assert_eq!(c.album.as_deref(), Some("Homework"));
        assert_eq!(c.year, Some(1997));
        assert_eq!(c.label.as_deref(), Some("Soma"));
        assert_eq!(c.release_id.as_deref(), Some("rel-old"));
    }

    #[test]
    fn a_partial_date_still_yields_a_year() {
        // Common on older releases: "1977", "1977-06".
        assert_eq!(year_of(Some("1977")), Some(1977));
        assert_eq!(year_of(Some("1977-06")), Some(1977));
        assert_eq!(year_of(Some("1977-06-02")), Some(1977));
    }

    #[test]
    fn a_nonsense_date_yields_nothing_rather_than_a_wrong_year() {
        assert_eq!(year_of(Some("0000")), None);
        assert_eq!(year_of(Some("")), None);
        assert_eq!(year_of(Some("not-a-date")), None);
        assert_eq!(year_of(None), None);
    }

    #[test]
    fn a_response_with_no_recordings_is_empty_not_an_error() {
        // "Nothing found" is a normal outcome and the UI distinguishes it.
        assert!(parse_search(br#"{"recordings":[]}"#, false)
            .unwrap()
            .is_empty());
        assert!(parse_search(br#"{"count":0}"#, false).unwrap().is_empty());
    }

    #[test]
    fn a_recording_missing_every_optional_field_still_parses() {
        // The schema-drift failure mode: fewer proposals, never a wrong value.
        let c = &parse_search(br#"{"recordings":[{"id":"x"}]}"#, false).unwrap()[0];
        assert_eq!(c.id.as_deref(), Some("x"));
        assert_eq!(c.title, None);
        assert_eq!(c.artist, None);
        assert_eq!(c.genre, None);
        assert_eq!(c.year, None);
        assert!(c.subgenres.is_empty());
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(matches!(
            parse_search(b"not json", false),
            Err(EnrichError::Parse { .. })
        ));
    }

    #[test]
    fn a_multi_part_artist_credit_joins_the_way_the_tag_reads() {
        let body = br#"{"recordings":[{"id":"x","artist-credit":[
          {"name":"A","joinphrase":" & "},{"name":"B","joinphrase":""}]}]}"#;
        let c = &parse_search(body, false).unwrap()[0];
        assert_eq!(c.artist.as_deref(), Some("A & B"));
    }

    #[tokio::test]
    async fn a_search_sends_the_required_user_agent() {
        // Not decoration: MusicBrainz blocks clients with a generic agent.
        assert!(USER_AGENT.starts_with("decks/"));
        assert!(USER_AGENT.contains("github.com"));

        let q = Query::new(Some("Daft Punk"), "Around the World");
        let http = FakeHttp::new().route(&search_url(&q, 5), BODY);
        let got = search(&http, &q, 5).await.unwrap();
        assert_eq!(got.len(), 1);
    }

    #[tokio::test]
    async fn a_rate_limit_response_surfaces_as_a_status_error() {
        // 503 is what MusicBrainz returns when you exceed the limit, and the
        // caller has to be able to tell that from "no match".
        let q = Query::new(None, "x");
        let http = FakeHttp::new().route_status(&search_url(&q, 5), 503, b"");
        assert!(matches!(
            search(&http, &q, 5).await,
            Err(EnrichError::Status { status: 503, .. })
        ));
    }
}
