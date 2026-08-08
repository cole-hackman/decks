//! Where to go and buy the ones you haven't got.
//!
//! Covers two Lexicon features as far as they can honestly be covered without
//! credentials:
//!
//! - **Track Matcher → onward search** (`08-streaming.md §Track Matcher`):
//!   "Unmatched entries can be sent onward to Spotify, Tidal or Beatport to
//!   hunt down."
//! - **Store Links** (`§Store Links`): "automates the tedious per-track search
//!   across online stores".
//!
//! ## What this is not
//!
//! Lexicon's Store Links also does **price comparison**, and its Send To pushes
//! a matched playlist into the service. Both need authenticated APIs — a
//! registered application and a user token per service — and neither is built.
//! What is built is the tedious part: constructing the correct search URL for
//! each store, per track, so a DJ working through fifty unmatched request-list
//! entries opens fifty right-first-time searches instead of typing fifty
//! queries.
//!
//! That boundary is deliberate and worth stating plainly rather than shipping a
//! half-authenticated client that fails at the first request. A search URL is
//! **honest**: it makes no claim the track exists, no claim about price, and it
//! cannot be wrong in a way that costs the user anything.

use serde::{Deserialize, Serialize};

/// A store or service an unmatched entry can be sent onward to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Beatport,
    Bandcamp,
    Discogs,
    Spotify,
    Tidal,
    SoundCloud,
    /// Not a store, but the fastest way to identify a half-remembered request
    /// before buying it anywhere.
    YouTube,
}

impl Store {
    pub fn label(&self) -> &'static str {
        match self {
            Store::Beatport => "Beatport",
            Store::Bandcamp => "Bandcamp",
            Store::Discogs => "Discogs",
            Store::Spotify => "Spotify",
            Store::Tidal => "Tidal",
            Store::SoundCloud => "SoundCloud",
            Store::YouTube => "YouTube",
        }
    }

    /// The three the spec names for Track Matcher's onward search, plus the
    /// stores a DJ actually buys from.
    pub fn all() -> &'static [Store] {
        &[
            Store::Beatport,
            Store::Bandcamp,
            Store::Discogs,
            Store::Spotify,
            Store::Tidal,
            Store::SoundCloud,
            Store::YouTube,
        ]
    }
}

/// Percent-encode a query value.
///
/// Space becomes `%20` rather than `+`: `+` is only valid in
/// `application/x-www-form-urlencoded`, and several of these stores put the
/// query in a path segment where a literal `+` stays a plus sign and breaks the
/// search.
fn encode(s: &str) -> String {
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

/// The search text for a track: `Artist Title`, or just the title.
///
/// Plain words rather than a fielded query. Every one of these stores takes a
/// free-text search, and the fielded syntaxes differ per store and change
/// without notice — a query that silently stops matching is worse than one that
/// is merely broad.
pub fn search_terms(title: &str, artist: Option<&str>) -> String {
    let title = title.trim();
    match artist.map(str::trim).filter(|a| !a.is_empty()) {
        Some(a) => format!("{a} {title}"),
        None => title.to_string(),
    }
}

/// A search URL for one track at one store.
///
/// Returns `None` for an empty title: a search URL with no query opens the
/// store's front page, which looks like the feature working and is not.
pub fn url(store: Store, title: &str, artist: Option<&str>) -> Option<String> {
    let terms = search_terms(title, artist);
    if terms.trim().is_empty() {
        return None;
    }
    let q = encode(&terms);
    Some(match store {
        Store::Beatport => format!("https://www.beatport.com/search?q={q}"),
        Store::Bandcamp => format!("https://bandcamp.com/search?q={q}"),
        Store::Discogs => format!("https://www.discogs.com/search/?q={q}&type=release"),
        Store::Spotify => format!("https://open.spotify.com/search/{q}"),
        Store::Tidal => format!("https://listen.tidal.com/search?q={q}"),
        Store::SoundCloud => format!("https://soundcloud.com/search?q={q}"),
        Store::YouTube => format!("https://www.youtube.com/results?search_query={q}"),
    })
}

/// One track's links across a set of stores.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackLinks {
    pub title: String,
    pub artist: Option<String>,
    /// `(store label, url)`, in the order the stores were requested.
    pub links: Vec<(String, String)>,
}

