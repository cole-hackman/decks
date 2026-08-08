//! Turning provider answers into proposals about one track.
//!
//! Two rules do most of the work, and both come from ADR-0008 (never present a
//! guess as a fact) rather than from the Lexicon manual:
//!
//! 1. **A field the library already has is never proposed over.** Enrichment
//!    fills gaps. A user with a carefully curated Genre does not want a
//!    folksonomy vote replacing it, and "backfill" is what the manual calls
//!    this feature.
//! 2. **Every proposal names its source.** A genre from Discogs and a genre
//!    from MusicBrainz are different claims with different provenance, and the
//!    user accepting one is entitled to know which.

use crate::types::{Candidate, FieldProposal, TrackProposal};

/// What the library already knows about the track.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Existing {
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub label: Option<String>,
    pub album: Option<String>,
}

fn filled(v: &Option<String>) -> bool {
    v.as_deref().map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Build the proposal for one track from every provider's best candidate.
///
/// `candidates` is in provider-preference order — the first provider to offer a
/// field wins it. That makes the default/second-source distinction meaningful
/// rather than decorative: MusicBrainz first, Discogs filling what it left.
pub fn build(track_id: &str, existing: &Existing, candidates: &[Candidate]) -> TrackProposal {
    let mut out = TrackProposal {
        track_id: track_id.to_string(),
        no_match: candidates.is_empty(),
        ..Default::default()
    };

    let mut push = |field: &str, before: Option<String>, after: Option<String>, c: &Candidate| {
        // Rule 1: gaps only.
        if filled(&before) {
            return;
        }
        let Some(after) = after else { return };
        let after = after.trim().to_string();
        if after.is_empty() {
            return;
        }
        // Already proposed by an earlier (more preferred) provider.
        if out.proposals.iter().any(|p| p.field == field) {
            return;
        }
        out.proposals.push(FieldProposal {
            field: field.to_string(),
            before,
            after,
            source: c.source.clone(),
            confidence: c.score,
        });
    };

    for c in candidates {
        push("genre", existing.genre.clone(), c.genre.clone(), c);
        push("album", existing.album.clone(), c.album.clone(), c);
        push("label", existing.label.clone(), c.label.clone(), c);
        push(
            "release_year",
            existing.year.map(|y| y.to_string()),
            c.year.map(|y| y.to_string()),
            c,
        );
    }

    // Subgenres are unioned across providers rather than first-wins: they land
    // as custom tags, which are additive by nature, so there is no field for a
    // second provider to lose. Order follows provider preference, and the
    // main genre is never repeated as a tag.
    let mut seen: Vec<String> = Vec::new();
    for c in candidates {
        for s in &c.subgenres {
            let s = s.trim();
            if s.is_empty() {
                continue;
            }
            let is_dup = seen.iter().any(|k| k.eq_ignore_ascii_case(s));
            let is_main = out
                .proposals
                .iter()
                .any(|p| p.field == "genre" && p.after.eq_ignore_ascii_case(s));
            if !is_dup && !is_main {
                seen.push(s.to_string());
            }
        }
    }
    out.tags = seen;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(source: &str) -> Candidate {
        Candidate {
            source: source.into(),
            genre: Some("House".into()),
            subgenres: vec!["French House".into()],
            year: Some(1997),
            label: Some("Soma".into()),
            album: Some("Homework".into()),
            score: 0.9,
            ..Default::default()
        }
    }

    #[test]
    fn an_empty_library_field_gets_a_proposal() {
        let p = build("t1", &Existing::default(), &[cand("MusicBrainz")]);
        let fields: Vec<&str> = p.proposals.iter().map(|x| x.field.as_str()).collect();
        assert!(fields.contains(&"genre"));
        assert!(fields.contains(&"album"));
        assert!(fields.contains(&"label"));
        assert!(fields.contains(&"release_year"));
        assert!(!p.no_match);
    }

    #[test]
    fn a_field_the_library_already_has_is_never_proposed_over() {
        // Enrichment backfills. It does not overwrite curation.
        let existing = Existing {
            genre: Some("Deep House".into()),
            ..Default::default()
        };
        let p = build("t1", &existing, &[cand("MusicBrainz")]);
        assert!(!p.proposals.iter().any(|x| x.field == "genre"));
        // The other gaps are still filled.
        assert!(p.proposals.iter().any(|x| x.field == "label"));
    }

    #[test]
    fn a_whitespace_only_library_value_counts_as_empty() {
        // Rekordbox libraries are full of these, and treating "   " as curated
        // would make the feature do nothing on a real collection.
        let existing = Existing {
            genre: Some("   ".into()),
            ..Default::default()
        };
        let p = build("t1", &existing, &[cand("MusicBrainz")]);
        assert!(p.proposals.iter().any(|x| x.field == "genre"));
    }

    #[test]
    fn every_proposal_names_its_source() {
        // ADR-0008. A value with no provenance is a guess dressed as a fact.
        let p = build("t1", &Existing::default(), &[cand("MusicBrainz")]);
        assert!(!p.proposals.is_empty());
        assert!(p.proposals.iter().all(|x| x.source == "MusicBrainz"));
    }

    #[test]
    fn the_first_provider_to_offer_a_field_wins_it() {
        // What makes "default source" and "second source" mean something.
        let mb = Candidate {
            genre: Some("House".into()),
            label: None,
            ..cand("MusicBrainz")
        };
        let dc = Candidate {
            genre: Some("Techno".into()),
            label: Some("Soma".into()),
            ..cand("Discogs")
        };
        let p = build("t1", &Existing::default(), &[mb, dc]);
        let genre = p.proposals.iter().find(|x| x.field == "genre").unwrap();
        assert_eq!(genre.after, "House");
        assert_eq!(genre.source, "MusicBrainz");
        // And the gap MusicBrainz left is filled by the second source.
        let label = p.proposals.iter().find(|x| x.field == "label").unwrap();
        assert_eq!(label.source, "Discogs");
    }

    #[test]
    fn subgenres_union_across_providers_rather_than_first_wins() {
        // They become custom tags, which are additive — there is no single
        // field for the second provider to lose.
        let mb = Candidate {
            subgenres: vec!["French House".into()],
            ..cand("MusicBrainz")
        };
        let dc = Candidate {
            subgenres: vec!["Filter House".into(), "french house".into()],
            ..cand("Discogs")
        };
        let p = build("t1", &Existing::default(), &[mb, dc]);
        assert_eq!(p.tags, vec!["French House", "Filter House"]);
    }

    #[test]
    fn the_main_genre_is_not_also_a_tag() {
        let c = Candidate {
            genre: Some("House".into()),
            subgenres: vec!["house".into(), "Acid House".into()],
            ..cand("MusicBrainz")
        };
        let p = build("t1", &Existing::default(), &[c]);
        assert_eq!(p.tags, vec!["Acid House"]);
    }

    #[test]
    fn no_candidates_is_a_no_match_not_an_empty_success() {
        // The UI shows different text for these, and one of them means the
        // user should clean their tags first.
        let p = build("t1", &Existing::default(), &[]);
        assert!(p.no_match);
        assert!(p.proposals.is_empty());
    }

    #[test]
    fn a_fully_populated_track_yields_nothing_but_is_not_a_no_match() {
        let existing = Existing {
            genre: Some("House".into()),
            year: Some(1997),
            label: Some("Soma".into()),
            album: Some("Homework".into()),
        };
        let p = build("t1", &existing, &[cand("MusicBrainz")]);
        assert!(p.proposals.is_empty());
        assert!(
            !p.no_match,
            "a match was found; there was just nothing to fill"
        );
    }

    #[test]
    fn a_blank_provider_value_is_not_proposed() {
        let c = Candidate {
            genre: Some("  ".into()),
            ..cand("MusicBrainz")
        };
        let p = build("t1", &Existing::default(), &[c]);
        assert!(!p.proposals.iter().any(|x| x.field == "genre"));
    }

    #[test]
    fn the_year_is_proposed_as_text_the_way_the_change_pipeline_stores_it() {
        let p = build("t1", &Existing::default(), &[cand("MusicBrainz")]);
        let y = p
            .proposals
            .iter()
            .find(|x| x.field == "release_year")
            .unwrap();
        assert_eq!(y.after, "1997");
    }
}
