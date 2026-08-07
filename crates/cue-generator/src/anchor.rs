//! Anchors — the structural landmarks a cue template places cues relative to.
//!
//! Anchors can arrive two ways:
//!
//! 1. **Detection** — analysis finds the drop, the breakdowns and the fade-out.
//! 2. **Custom cue anchors** — the user already placed cues by hand and tells
//!    the generator which is which. Detection is skipped entirely.
//!
//! (2) is implemented first deliberately. It is pure matching with no analysis
//! behind it, it delivers the whole template system on its own, and it gives us
//! human-placed ground truth to evaluate detection against later.

use serde::{Deserialize, Serialize};

/// A structural landmark in a track.
///
/// Ordinals are 1-based and match how DJs talk: "the second drop", "the first
/// breakdown". Lexicon's own settings make the same distinction — losing the
/// first breakdown means the second drop is never found either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// The start cue. Its position depends on `StartCueBehavior`.
    Start,
    Drop {
        ordinal: u8,
    },
    Breakdown {
        ordinal: u8,
    },
    /// The point to begin mixing out. Detected from low frequencies only, so it
    /// lands *before* a quiet outro rather than inside it.
    FadeOut,
    /// The literal end of the track.
    End,
}

impl Anchor {
    pub fn drop(ordinal: u8) -> Self {
        Anchor::Drop { ordinal }
    }
    pub fn breakdown(ordinal: u8) -> Self {
        Anchor::Breakdown { ordinal }
    }
}

/// How confident we are that an anchor is where we say it is.
///
/// Carried all the way to the UI rather than dropped after ranking: per
/// ADR-0008 a guess must never be presented as fact. A user-supplied anchor is
/// `Certain` because a human put it there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Placed by a human, or read from an existing cue.
    Certain,
    /// Detected, with a 0.0–1.0 score.
    Detected(f32),
}

impl Confidence {
    pub fn score(self) -> f32 {
        match self {
            Confidence::Certain => 1.0,
            Confidence::Detected(s) => s.clamp(0.0, 1.0),
        }
    }

    /// Whether this should be surfaced as provisional in the UI.
    pub fn is_provisional(self) -> bool {
        match self {
            Confidence::Certain => false,
            Confidence::Detected(s) => s < 0.6,
        }
    }
}

/// A located anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedAnchor {
    pub anchor: Anchor,
    pub position_ms: i64,
    pub confidence: Confidence,
}

/// A rule mapping an existing cue onto an anchor, by name and/or colour.
///
/// Lexicon's matching rules, which we follow exactly:
/// - name **and** colour supplied → both must match
/// - name only → first cue with that name, any colour
/// - colour only → first cue with that colour, any name
/// - neither → matches nothing (a mapping with no criteria is a mistake, not a
///   wildcard)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomAnchorRule {
    pub anchor: Anchor,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
}

/// The subset of a cue this module needs, so the crate does not depend on the
/// full Rekordbox types.
#[derive(Debug, Clone, PartialEq)]
pub struct CueRef {
    pub position_ms: i64,
    pub name: Option<String>,
    pub color: Option<i64>,
}

fn name_matches(rule: &str, cue: Option<&str>) -> bool {
    // Trimmed and case-insensitive: users do not reliably reproduce their own
    // capitalisation, and a silent non-match here looks like the feature is
    // broken rather than mis-configured.
    match cue {
        Some(c) => c.trim().eq_ignore_ascii_case(rule.trim()),
        None => false,
    }
}

