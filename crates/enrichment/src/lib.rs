//! Metadata enrichment: MusicBrainz + Cover Art Archive by default, Discogs
//! opt-in.
//!
//! Implements `docs/lexicon/07-health.md §Find Tags & Album Art`. Two places
//! where this deliberately diverges from Lexicon, both forced:
//!
//! - **No SonoVault.** Lexicon's own metadata backend is not a public API.
//!   MusicBrainz is the closest open equivalent, needs no account, and its data
//!   is CC0.
//! - **No Spotify.** Lexicon fills Energy/Danceability/Popularity/Happiness
//!   from Spotify's `audio-features`, which was deprecated on 2024-11-27 and
//!   returns 403 to applications registered since. Our Energy comes from our
//!   own analysis instead (ADR-0015); Danceability, Popularity and Happiness
//!   have no honest source and are not faked. See ADR-0012.
//!
//! ## The privacy contract
//!
//! `CLAUDE.md`: "The library never leaves the machine except through enrichment
//! APIs the user explicitly enables, and those go through a local cache first."
//! Both halves are structural here rather than aspirational:
//!
//! - Every outbound request goes through the [`http::Http`] trait. Grep for its
//!   implementors to enumerate the entire network surface of this crate.
//! - [`Service::lookup`] consults the cache before any provider, so a repeated
//!   lookup for the same artist/title never leaves the machine twice.
//! - What is sent is an artist and a title. Never a path, never a library
//!   identifier, never the collection.

pub mod cover_art;
pub mod discogs;
pub mod http;
pub mod merge;
pub mod musicbrainz;
pub mod rate_limit;
#[cfg(feature = "reqwest")]
pub mod reqwest_http;
pub mod title;
pub mod types;

pub use merge::Existing;
pub use types::{Candidate, EnrichError, FieldProposal, Query, Result, TrackProposal};

use rate_limit::RateLimiter;

/// Where a cached provider answer is read from and written to.
///
/// A trait rather than a concrete `CacheDb` so this crate does not depend on
/// `cache`, and so the cache-before-network guarantee is testable with a stub.
pub trait ResponseCache: Send + Sync {
    /// Cached candidates for this key, if any are still considered current.
    fn get(&self, key: &str) -> Option<Vec<Candidate>>;
    fn put(&self, key: &str, candidates: &[Candidate]);
}

/// A cache that stores nothing, for callers that genuinely want a live read.
pub struct NoCache;

impl ResponseCache for NoCache {
    fn get(&self, _key: &str) -> Option<Vec<Candidate>> {
        None
    }
    fn put(&self, _key: &str, _candidates: &[Candidate]) {}
}

/// Which sources are switched on.
#[derive(Debug, Clone, Default)]
pub struct Providers {
    /// On by default and not switchable off: it is the only source that needs
    /// no setup, and a lookup with every source off is not a feature.
    pub discogs_token: Option<String>,
    /// Fetch album art alongside the metadata.
    pub album_art: bool,
}

/// The enrichment entry point.
pub struct Service<H: http::Http, C: ResponseCache> {
    http: H,
    cache: C,
    mb_limit: RateLimiter,
    discogs_limit: RateLimiter,
    art_limit: RateLimiter,
}

impl<H: http::Http, C: ResponseCache> Service<H, C> {
    pub fn new(http: H, cache: C) -> Self {
        Self {
            http,
            cache,
            mb_limit: RateLimiter::new(rate_limit::MUSICBRAINZ_INTERVAL),
            discogs_limit: RateLimiter::new(rate_limit::DISCOGS_INTERVAL),
            art_limit: RateLimiter::new(rate_limit::COVER_ART_INTERVAL),
        }
    }

    /// Candidates for one query, cache first, in provider-preference order.
    ///
    /// Provider failures are collected rather than propagated: Discogs being
    /// down or the token being stale must not lose the MusicBrainz answer that
    /// already arrived. The caller gets whatever was found plus a list of what
    /// went wrong, and the UI shows both.
    pub async fn lookup(&self, q: &Query, providers: &Providers) -> (Vec<Candidate>, Vec<String>) {
        let mut out = Vec::new();
        let mut errors = Vec::new();

        match self
            .cached_or_fetch(q, musicbrainz::NAME, || async {
                self.mb_limit.acquire().await;
                musicbrainz::search(&self.http, q, 5).await
            })
            .await
        {
            Ok(cs) => out.extend(cs.into_iter().take(1)),
            Err(e) => errors.push(format!("{}: {e}", musicbrainz::NAME)),
        }

        if let Some(token) = providers
            .discogs_token
            .as_deref()
            .filter(|t| !t.trim().is_empty())
        {
            match self
                .cached_or_fetch(q, discogs::NAME, || async {
                    self.discogs_limit.acquire().await;
                    discogs::search(&self.http, q, token).await
                })
                .await
            {
                Ok(cs) => out.extend(cs.into_iter().take(1)),
                Err(e) => errors.push(format!("{}: {e}", discogs::NAME)),
            }
        }

        (out, errors)
    }

