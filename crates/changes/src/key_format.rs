//! Musical-key → Camelot / Open Key conversion.
//!
//! Mirrors the renderer-side `apps/desktop/src/lib/camelot.ts` table so that
//! values written to `master.db` are byte-identical to what the UI displays.
//!
//! Returns `None` for unparseable input — callers fall back to writing the
//! original string rather than failing the whole sync.

/// Convert "C minor" / "F# major" / "Cm" / "8A" → canonical Camelot.
pub fn to_camelot(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Already-camelot input: "8A", "11b", " 5  B ".
    if let Some((num_part, suffix)) = split_camelot(trimmed) {
        if (1..=12).contains(&num_part) && (suffix == 'A' || suffix == 'B') {
            return Some(format!("{}{}", num_part, suffix));
        }
        return None;
    }

    // Open Key input: "10m", "5d". Unambiguous — no Camelot code ends in m or
    // d, and no note name is a number — so this can sit ahead of the musical
    // key parser without stealing anything from it.
    if let Some((num_part, suffix)) = split_open_key(trimmed) {
        if (1..=12).contains(&num_part) {
            return Some(format!("{}{}", num_part, suffix));
        }
        return None;
    }

    let (root, is_minor) = parse_musical_key(trimmed)?;
    let table = if is_minor { MINOR_TABLE } else { MAJOR_TABLE };
    table
        .iter()
        .find(|(name, _)| *name == root.as_str())
        .map(|(_, code)| (*code).to_owned())
}

/// Convert "C minor" → "5m", "C major" → "8d" (Open Key notation).
///
/// Open Key uses the same number wheel as Camelot but with `d` (= "dur" /
/// major) and `m` (= "moll" / minor) suffixes.
pub fn to_open_key(key: &str) -> Option<String> {
    let camelot = to_camelot(key)?;
    let (num, suffix) = split_camelot(&camelot)?;
    let new_suffix = match suffix {
        'A' => 'm',
        'B' => 'd',
        _ => return None,
    };
    Some(format!("{}{}", num, new_suffix))
}