/// Build links for a list of tracks across a list of stores.
pub fn links_for(tracks: &[(String, Option<String>)], stores: &[Store]) -> Vec<TrackLinks> {
    tracks
        .iter()
        .map(|(title, artist)| TrackLinks {
            title: title.clone(),
            artist: artist.clone(),
            links: stores
                .iter()
                .filter_map(|s| {
                    url(*s, title, artist.as_deref()).map(|u| (s.label().to_string(), u))
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_terms_are_artist_then_title() {
        assert_eq!(
            search_terms("Around the World", Some("Daft Punk")),
            "Daft Punk Around the World"
        );
    }

    #[test]
    fn a_missing_artist_searches_the_title_alone() {
        assert_eq!(search_terms("Windowlicker", None), "Windowlicker");
        assert_eq!(search_terms("Windowlicker", Some("  ")), "Windowlicker");
    }

    #[test]
    fn a_space_encodes_as_percent_20_not_plus() {
        // Several of these put the query in a path segment, where a literal `+`
        // stays a plus sign and breaks the search.
        let u = url(Store::Spotify, "Around the World", Some("Daft Punk")).unwrap();
        assert!(u.contains("%20"), "{u}");
        assert!(!u.contains('+'), "{u}");
    }

    #[test]
    fn special_characters_do_not_escape_the_query() {
        // A title with `&` or `#` would otherwise truncate the URL or add a
        // parameter the store never sees as part of the search.
        let u = url(Store::Beatport, "Sex & Drugs #1", Some("A/B")).unwrap();
        assert!(u.contains("%26"), "{u}");
        assert!(u.contains("%23"), "{u}");
        assert!(u.contains("%2F"), "{u}");
        assert_eq!(u.matches('?').count(), 1, "{u}");
    }

    #[test]
    fn every_store_produces_a_distinct_https_url() {
        for s in Store::all() {
            let u = url(*s, "Track", Some("Artist")).unwrap();
            assert!(u.starts_with("https://"), "{s:?} -> {u}");
        }
        let all: Vec<String> = Store::all()
            .iter()
            .map(|s| url(*s, "T", Some("A")).unwrap())
            .collect();
        let mut deduped = all.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), all.len(), "two stores share a URL");
    }

    #[test]
    fn the_three_stores_the_spec_names_are_all_present() {
        // "Unmatched entries can be sent onward to Spotify, Tidal or Beatport."
        for s in [Store::Spotify, Store::Tidal, Store::Beatport] {
            assert!(Store::all().contains(&s), "{s:?} missing");
        }
    }

    #[test]
    fn an_empty_title_yields_no_link_rather_than_a_front_page() {
        // A query-less search URL opens the store's home page, which looks like
        // the feature working and is not.
        assert_eq!(url(Store::Beatport, "", None), None);
        assert_eq!(url(Store::Beatport, "   ", Some("  ")), None);
    }

    #[test]
    fn links_are_built_in_the_order_the_stores_were_asked_for() {
        let tracks = vec![("T".to_string(), Some("A".to_string()))];
        let got = links_for(&tracks, &[Store::Tidal, Store::Beatport]);
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0]
                .links
                .iter()
                .map(|(l, _)| l.as_str())
                .collect::<Vec<_>>(),
            vec!["Tidal", "Beatport"]
        );
    }

    #[test]
    fn a_track_with_no_usable_title_gets_an_empty_link_list_not_a_broken_one() {
        let tracks = vec![("".to_string(), None)];
        let got = links_for(&tracks, Store::all());
        assert_eq!(got.len(), 1);
        assert!(got[0].links.is_empty());
    }

    #[test]
    fn no_store_url_carries_a_credential_or_an_api_host() {
        // These are plain public search pages by design. An api. host here
        // would mean somebody had started down the authenticated path without
        // the token plumbing that requires.
        for s in Store::all() {
            let u = url(*s, "T", Some("A")).unwrap();
            assert!(!u.contains("api."), "{s:?} -> {u}");
            assert!(!u.contains("token"), "{s:?} -> {u}");
            assert!(!u.contains("key="), "{s:?} -> {u}");
        }
    }
}