    async fn cached_or_fetch<F, Fut>(
        &self,
        q: &Query,
        provider: &str,
        fetch: F,
    ) -> Result<Vec<Candidate>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<Candidate>>>,
    {
        let key = q.cache_key(provider);
        if let Some(hit) = self.cache.get(&key) {
            return Ok(hit);
        }
        let fresh = fetch().await?;
        // A miss is cached too. "This artist/title has no match" is worth
        // remembering: without it, a bulk run over a library of bootlegs pays
        // the full rate-limited round trip again on every re-run.
        self.cache.put(&key, &fresh);
        Ok(fresh)
    }

    /// Album art for a candidate, if it carries a release id and the file can
    /// hold a picture at all.
    pub async fn album_art(
        &self,
        candidate: &Candidate,
        file_path: &str,
    ) -> Result<Option<cover_art::Art>> {
        if !cover_art::supports_art(file_path) {
            return Ok(None);
        }
        let Some(release) = candidate.release_id.as_deref() else {
            return Ok(None);
        };
        self.art_limit.acquire().await;
        cover_art::fetch(&self.http, release).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::fake::FakeHttp;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemCache(Mutex<HashMap<String, Vec<Candidate>>>);

    impl ResponseCache for MemCache {
        fn get(&self, key: &str) -> Option<Vec<Candidate>> {
            self.0.lock().unwrap().get(key).cloned()
        }
        fn put(&self, key: &str, candidates: &[Candidate]) {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_string(), candidates.to_vec());
        }
    }

