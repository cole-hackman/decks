use crate::types::{BrokenMetadataReport, DuplicateGroup, DuplicateKind, Track};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};

pub fn duplicate_tracks(tracks: Vec<Track>) -> Vec<DuplicateGroup> {
    let mut groups: BTreeMap<(String, String), Vec<Track>> = BTreeMap::new();
    for track in tracks {
        let title_key = normalize(&track.title);
        let artist_key = normalize(track.artist.as_deref().unwrap_or(""));
        if title_key.is_empty() {
            continue;
        }
        groups
            .entry((title_key, artist_key))
            .or_default()
            .push(track);
    }

    groups
        .into_values()
        .filter(|tracks| tracks.len() > 1)
        .map(|tracks| DuplicateGroup {
            title: tracks[0].title.clone(),
            artist: tracks[0].artist.clone(),
            tracks,
            kind: DuplicateKind::ExactTitleArtist,
            confidence: 1.0,
        })
        .collect()
}

/// Heuristic duplicate detector. Groups tracks by a heavily-normalized signature
/// derived from title + primary artist: lowercased, punctuation stripped, common
/// parenthetical/bracket annotations removed (e.g. "(Original Mix)",
/// "(Extended Edit)"), feature markers collapsed, whitespace squeezed.
///
/// Catches typo-y duplicates that `duplicate_tracks` (which uses exact
/// normalized title+artist) misses, at the cost of occasional false positives —
/// callers should treat results as "candidates", not authoritative.
pub fn fuzzy_duplicate_tracks(tracks: Vec<Track>) -> Vec<DuplicateGroup> {
    let mut groups: BTreeMap<(String, String), Vec<Track>> = BTreeMap::new();
    for track in tracks {
        let title_key = fuzzy_signature(&track.title);
        let artist_key = fuzzy_artist(track.artist.as_deref().unwrap_or(""));
        if title_key.is_empty() {
            continue;
        }
        groups
            .entry((title_key, artist_key))
            .or_default()
            .push(track);
    }

    groups
        .into_values()
        .filter(|tracks| tracks.len() > 1)
        .map(|tracks| DuplicateGroup {
            title: tracks[0].title.clone(),
            artist: tracks[0].artist.clone(),
            tracks,
            kind: DuplicateKind::FuzzyTitle,
            confidence: 0.85,
        })
        .collect()
}

