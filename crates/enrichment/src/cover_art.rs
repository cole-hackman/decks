//! Cover Art Archive — album art, keyed by a MusicBrainz release id.
//!
//! Pairs with MusicBrainz by design: the archive is indexed by release MBID, so
//! a successful metadata lookup already carries everything the art lookup needs
//! and no second search happens.
//!
//! `docs/lexicon/07-health.md §Find Tags & Album Art` records the caveat that
//! **Rekordbox cannot store album art on WAV at all**. That is enforced in
//! [`supports_art`] rather than discovered by the user after a download.

use crate::http::Http;
use crate::types::{EnrichError, Result};

pub const NAME: &str = "Cover Art Archive";

const BASE: &str = "https://coverartarchive.org";

/// The front-cover URL for a release.
///
/// `-500` asks for the 500px thumbnail. Full-size scans run to several
/// megabytes and are being embedded into every matching audio file; 500px is
/// what Rekordbox displays and what other players show in a list.
pub fn front_url(release_id: &str) -> String {
    format!("{BASE}/release/{release_id}/front-500")
}

/// Can this file format hold album art in a way Rekordbox will read?
///
/// WAV is the documented exception. Downloading art for a WAV and embedding it
/// would spend the bandwidth and the write, and Rekordbox would show nothing.
pub fn supports_art(path: &str) -> bool {
    !matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("wav") | Some("wave") | None
    )
}

/// Downloaded art, with the MIME type sniffed from its own bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Art {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// The image type, from magic bytes rather than from the URL or a header.
///
/// The archive redirects to the Internet Archive, whose `Content-Type` is not
/// always specific, and an ID3 picture frame with a wrong MIME is a picture
/// players silently refuse to show.
pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Fetch the front cover for a release, if the archive has one.
///
/// `Ok(None)` means "no art for this release", which is ordinary — most
/// releases have none. Only a transport failure or an unusable payload is an
/// error.
pub async fn fetch<H: Http>(http: &H, release_id: &str) -> Result<Option<Art>> {
    let res = http
        .get(
            &front_url(release_id),
            &[("User-Agent", crate::musicbrainz::USER_AGENT)],
        )
        .await?;

    // 404 is the archive's normal "no cover for this release".
    if res.status == 404 {
        return Ok(None);
    }
    if !res.ok() {
        return Err(EnrichError::Status {
            provider: NAME.into(),
            status: res.status,
        });
    }

    // An empty or unrecognised body is treated as "no art" rather than as a
    // failure: embedding bytes we cannot identify would write a broken picture
    // frame into the user's file, which is worse than having no picture.
    let Some(mime) = sniff_mime(&res.body) else {
        return Ok(None);
    };
    Ok(Some(Art {
        mime: mime.to_string(),
        bytes: res.body,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeHttp;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00";

    #[test]
    fn the_url_is_keyed_by_release_id() {
        assert_eq!(
            front_url("rel-1"),
            "https://coverartarchive.org/release/rel-1/front-500"
        );
    }

    #[test]
    fn wav_cannot_hold_art_that_rekordbox_will_read() {
        // The manual's own caveat, enforced before the download rather than
        // discovered after it.
        assert!(!supports_art("/music/track.wav"));
        assert!(!supports_art("/music/track.WAV"));
        assert!(!supports_art("/music/track.wave"));
    }

    #[test]
    fn the_formats_that_can_are_allowed() {
        for p in ["/m/a.mp3", "/m/a.flac", "/m/a.m4a", "/m/a.MP3", "/m/a.aiff"] {
            assert!(supports_art(p), "{p}");
        }
    }

    #[test]
    fn a_file_with_no_extension_is_treated_as_unsupported() {
        // Nothing to write a picture frame into that we can be sure of.
        assert!(!supports_art("/music/track"));
    }

    #[test]
    fn mime_comes_from_the_bytes_not_the_url() {
        assert_eq!(sniff_mime(JPEG), Some("image/jpeg"));
        assert_eq!(sniff_mime(PNG), Some("image/png"));
        assert_eq!(sniff_mime(b"GIF89a...."), Some("image/gif"));
        assert_eq!(sniff_mime(b"RIFF\0\0\0\0WEBPxx"), Some("image/webp"));
    }

    #[test]
    fn an_unrecognised_payload_has_no_mime() {
        assert_eq!(sniff_mime(b"<html>404</html>"), None);
        assert_eq!(sniff_mime(b""), None);
        // Short enough to run off the end of a naive slice index.
        assert_eq!(sniff_mime(b"RIFF"), None);
    }

    #[tokio::test]
    async fn a_release_with_art_returns_it() {
        let http = FakeHttp::new().route_status(&front_url("rel-1"), 200, JPEG);
        let art = fetch(&http, "rel-1").await.unwrap().unwrap();
        assert_eq!(art.mime, "image/jpeg");
        assert_eq!(art.bytes, JPEG);
    }

    #[tokio::test]
    async fn a_release_with_no_art_is_none_not_an_error() {
        // Most releases have no cover; this is the common path, not a failure.
        let http = FakeHttp::new();
        assert_eq!(fetch(&http, "missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn an_unidentifiable_body_is_none_rather_than_a_broken_picture_frame() {
        let http = FakeHttp::new().route_status(&front_url("rel-1"), 200, b"<html>oops</html>");
        assert_eq!(fetch(&http, "rel-1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_server_error_is_an_error() {
        let http = FakeHttp::new().route_status(&front_url("rel-1"), 500, b"");
        assert!(matches!(
            fetch(&http, "rel-1").await,
            Err(EnrichError::Status { status: 500, .. })
        ));
    }
}
