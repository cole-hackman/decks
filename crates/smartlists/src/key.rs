//! Notation-aware key canonicalisation for smartlist rules.
//!
//! A rule written as `8A`, `Am`, `A minor` or `8m` must all match a track
//! whatever notation it happens to be stored in. Everything canonicalises to
//! Camelot, which is what `changes::key_format` already speaks.
//!
//! `changes::key_format::to_camelot` handles Camelot and musical spellings but
//! not Open Key, so Open Key input is translated here first.
//!
//! # A note on the Open Key mapping
//!
//! This crate follows the convention already implemented in
//! `changes::key_format::to_open_key`: the wheel number is preserved and only
//! the suffix changes (Camelot `8A` ↔ Open Key `8m`, `8B` ↔ `8d`).
//!
//! Be aware that the published Open Key standard is *rotated* relative to
//! Camelot — under it, C major is `1d` where Camelot calls it `8B`. Lexicon's
//! manual describes Open Key as "the same as Camelot but with different
//! letters", yet its own worked example (searching `4M` finds `Am`) implies the
//! rotated form. We deliberately do **not** resolve that here: `to_open_key` is
//! already used by the shipped Sync flow to write key values into `master.db`,
//! so changing the mapping would silently rewrite users' libraries. Tracked as
//! an open question in `docs/lexicon/GAPS.md`.

use changes::key_format::to_camelot;

/// Canonicalise any supported key notation to Camelot (`"8A"`, `"11B"`).
/// Returns `None` for input that cannot be parsed.
pub fn canonical_key(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(camelot) = open_key_to_camelot(trimmed) {
        return Some(camelot);
    }
    to_camelot(trimmed)
}

/// Translate Open Key (`8m`, `12d`) to Camelot. Returns `None` when the input
/// is not Open Key, so callers can fall through to the other parsers.
///
/// Requires a leading digit so musical spellings like `Am` and `Ebm` — which
/// also end in `m` — are left for `to_camelot`.
fn open_key_to_camelot(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if !bytes.first()?.is_ascii_digit() {
        return None;
    }
    let suffix = match *bytes.last()? as char {
        'm' | 'M' => 'A',
        'd' | 'D' => 'B',
        _ => return None,
    };
    let digits: String = s[..s.len() - 1]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let num: u8 = digits.parse().ok()?;
    if !(1..=12).contains(&num) {
        return None;
    }
    Some(format!("{num}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalises_camelot() {
        assert_eq!(canonical_key("8A"), Some("8A".into()));
        assert_eq!(canonical_key("11b"), Some("11B".into()));
        assert_eq!(canonical_key(" 5 B "), Some("5B".into()));
    }

    #[test]
    fn canonicalises_musical_spellings() {
        assert_eq!(canonical_key("Am"), Some("8A".into()));
        assert_eq!(canonical_key("A minor"), Some("8A".into()));
        assert_eq!(canonical_key("C"), Some("8B".into()));
        assert_eq!(canonical_key("C major"), Some("8B".into()));
    }

    #[test]
    fn canonicalises_open_key() {
        assert_eq!(canonical_key("8m"), Some("8A".into()));
        assert_eq!(canonical_key("8d"), Some("8B".into()));
        assert_eq!(canonical_key("12M"), Some("12A".into()));
        assert_eq!(canonical_key("1D"), Some("1B".into()));
    }

    #[test]
    fn open_key_parser_ignores_musical_spellings() {
        // These end in 'm' but must be handled by the musical-key parser, not
        // the Open Key one.
        assert_eq!(open_key_to_camelot("Am"), None);
        assert_eq!(open_key_to_camelot("Ebm"), None);
        assert_eq!(open_key_to_camelot("F#m"), None);
    }

    #[test]
    fn rejects_out_of_range_and_garbage() {
        assert_eq!(canonical_key("13m"), None);
        assert_eq!(canonical_key("0d"), None);
        assert_eq!(canonical_key("13A"), None);
        assert_eq!(canonical_key("Banana"), None);
        assert_eq!(canonical_key(""), None);
        assert_eq!(canonical_key("   "), None);
    }

    #[test]
    fn enharmonics_agree() {
        assert_eq!(canonical_key("F# minor"), canonical_key("Gb minor"));
        assert_eq!(canonical_key("F# major"), canonical_key("Gb major"));
    }

    #[test]
    fn all_spellings_of_a_minor_agree() {
        let expected = Some("8A".to_string());
        for s in ["8A", "8a", "8m", "8M", "Am", "A minor", "A min"] {
            assert_eq!(canonical_key(s), expected, "spelling {s}");
        }
    }
}