fn fuzzy_signature(value: &str) -> String {
    let lower = value.to_lowercase();
    // Drop bracketed/parenthetical annotations like "(Original Mix)" or "[Remix]".
    let mut out = String::with_capacity(lower.len());
    let mut depth = 0i32;
    for c in lower.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // Drop "feat." / "ft." remainders so collabs collapse.
    for marker in [" feat. ", " feat ", " ft. ", " ft "] {
        if let Some(idx) = out.find(marker) {
            out.truncate(idx);
        }
    }
    // Keep only alphanumerics + spaces.
    let stripped: String = out
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fuzzy_artist(value: &str) -> String {
    // Take the first artist in collaboration lists, then run the signature
    // normalizer on it.
    let primary = value
        .split([',', '&', ';'])
        .next()
        .unwrap_or(value)
        .split(" vs ")
        .next()
        .unwrap_or(value)
        .split(" x ")
        .next()
        .unwrap_or(value);
    fuzzy_signature(primary)
}

/// Maximum number of differing **bits** to still call two fingerprints a match.
/// Chromagram blobs are nominally 128 bits (16 bytes); 10 bits ≈ 92% similarity.
pub const FINGERPRINT_HAMMING_MAX_BITS: u32 = 10;

/// Bucket prefix size, in bytes, for the O(n) candidate-pairing heuristic.
/// Tradeoff: fingerprints whose first BUCKET_PREFIX_BYTES bytes differ are
/// never compared, so two recordings that diverge in their first ~32 bits
/// won't group even if the remaining 96 bits match. Acceptable for the
/// near-duplicate use case (real dupes share leading chroma frames); the
/// alternative is O(n²) pairwise comparison which doesn't scale past ~5k tracks.
const BUCKET_PREFIX_BYTES: usize = 4;

/// Only tracks between 15 seconds and 15 minutes are fingerprint-matched.
///
/// The bounds are the spec's, and the reasons are sound in both directions:
/// DJ sets and mixes above the ceiling match each other on loose chroma
/// similarity, and samples below the floor match almost anything.
///
/// A track with no recorded duration is **included**: an unknown length is not
/// evidence of a long one, and excluding it would silently drop tracks that
/// have simply never been analysed.
pub fn fingerprintable_length(duration_secs: Option<i64>) -> bool {
    match duration_secs {
        None => true,
        Some(secs) => (MIN_FINGERPRINT_SECS..=MAX_FINGERPRINT_SECS).contains(&secs),
    }
}

/// 15 seconds, per the spec.
pub const MIN_FINGERPRINT_SECS: i64 = 15;
/// 15 minutes, per the spec.
pub const MAX_FINGERPRINT_SECS: i64 = 15 * 60;

pub fn audio_fingerprint_duplicates(
    tracks: Vec<Track>,
    fingerprints: &HashMap<String, Vec<u8>>,
) -> Result<Vec<DuplicateGroup>, anyhow::Error> {
    // Filter to tracks we actually have fingerprints for, within the length
    // bounds the spec states. A chromagram of a two-hour mix matches every
    // other two-hour mix loosely enough to be noise, and a one-second stab
    // matches almost anything — both directions produce groups a user would
    // have to reject one at a time.
    let fps: Vec<(Track, &Vec<u8>)> = tracks
        .into_iter()
        .filter(|t| fingerprintable_length(t.duration_secs))
        .filter_map(|t| {
            t.folder_path
                .as_deref()
                .and_then(|path| fingerprints.get(path))
                .map(|fp| (t, fp))
        })
        .collect();

    // Bucket by the first BUCKET_PREFIX_BYTES bytes of the chromagram. Only
    // tracks that share a prefix are compared pairwise — this turns a 50k-track
    // O(n²) (2.5B compares) into a sum of small per-bucket O(k²) compares.
    let mut buckets: HashMap<Vec<u8>, Vec<usize>> = HashMap::new();
    for (idx, (_, fp)) in fps.iter().enumerate() {
        if fp.len() < BUCKET_PREFIX_BYTES {
            continue;
        }
        let key = fp[..BUCKET_PREFIX_BYTES].to_vec();
        buckets.entry(key).or_default().push(idx);
    }

    let mut groups = Vec::new();
    let mut used = vec![false; fps.len()];

    // Deterministic group order: iterate buckets in insertion order via a sort.
    let mut bucket_keys: Vec<&Vec<u8>> = buckets.keys().collect();
    bucket_keys.sort();

    for key in bucket_keys {
        let indices = &buckets[key];
        for &i in indices {
            if used[i] {
                continue;
            }
            let mut group_idxs = vec![i];
            let mut group_dists: Vec<u32> = vec![0];
            used[i] = true;
            for &j in indices {
                if j <= i || used[j] {
                    continue;
                }
                let dist = hamming_bits(fps[i].1, fps[j].1);
                if dist <= FINGERPRINT_HAMMING_MAX_BITS {
                    group_idxs.push(j);
                    group_dists.push(dist);
                    used[j] = true;
                }
            }
            if group_idxs.len() > 1 {
                let max_dist = *group_dists.iter().max().unwrap_or(&0) as f32;
                let max_bits = (fps[i].1.len() * 8) as f32;
                let confidence = if max_bits > 0.0 {
                    (1.0 - max_dist / max_bits).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let tracks: Vec<Track> = group_idxs.iter().map(|&k| fps[k].0.clone()).collect();
                groups.push(DuplicateGroup {
                    title: format!("{} (Audio Match)", tracks[0].title),
                    artist: tracks[0].artist.clone(),
                    tracks,
                    kind: DuplicateKind::AudioFingerprint,
                    confidence,
                });
            }
        }
    }

    Ok(groups)
}

/// Hamming distance in **bits** between two equal-length byte slices.
/// If lengths differ, the extra bytes count as fully-different (8 bits each).
fn hamming_bits(a: &[u8], b: &[u8]) -> u32 {
    let common = a.len().min(b.len());
    let extra = (a.len().max(b.len()) - common) as u32 * 8;
    let mut diff: u32 = 0;
    for i in 0..common {
        diff += (a[i] ^ b[i]).count_ones();
    }
    diff + extra
}

/// One-shot library-wide duplicate scan combining all three strategies.
///
/// Returns groups in this order: exact-title-artist, then fuzzy-title, then
/// audio-fingerprint. The same track may appear in multiple groups across
/// different `kind`s — callers decide whether to dedupe.
pub fn library_duplicate_groups(
    tracks: Vec<Track>,
    fingerprints: &HashMap<String, Vec<u8>>,
) -> Result<Vec<DuplicateGroup>> {
    let mut out = Vec::new();
    out.extend(duplicate_tracks(tracks.clone()));
    out.extend(fuzzy_duplicate_tracks(tracks.clone()));
    out.extend(audio_fingerprint_duplicates(tracks, fingerprints)?);
    Ok(out)
}

pub fn broken_metadata_report(tracks: Vec<Track>) -> Result<BrokenMetadataReport> {
    let mut report = BrokenMetadataReport::default();

    for track in tracks {
        if is_blank(track.artist.as_deref()) {
            report.missing_artist.push(track.clone());
        }
        if track.bpm.is_none() || track.bpm == Some(0.0) {
            report.missing_bpm.push(track.clone());
        }
        if is_blank(track.musical_key.as_deref()) {
            report.missing_key.push(track.clone());
        }
        if is_blank(track.genre.as_deref()) {
            report.missing_genre.push(track.clone());
        }
        if looks_suspicious(&track) {
            report.suspicious.push(track);
        }
    }

    Ok(report)
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_blank(value: Option<&str>) -> bool {
    value.map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn looks_suspicious(track: &Track) -> bool {
    let title = track.title.trim().to_lowercase();
    title.is_empty()
        || matches!(
            title.as_str(),
            "unknown" | "untitled" | "track" | "audio track" | "new track"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str, title: &str, artist: Option<&str>) -> Track {
        Track {
            id: id.to_owned(),
            title: title.to_owned(),
            artist: artist.map(str::to_owned),
            album: None,
            genre: Some("Techno".to_owned()),
            musical_key: Some("8A".to_owned()),
            bpm: Some(128.0),
            duration_secs: Some(300),
            rating: None,
            comment: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: None,
            dj_play_count: None,
            energy: None,
        }
    }

    #[test]
    fn duplicates_use_normalized_title_and_artist() {
        let groups = duplicate_tracks(vec![
            track("1", " Test  Track ", Some("Artist")),
            track("2", "test track", Some("artist")),
            track("3", "Other", Some("artist")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
    }

    #[test]
    fn fuzzy_duplicates_collapses_remix_and_feature_markers() {
        let groups = fuzzy_duplicate_tracks(vec![
            track("1", "Test Track (Original Mix)", Some("Artist")),
            track(
                "2",
                "test track [Extended Edit]",
                Some("Artist feat. Guest"),
            ),
            track("3", "Different Song", Some("Artist")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
    }

    #[test]
    fn fuzzy_duplicates_primary_artist_for_collabs() {
        let groups = fuzzy_duplicate_tracks(vec![
            track("1", "Anthem", Some("A & B")),
            track("2", "Anthem", Some("A, C")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
    }

    fn track_with_path(id: &str, title: &str, artist: Option<&str>, path: &str) -> Track {
        let mut t = track(id, title, artist);
        t.folder_path = Some(path.to_owned());
        t
    }

    #[test]
    fn groups_exact_title_artist() {
        let groups = duplicate_tracks(vec![
            track("1", "Strobe", Some("Deadmau5")),
            track("2", "Strobe", Some("Deadmau5")),
            track("3", "Unrelated", Some("Deadmau5")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
        assert_eq!(groups[0].kind, DuplicateKind::ExactTitleArtist);
        assert_eq!(groups[0].confidence, 1.0);
    }

    #[test]
    fn groups_fuzzy_title_above_threshold() {
        // Two titles whose fuzzy signatures collapse to the same string.
        let groups = fuzzy_duplicate_tracks(vec![
            track("1", "Strobe", Some("Deadmau5")),
            track("2", "Strobe (Original Mix)", Some("Deadmau5")),
            track("3", "Unrelated", Some("Deadmau5")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
        assert_eq!(groups[0].kind, DuplicateKind::FuzzyTitle);
    }

    #[test]
    fn groups_fingerprints_within_hamming_threshold() {
        // Two 16-byte chromagrams (128 bits) differing in 5 bits → group.
        let a = vec![0u8; 16];
        let mut b = vec![0u8; 16];
        // Flip 5 bits in `b`'s last byte (well outside the bucket-prefix bytes
        // so the bucketed pairing still finds them).
        b[15] = 0b0001_1111; // 5 ones
                             // A clearly-different fingerprint: 30 bits flipped (>10).
        let mut c = vec![0u8; 16];
        for byte in c.iter_mut().take(BUCKET_PREFIX_BYTES) {
            *byte = 0xFF; // 8 bits each in the prefix → c lands in a different bucket
        }

        let mut fps: HashMap<String, Vec<u8>> = HashMap::new();
        fps.insert("/a".to_owned(), a.clone());
        fps.insert("/b".to_owned(), b);
        fps.insert("/c".to_owned(), c);

        let groups = audio_fingerprint_duplicates(
            vec![
                track_with_path("1", "Track A", Some("X"), "/a"),
                track_with_path("2", "Track B", Some("X"), "/b"),
                track_with_path("3", "Track C", Some("X"), "/c"),
            ],
            &fps,
        )
        .unwrap();

        // Only A↔B should pair.
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
        assert_eq!(groups[0].kind, DuplicateKind::AudioFingerprint);
        // 5 bits out of 128 → ~96% similarity.
        assert!(groups[0].confidence > 0.95, "got {}", groups[0].confidence);

        // Sanity: also rejects pairs differing by >10 bits within a single bucket.
        let mut d = a.clone();
        d[15] = 0xFF; // 8 bits
        d[14] = 0xFF; // +8 bits = 16 total, > threshold of 10
        let mut fps2: HashMap<String, Vec<u8>> = HashMap::new();
        fps2.insert("/a".to_owned(), a);
        fps2.insert("/d".to_owned(), d);
        let groups2 = audio_fingerprint_duplicates(
            vec![
                track_with_path("1", "Track A", Some("X"), "/a"),
                track_with_path("2", "Track D", Some("X"), "/d"),
            ],
            &fps2,
        )
        .unwrap();
        assert!(groups2.is_empty(), "expected no group, got {:?}", groups2);
    }

    #[test]
    fn library_groups_combines_all_three_strategies() {
        let mut fps: HashMap<String, Vec<u8>> = HashMap::new();
        fps.insert("/a".to_owned(), vec![0u8; 16]);
        fps.insert("/b".to_owned(), vec![0u8; 16]);
        let tracks = vec![
            // Exact dupes.
            track_with_path("1", "Strobe", Some("Deadmau5"), "/a"),
            track_with_path("2", "Strobe", Some("Deadmau5"), "/b"),
            // Fuzzy-only dupes.
            track("3", "Anthem (Original Mix)", Some("A")),
            track("4", "Anthem", Some("A")),
        ];
        let groups = library_duplicate_groups(tracks, &fps).unwrap();
        // 1 exact + 1 fuzzy + 1 fingerprint = 3 groups.
        let kinds: Vec<DuplicateKind> = groups.iter().map(|g| g.kind).collect();
        assert!(kinds.contains(&DuplicateKind::ExactTitleArtist));
        assert!(kinds.contains(&DuplicateKind::FuzzyTitle));
        assert!(kinds.contains(&DuplicateKind::AudioFingerprint));
    }

    #[test]
    fn broken_report_finds_missing_values() {
        let mut broken = track("1", "Untitled", None);
        broken.genre = None;
        broken.musical_key = None;
        broken.bpm = None;
        let report = broken_metadata_report(vec![broken]).unwrap();
        assert_eq!(report.missing_artist.len(), 1);
        assert_eq!(report.missing_genre.len(), 1);
        assert_eq!(report.missing_key.len(), 1);
        assert_eq!(report.missing_bpm.len(), 1);
        assert_eq!(report.suspicious.len(), 1);
    }
}