/// Resolve anchors from cues the user already placed.
///
/// Cues are considered in the order given, so "the first cue with that name"
/// means the first in the supplied ordering — callers should pass them sorted
/// by position. A rule that matches nothing simply yields no anchor.
pub fn resolve_custom_anchors(cues: &[CueRef], rules: &[CustomAnchorRule]) -> Vec<ResolvedAnchor> {
    let mut out = Vec::new();
    for rule in rules {
        if rule.name.is_none() && rule.color.is_none() {
            continue;
        }
        let found = cues.iter().find(|cue| {
            let name_ok = match &rule.name {
                Some(n) => name_matches(n, cue.name.as_deref()),
                None => true,
            };
            let color_ok = match rule.color {
                Some(c) => cue.color == Some(c),
                None => true,
            };
            name_ok && color_ok
        });
        if let Some(cue) = found {
            out.push(ResolvedAnchor {
                anchor: rule.anchor,
                position_ms: cue.position_ms,
                // A human placed this cue, so there is nothing to be unsure of.
                confidence: Confidence::Certain,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(pos: i64, name: Option<&str>, color: Option<i64>) -> CueRef {
        CueRef {
            position_ms: pos,
            name: name.map(String::from),
            color,
        }
    }

    fn rule(anchor: Anchor, name: Option<&str>, color: Option<i64>) -> CustomAnchorRule {
        CustomAnchorRule {
            anchor,
            name: name.map(String::from),
            color,
        }
    }

    #[test]
    fn name_and_colour_together_require_both() {
        let cues = vec![
            cue(1000, Some("Drop"), Some(1)),
            cue(2000, Some("Drop"), Some(4)),
        ];
        let got = resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), Some("Drop"), Some(4))]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].position_ms, 2000);
    }

    #[test]
    fn name_only_takes_the_first_cue_with_that_name_regardless_of_colour() {
        let cues = vec![
            cue(1000, Some("Drop"), Some(1)),
            cue(2000, Some("Drop"), Some(4)),
        ];
        let got = resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), Some("Drop"), None)]);
        assert_eq!(got[0].position_ms, 1000);
    }

    #[test]
    fn colour_only_takes_the_first_cue_with_that_colour_regardless_of_name() {
        let cues = vec![
            cue(1000, Some("Intro"), Some(4)),
            cue(2000, Some("Drop"), Some(4)),
        ];
        let got = resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), None, Some(4))]);
        assert_eq!(got[0].position_ms, 1000);
    }

    #[test]
    fn a_rule_with_no_criteria_matches_nothing() {
        let cues = vec![cue(1000, Some("Drop"), Some(4))];
        assert!(resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), None, None)]).is_empty());
    }

    #[test]
    fn name_matching_ignores_case_and_surrounding_space() {
        let cues = vec![cue(1000, Some("  drop  "), None)];
        let got = resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), Some("DROP"), None)]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn an_unmatched_rule_yields_no_anchor_rather_than_erroring() {
        let cues = vec![cue(1000, Some("Intro"), None)];
        let got = resolve_custom_anchors(
            &cues,
            &[
                rule(Anchor::drop(1), Some("Drop"), None),
                rule(Anchor::Start, Some("Intro"), None),
            ],
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].anchor, Anchor::Start);
    }

    #[test]
    fn a_cue_with_no_name_never_matches_a_name_rule() {
        let cues = vec![cue(1000, None, Some(4))];
        assert!(
            resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), Some("Drop"), None)]).is_empty()
        );
    }

    #[test]
    fn user_supplied_anchors_are_certain() {
        let cues = vec![cue(1000, Some("Drop"), None)];
        let got = resolve_custom_anchors(&cues, &[rule(Anchor::drop(1), Some("Drop"), None)]);
        assert_eq!(got[0].confidence, Confidence::Certain);
        assert!(!got[0].confidence.is_provisional());
    }

    #[test]
    fn confidence_scores_clamp_and_flag_provisional_results() {
        assert_eq!(Confidence::Certain.score(), 1.0);
        assert_eq!(Confidence::Detected(1.5).score(), 1.0);
        assert_eq!(Confidence::Detected(-1.0).score(), 0.0);
        assert!(Confidence::Detected(0.4).is_provisional());
        assert!(!Confidence::Detected(0.9).is_provisional());
    }

    #[test]
    fn anchors_round_trip_through_json() {
        let a = Anchor::breakdown(2);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<Anchor>(&json).unwrap(), a);
    }
}
