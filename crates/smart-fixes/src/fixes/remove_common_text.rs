use crate::{FixConfig, FixProposal, TrackView};

pub fn propose(track: &TrackView, config: &FixConfig) -> Vec<FixProposal> {
    if config.common_text_patterns.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    super::for_each_text_field(
        track,
        &["Title", "Artist", "Album", "Commnt"],
        |field, val| {
            // Case-insensitive removal goes through `recipes::text` rather
            // than a local lower-case-and-splice: lower-casing can change a
            // string's byte length (`İ` → `i̇`), so an index found in the
            // lower-cased copy is not an index into the original, and splicing
            // with the *pattern's* byte length lands mid-character. That is a
            // panic, not a wrong answer.
            let mut new = val.to_string();
            for pat in &config.common_text_patterns {
                new = recipes::text::remove_text(&new, pat, true);
            }
            while new.contains("  ") {
                new = new.replace("  ", " ");
            }
            let trimmed = new.trim().to_string();
            if trimmed != val && !trimmed.is_empty() {
                out.push(FixProposal::new(
                    "remove_common_text",
                    track,
                    field,
                    val,
                    &trimmed,
                ));
            }
        },
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tv(t: &str) -> TrackView {
        TrackView {
            id: "t".into(),
            title: Some(t.into()),
            artist: None,
            album: None,
            comment: None,
        }
    }

    #[test]
    fn strips_official_audio_tag() {
        let p = propose(&tv("Song (Official Audio)"), &FixConfig::with_defaults());
        assert_eq!(p[0].new_value, "Song");
    }

    #[test]
    fn case_insensitive() {
        let p = propose(&tv("Song hd"), &FixConfig::with_defaults());
        assert_eq!(p[0].new_value, "Song");
    }

    #[test]
    fn a_pattern_whose_case_changes_length_does_not_panic() {
        // Lower-casing `İ` yields two code points, so an index taken from the
        // lower-cased copy is not an index into the original.
        let config = FixConfig {
            common_text_patterns: vec!["İ".to_string()],
            ..Default::default()
        };
        let got = propose(&tv("Aİa Remix"), &config);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].new_value, "Aa Remix");
    }

    #[test]
    fn removal_ignores_case() {
        let config = FixConfig {
            common_text_patterns: vec!["(original mix)".to_string()],
            ..Default::default()
        };
        let got = propose(&tv("Track (Original Mix)"), &config);
        assert_eq!(got[0].new_value, "Track");
    }
}
