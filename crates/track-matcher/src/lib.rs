//! Token-sort Levenshtein fuzzy match for external track lists.

use serde::Serialize;

pub mod csv_import;
pub mod csv_input;
pub mod normalise;
pub mod store_links;
pub mod tracklist;

#[derive(Debug, Clone)]
pub struct MatchCandidate {
    /// Local track id (`djmdContent.ID`).
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchInput {
    pub title: String,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum MatchStatus {
    Exact,
    Fuzzy,
    Unmatched,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchedTrack {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    pub input_title: String,
    pub input_artist: Option<String>,
    pub track: Option<MatchedTrack>,
    pub score: f32,
    pub status: MatchStatus,
}

const FUZZY_THRESHOLD: f32 = 0.85;

pub fn match_all(library: &[MatchCandidate], inputs: &[MatchInput]) -> Vec<MatchResult> {
    // Pre-normalise library once.
    let normalised_lib: Vec<(String, String)> = library
        .iter()
        .map(|c| {
            let combined = format!("{} {}", c.artist.as_deref().unwrap_or(""), c.title);
            (normalise::title_only(&c.title), normalise::full(&combined))
        })
        .collect();

    inputs
        .iter()
        .map(|inp| match_one(library, &normalised_lib, inp))
        .collect()
}

fn match_one(
    library: &[MatchCandidate],
    normalised_lib: &[(String, String)],
    input: &MatchInput,
) -> MatchResult {
    let combined_input = format!("{} {}", input.artist.as_deref().unwrap_or(""), input.title);
    let key_full = normalise::full(&combined_input);
    let key_title = normalise::title_only(&input.title);

    // Exact full-key match
    for (idx, (_lib_title, lib_full)) in normalised_lib.iter().enumerate() {
        if !key_full.is_empty() && *lib_full == key_full {
            let c = &library[idx];
            return MatchResult {
                input_title: input.title.clone(),
                input_artist: input.artist.clone(),
                track: Some(MatchedTrack {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    artist: c.artist.clone(),
                }),
                score: 1.0,
                status: MatchStatus::Exact,
            };
        }
    }

    // Fuzzy title-only match
    let mut best: Option<(usize, f32)> = None;
    for (idx, (lib_title, _)) in normalised_lib.iter().enumerate() {
        let s = token_sort_ratio(&key_title, lib_title);
        if best.is_none_or(|(_, bs)| s > bs) {
            best = Some((idx, s));
        }
    }

    if let Some((idx, score)) = best {
        if score >= FUZZY_THRESHOLD {
            let c = &library[idx];
            return MatchResult {
                input_title: input.title.clone(),
                input_artist: input.artist.clone(),
                track: Some(MatchedTrack {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    artist: c.artist.clone(),
                }),
                score,
                status: MatchStatus::Fuzzy,
            };
        }
    }

    MatchResult {
        input_title: input.title.clone(),
        input_artist: input.artist.clone(),
        track: None,
        score: best.map(|(_, s)| s).unwrap_or(0.0),
        status: MatchStatus::Unmatched,
    }
}

/// Token-sort ratio: split both strings into tokens, sort them, rejoin, then
/// compute Levenshtein-similarity in [0, 1].
pub fn token_sort_ratio(a: &str, b: &str) -> f32 {
    let mut ta: Vec<&str> = a.split_whitespace().collect();
    let mut tb: Vec<&str> = b.split_whitespace().collect();
    ta.sort_unstable();
    tb.sort_unstable();
    let na = ta.join(" ");
    let nb = tb.join(" ");
    levenshtein_similarity(&na, &nb)
}

fn levenshtein_similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let max_len = a.chars().count().max(b.chars().count()) as f32;
    if max_len == 0.0 {
        return 1.0;
    }
    let dist = levenshtein(a, b) as f32;
    1.0 - (dist / max_len)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev = (0..=n).collect::<Vec<usize>>();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Vec<MatchCandidate> {
        vec![
            MatchCandidate {
                id: "1".into(),
                title: "Strobe".into(),
                artist: Some("deadmau5".into()),
            },
            MatchCandidate {
                id: "2".into(),
                title: "Opus".into(),
                artist: Some("Eric Prydz".into()),
            },
            MatchCandidate {
                id: "3".into(),
                title: "Strobe (Club Edit)".into(),
                artist: Some("deadmau5".into()),
            },
        ]
    }

    #[test]
    fn exact_match_on_artist_title() {
        let inp = [MatchInput {
            title: "Opus".into(),
            artist: Some("Eric Prydz".into()),
        }];
        let res = match_all(&lib(), &inp);
        assert_eq!(res[0].status, MatchStatus::Exact);
        assert_eq!(res[0].track.as_ref().unwrap().id, "2");
    }

    #[test]
    fn fuzzy_matches_close_title() {
        let inp = [MatchInput {
            title: "Strobe (Original Mix)".into(),
            artist: None,
        }];
        let res = match_all(&lib(), &inp);
        assert!(matches!(
            res[0].status,
            MatchStatus::Exact | MatchStatus::Fuzzy
        ));
        assert!(res[0].track.is_some());
    }

    #[test]
    fn unmatched_when_far() {
        let inp = [MatchInput {
            title: "Completely Different Banger".into(),
            artist: None,
        }];
        let res = match_all(&lib(), &inp);
        assert_eq!(res[0].status, MatchStatus::Unmatched);
    }
}