/// Pad a single-digit wheel position to two digits: `1A` → `01A`, `5m` → `05m`.
///
/// This exists **purely so DJ apps sort correctly**. Sorted as text, an
/// unpadded library reads `1A, 10A, 11A, 12A, 2A, …` — the wheel positions come
/// out interleaved, which is exactly what a DJ scanning the key column cannot
/// use.
///
/// Anything that is not `<digits><letter>` is returned unchanged: a spelled-out
/// key has no wheel position to pad, and padding is not a licence to rewrite a
/// value we did not understand.
pub fn with_leading_zero(key: &str) -> String {
    let trimmed = key.trim();
    let digits = trimmed.len()
        - trimmed
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    let suffix = &trimmed[digits..];
    // One digit, and everything after it is the mode letter — `1A`, `5m`, `9d`.
    if digits == 1 && suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_alphabetic()) {
        format!("0{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn split_camelot(s: &str) -> Option<(u8, char)> {
    let bytes = s.trim().as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let last = bytes[bytes.len() - 1] as char;
    let suffix = match last {
        'a' | 'A' => 'A',
        'b' | 'B' => 'B',
        _ => return None,
    };
    let num_str: String = s[..s.len() - 1]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let num: u8 = num_str.parse().ok()?;
    Some((num, suffix))
}

/// Split Open Key input into its number and the *Camelot* letter it maps to.
///
/// `m` ("moll") is minor → `A`; `d` ("dur") is major → `B`.
fn split_open_key(s: &str) -> Option<(u8, char)> {
    let trimmed = s.trim();
    let last = trimmed.chars().last()?;
    let suffix = match last {
        'm' | 'M' => 'A',
        'd' | 'D' => 'B',
        _ => return None,
    };
    let num_str: String = trimmed[..trimmed.len() - last.len_utf8()]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let num: u8 = num_str.parse().ok()?;
    Some((num, suffix))
}

fn parse_musical_key(input: &str) -> Option<(String, bool)> {
    // Grab leading letter + optional accidental.
    let mut chars = input.chars().peekable();
    let letter = chars.next()?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    let letter_upper = letter.to_ascii_uppercase();
    if !"ABCDEFG".contains(letter_upper) {
        return None;
    }

    let mut accidental = String::new();
    if let Some(&c) = chars.peek() {
        if c == '#' || c == 'b' || c == 'B' || c == '♯' || c == '♭' {
            // Disambiguate "B" the note from "b" the flat: only consume as
            // flat when preceded by something parseable. Since we matched a
            // letter already, the next char being lowercase `b` is flat.
            let next = chars.next().unwrap();
            accidental = match next {
                '#' | '♯' => "#".to_owned(),
                'b' | 'B' | '♭' => "b".to_owned(),
                _ => String::new(),
            };
        }
    }
    let root = format!("{}{}", letter_upper, accidental);

    let tail: String = chars.collect();
    let tail = tail.trim().to_ascii_lowercase().replace('.', "");
    let is_minor = match tail.as_str() {
        "" | "maj" | "major" => false,
        "m" | "min" | "minor" => true,
        _ => return None,
    };
    Some((root, is_minor))
}

// (root, camelot) pairs — order doesn't matter; lookup is linear (<= 24 entries).
const MAJOR_TABLE: &[(&str, &str)] = &[
    ("B", "1B"),
    ("F#", "2B"),
    ("Gb", "2B"),
    ("C#", "3B"),
    ("Db", "3B"),
    ("G#", "4B"),
    ("Ab", "4B"),
    ("D#", "5B"),
    ("Eb", "5B"),
    ("A#", "6B"),
    ("Bb", "6B"),
    ("F", "7B"),
    ("C", "8B"),
    ("G", "9B"),
    ("D", "10B"),
    ("A", "11B"),
    ("E", "12B"),
];

const MINOR_TABLE: &[(&str, &str)] = &[
    ("G#", "1A"),
    ("Ab", "1A"),
    ("D#", "2A"),
    ("Eb", "2A"),
    ("A#", "3A"),
    ("Bb", "3A"),
    ("F", "4A"),
    ("C", "5A"),
    ("G", "6A"),
    ("D", "7A"),
    ("A", "8A"),
    ("E", "9A"),
    ("B", "10A"),
    ("F#", "11A"),
    ("Gb", "11A"),
    ("C#", "12A"),
    ("Db", "12A"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_c_minor_to_camelot_5a() {
        assert_eq!(to_camelot("C minor"), Some("5A".into()));
    }

    #[test]
    fn converts_d_major_to_camelot_10b() {
        assert_eq!(to_camelot("D major"), Some("10B".into()));
    }

    #[test]
    fn returns_none_for_invalid_key() {
        assert_eq!(to_camelot("Banana"), None);
        assert_eq!(to_camelot(""), None);
        assert_eq!(to_camelot("   "), None);
    }

    #[test]
    fn converts_c_minor_to_open_key_5m() {
        assert_eq!(to_open_key("C minor"), Some("5m".into()));
    }

    #[test]
    fn converts_c_major_to_open_key_8d() {
        assert_eq!(to_open_key("C major"), Some("8d".into()));
    }

    #[test]
    fn handles_short_form_cm() {
        assert_eq!(to_camelot("Cm"), Some("5A".into()));
        assert_eq!(to_camelot("C"), Some("8B".into()));
    }

    #[test]
    fn handles_enharmonic_equivalents() {
        assert_eq!(to_camelot("F# major"), Some("2B".into()));
        assert_eq!(to_camelot("Gb major"), Some("2B".into()));
        assert_eq!(to_camelot("F# minor"), Some("11A".into()));
        assert_eq!(to_camelot("Gb minor"), Some("11A".into()));
    }

    #[test]
    fn accepts_open_key_input() {
        // Lexicon presents Open Key, so a library edited there stores it.
        assert_eq!(to_camelot("5m"), Some("5A".into()));
        assert_eq!(to_camelot("8d"), Some("8B".into()));
        assert_eq!(to_camelot("12M"), Some("12A".into()));
        assert_eq!(to_camelot("13m"), None);
        assert_eq!(to_open_key("10m"), Some("10m".into()));
    }

    #[test]
    fn open_key_parsing_does_not_steal_minor_keys() {
        // "Dm" ends in m but is a note name, not Open Key.
        assert_eq!(to_camelot("Dm"), Some("7A".into()));
        assert_eq!(to_camelot("Am"), Some("8A".into()));
        // ...and "D" alone is still D major.
        assert_eq!(to_camelot("D"), Some("10B".into()));
    }

    #[test]
    fn leading_zero_pads_single_digit_wheel_positions() {
        // Sorted as text, an unpadded library reads 1A, 10A, 11A, 2A — the
        // whole reason the option exists.
        assert_eq!(with_leading_zero("1A"), "01A");
        assert_eq!(with_leading_zero("9B"), "09B");
        assert_eq!(with_leading_zero("5m"), "05m");
        assert_eq!(with_leading_zero("8d"), "08d");
    }

    #[test]
    fn leading_zero_leaves_everything_else_alone() {
        // Already two digits.
        assert_eq!(with_leading_zero("10A"), "10A");
        assert_eq!(with_leading_zero("12m"), "12m");
        // No wheel position to pad — padding is not a licence to rewrite a
        // value we did not understand.
        assert_eq!(with_leading_zero("C minor"), "C minor");
        assert_eq!(with_leading_zero("Am"), "Am");
        assert_eq!(with_leading_zero(""), "");
        assert_eq!(with_leading_zero("unknown"), "unknown");
        // Digits with no mode letter, or more than one trailing letter.
        assert_eq!(with_leading_zero("7"), "7");
        assert_eq!(with_leading_zero("1Am"), "1Am");
    }

    #[test]
    fn leading_zero_trims_but_does_not_otherwise_touch_case() {
        assert_eq!(with_leading_zero(" 3a "), "03a");
    }

    #[test]
    fn padding_is_idempotent() {
        // Sync runs more than once; the second run must not give 001A.
        let once = with_leading_zero("1A");
        assert_eq!(with_leading_zero(&once), once);
    }

    #[test]
    fn passthrough_already_camelot() {
        assert_eq!(to_camelot("8A"), Some("8A".into()));
        assert_eq!(to_camelot("11b"), Some("11B".into()));
        assert_eq!(to_camelot(" 5 B "), Some("5B".into()));
        assert_eq!(to_camelot("13A"), None);
    }
}
