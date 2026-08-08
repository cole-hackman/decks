pub mod mixable;

pub use mixable::{
    compatible_keys, find_mixable, key_relation, keys_compatible, BpmRelation, KeyMixingMode,
    KeyRelation, MixableContext, MixableMatch, MixableOptions, NumericRule, YearRule,
};

use rekordbox_db::Track;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionScore {
    pub score: f32, // 0 to 100
    pub reasons: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CamelotKey {
    pub number: u8,     // 1 to 12
    pub is_minor: bool, // A = minor, B = major
}

impl CamelotKey {
    /// Parse anything the library might hold in the key field.
    ///
    /// Delegates to `changes::key_format::to_camelot`, which is the one place
    /// that knows the 24-key table and its enharmonics — so `C minor`, `Cm`,
    /// `5m` and `5A` all land on the same wheel position. Scoring used to carry
    /// its own Camelot-only parser, which silently gave every spelled-out key a
    /// "Missing Key Data" score.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let camelot = changes::key_format::to_camelot(s)?;
        let bytes = camelot.as_bytes();
        let is_minor = match bytes[bytes.len() - 1] {
            b'A' => true,
            b'B' => false,
            _ => return None,
        };
        let number: u8 = camelot[..camelot.len() - 1].parse().ok()?;
        if !(1..=12).contains(&number) {
            return None;
        }
        Some(Self { number, is_minor })
    }
}

pub fn score_transition(track_a: &Track, track_b: &Track) -> TransitionScore {
    let mut reasons = Vec::new();

    // Harmonic scoring
    let mut harmonic_score = 20.0; // Base score for unknown/clashing
    if let (Some(key_a_str), Some(key_b_str)) = (&track_a.musical_key, &track_b.musical_key) {
        if let (Some(key_a), Some(key_b)) =
            (CamelotKey::parse(key_a_str), CamelotKey::parse(key_b_str))
        {
            let num_diff = (key_a.number as i16 - key_b.number as i16).abs();
            let shortest_num_dist = std::cmp::min(num_diff, 12 - num_diff);
            let same_mode = key_a.is_minor == key_b.is_minor;

            if shortest_num_dist == 0 && same_mode {
                harmonic_score = 100.0;
                reasons.push("Perfect Harmonic Match".to_string());
            } else if shortest_num_dist == 0 && !same_mode {
                harmonic_score = 90.0;
                reasons.push("Relative Major/Minor".to_string());
            } else if shortest_num_dist == 1 && same_mode {
                harmonic_score = 80.0;
                reasons.push("Adjacent Key (+1/-1)".to_string());
            } else if shortest_num_dist == 1 && !same_mode {
                harmonic_score = 60.0;
                reasons.push("Diagonal Key Change".to_string());
            } else if shortest_num_dist == 2 && same_mode {
                harmonic_score = 50.0;
                reasons.push("Energy Boost (+2)".to_string());
            } else {
                reasons.push("Harmonic Clash".to_string());
            }
        } else {
            reasons.push("Unknown Key Format".to_string());
        }
    } else {
        reasons.push("Missing Key Data".to_string());
    }

    // BPM scoring
    let mut bpm_score = 10.0; // Base penalty
    if let (Some(bpm_a), Some(bpm_b)) = (track_a.bpm, track_b.bpm) {
        if bpm_a > 0.0 && bpm_b > 0.0 {
            let delta = (bpm_a - bpm_b).abs();
            let pct = delta / bpm_a * 100.0;

            if pct <= 2.0 {
                bpm_score = 100.0;
                reasons.push(format!("Perfect BPM Match ({:.1} vs {:.1})", bpm_a, bpm_b));
            } else if pct <= 4.0 {
                bpm_score = 80.0;
                reasons.push(format!("Good BPM Match ({:.1} vs {:.1})", bpm_a, bpm_b));
            } else if pct <= 8.0 {
                bpm_score = 50.0;
                reasons.push(format!("Moderate BPM Delta ({:.1} vs {:.1})", bpm_a, bpm_b));
            } else {
                reasons.push(format!("High BPM Delta ({:.1} vs {:.1})", bpm_a, bpm_b));
            }
        } else {
            reasons.push("Invalid BPM Data".to_string());
        }
    } else {
        reasons.push("Missing BPM Data".to_string());
    }

    let score = (harmonic_score * 0.6) + (bpm_score * 0.4);

    TransitionScore { score, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_track(key: Option<&str>, bpm: Option<f64>) -> Track {
        Track {
            id: "1".into(),
            title: "Test".into(),
            artist: None,
            album: None,
            genre: None,
            musical_key: key.map(|s| s.to_string()),
            bpm,
            duration_secs: None,
            rating: None,
            comment: None,
            folder_path: None,
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: None,
            release_year: None,
            dj_play_count: None,
            label: None,
            remixer: None,
            mix: None,
            color: None,
            date_added: None,
            energy: None,
        }
    }

    #[test]
    fn test_camelot_parser() {
        assert_eq!(
            CamelotKey::parse("8A"),
            Some(CamelotKey {
                number: 8,
                is_minor: true
            })
        );
        assert_eq!(
            CamelotKey::parse("11B"),
            Some(CamelotKey {
                number: 11,
                is_minor: false
            })
        );
        assert_eq!(
            CamelotKey::parse(" 12a "),
            Some(CamelotKey {
                number: 12,
                is_minor: true
            })
        );
        assert_eq!(CamelotKey::parse("13A"), None);
        assert_eq!(CamelotKey::parse("8C"), None);
    }

    #[test]
    fn test_perfect_match() {
        let t1 = mock_track(Some("8A"), Some(120.0));
        let t2 = mock_track(Some("8A"), Some(120.0));
        let score = score_transition(&t1, &t2);
        assert_eq!(score.score, 100.0);
    }

    #[test]
    fn test_relative_match() {
        let t1 = mock_track(Some("8A"), Some(120.0));
        let t2 = mock_track(Some("8B"), Some(120.0));
        let score = score_transition(&t1, &t2);
        assert_eq!(score.score, 94.0); // 90 * 0.6 + 100 * 0.4 = 54 + 40 = 94
    }

    #[test]
    fn test_adjacent_match() {
        let t1 = mock_track(Some("8A"), Some(120.0));
        let t2 = mock_track(Some("9A"), Some(120.0));
        let score = score_transition(&t1, &t2);
        assert_eq!(score.score, 88.0); // 80 * 0.6 + 100 * 0.4 = 48 + 40 = 88
    }

    #[test]
    fn test_bpm_delta() {
        let t1 = mock_track(Some("8A"), Some(120.0));
        let t2 = mock_track(Some("8A"), Some(128.0));
        let score = score_transition(&t1, &t2);
        // delta is 8 / 120 = 6.6%. Between 4% and 8%, so bpm score is 50.
        assert_eq!(score.score, 80.0); // 100 * 0.6 + 50 * 0.4 = 60 + 20 = 80
    }
}