    const MB_BODY: &str = r#"{"recordings":[{"id":"r1","score":95,"title":"T",
      "artist-credit":[{"name":"A","joinphrase":""}],
      "tags":[{"name":"house","count":3}],
      "releases":[{"id":"rel1","title":"Alb","date":"1997",
                   "label-info":[{"label":{"name":"Soma"}}]}]}]}"#;

    const DC_BODY: &str = r#"{"results":[{"id":1,"title":"A - T","year":"1997",
      "label":["Other"],"style":["Techno"]}]}"#;

    fn q() -> Query {
        Query::new(Some("A"), "T")
    }

    #[tokio::test]
    async fn a_lookup_returns_the_default_source_with_no_configuration() {
        // MusicBrainz needs no key; that is why it is the default.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, NoCache);
        let (cs, errs) = svc.lookup(&q(), &Providers::default()).await;
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].source, musicbrainz::NAME);
        assert!(errs.is_empty());
    }

    #[tokio::test]
    async fn discogs_is_not_consulted_without_a_token() {
        // Opt-in means opt-in: no token, no request, not even a failed one.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, NoCache);
        let _ = svc.lookup(&q(), &Providers::default()).await;
        assert!(
            !svc.http.called().iter().any(|u| u.contains("discogs")),
            "{:?}",
            svc.http.called()
        );
    }

    #[tokio::test]
    async fn with_a_token_both_sources_answer_in_preference_order() {
        let http = FakeHttp::new()
            .route(&musicbrainz::search_url(&q(), 5), MB_BODY)
            .route(&discogs::search_url(&q()), DC_BODY);
        let svc = Service::new(http, NoCache);
        let providers = Providers {
            discogs_token: Some("tok".into()),
            ..Default::default()
        };
        let (cs, errs) = svc.lookup(&q(), &providers).await;
        assert!(errs.is_empty());
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].source, musicbrainz::NAME);
        assert_eq!(cs[1].source, discogs::NAME);
    }

    #[tokio::test]
    async fn a_second_lookup_never_leaves_the_machine() {
        // The cache-first half of the privacy contract in CLAUDE.md, asserted
        // on the request log rather than on the returned value — returning the
        // same answer twice would not prove the network was skipped.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, MemCache::default());
        let _ = svc.lookup(&q(), &Providers::default()).await;
        assert_eq!(svc.http.call_count(), 1);
        let _ = svc.lookup(&q(), &Providers::default()).await;
        assert_eq!(svc.http.call_count(), 1, "second lookup hit the network");
    }

    #[tokio::test]
    async fn a_no_match_is_cached_too() {
        // Otherwise a library of bootlegs pays the full rate-limited round trip
        // on every re-run, which is the case most likely to be re-run.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), r#"{"recordings":[]}"#);
        let svc = Service::new(http, MemCache::default());
        let (a, _) = svc.lookup(&q(), &Providers::default()).await;
        let (b, _) = svc.lookup(&q(), &Providers::default()).await;
        assert!(a.is_empty() && b.is_empty());
        assert_eq!(svc.http.call_count(), 1);
    }

    #[tokio::test]
    async fn the_cache_key_ignores_case_and_padding() {
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, MemCache::default());
        let _ = svc.lookup(&q(), &Providers::default()).await;
        let _ = svc
            .lookup(&Query::new(Some(" a "), " T "), &Providers::default())
            .await;
        assert_eq!(svc.http.call_count(), 1);
    }

    #[tokio::test]
    async fn the_two_providers_have_separate_cache_entries() {
        // Sharing a key would serve MusicBrainz's answer as Discogs'.
        let http = FakeHttp::new()
            .route(&musicbrainz::search_url(&q(), 5), MB_BODY)
            .route(&discogs::search_url(&q()), DC_BODY);
        let svc = Service::new(http, MemCache::default());
        let providers = Providers {
            discogs_token: Some("tok".into()),
            ..Default::default()
        };
        let (cs, _) = svc.lookup(&q(), &providers).await;
        assert_eq!(cs.len(), 2);
        assert_ne!(cs[0].source, cs[1].source);
    }

    #[tokio::test]
    async fn one_provider_failing_does_not_lose_the_others_answer() {
        // Discogs is unrouted, so it 404s. MusicBrainz already answered.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, NoCache);
        let providers = Providers {
            discogs_token: Some("tok".into()),
            ..Default::default()
        };
        let (cs, errs) = svc.lookup(&q(), &providers).await;
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].source, musicbrainz::NAME);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].starts_with("Discogs"), "{errs:?}");
    }

    #[tokio::test]
    async fn only_an_artist_and_a_title_are_ever_sent() {
        // The privacy contract, checked on the wire. A path or a library id
        // appearing in a URL would be a violation, not a bug.
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, NoCache);
        let _ = svc.lookup(&q(), &Providers::default()).await;
        for url in svc.http.called() {
            assert!(!url.contains("master.db"), "{url}");
            assert!(!url.contains("/Users/"), "{url}");
            assert!(!url.contains("Volumes"), "{url}");
        }
    }

    #[tokio::test]
    async fn art_is_not_fetched_for_a_wav() {
        // Rekordbox cannot store it, so the download and the write are both
        // wasted — and the user would see nothing either way.
        let svc = Service::new(FakeHttp::new(), NoCache);
        let c = Candidate {
            release_id: Some("rel1".into()),
            ..Default::default()
        };
        assert_eq!(svc.album_art(&c, "/m/a.wav").await.unwrap(), None);
        assert_eq!(svc.http.call_count(), 0);
    }

    #[tokio::test]
    async fn art_is_not_fetched_without_a_release_id() {
        // Discogs candidates have none; the archive is keyed by MBID only.
        let svc = Service::new(FakeHttp::new(), NoCache);
        assert_eq!(
            svc.album_art(&Candidate::default(), "/m/a.mp3")
                .await
                .unwrap(),
            None
        );
        assert_eq!(svc.http.call_count(), 0);
    }

    #[tokio::test]
    async fn art_is_fetched_for_a_release_that_has_it() {
        let http = FakeHttp::new().route_status(
            &cover_art::front_url("rel1"),
            200,
            &[0xFF, 0xD8, 0xFF, 0x00],
        );
        let svc = Service::new(http, NoCache);
        let c = Candidate {
            release_id: Some("rel1".into()),
            ..Default::default()
        };
        let art = svc.album_art(&c, "/m/a.mp3").await.unwrap().unwrap();
        assert_eq!(art.mime, "image/jpeg");
    }

    #[tokio::test]
    async fn the_whole_pipeline_produces_proposals_for_a_bare_track() {
        let http = FakeHttp::new().route(&musicbrainz::search_url(&q(), 5), MB_BODY);
        let svc = Service::new(http, NoCache);
        let (cs, _) = svc.lookup(&q(), &Providers::default()).await;
        let p = merge::build("t1", &Existing::default(), &cs);
        assert!(!p.no_match);
        let genre = p.proposals.iter().find(|x| x.field == "genre").unwrap();
        assert_eq!(genre.after, "House");
        assert_eq!(genre.source, "MusicBrainz");
    }
}
