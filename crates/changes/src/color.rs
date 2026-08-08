//! Rekordbox's track-colour palette, and mapping arbitrary colours into it.
//!
//! Per `docs/lexicon/01-interop.md §Colors → nearest`: Lexicon carries a larger
//! palette than most DJ apps, and the option decides what happens to a colour
//! the target app cannot represent. **Off means nothing is written** when there
//! is no exact match — the colour is left alone rather than approximated.
//!
//! Rekordbox's palette is fixed at eight. Unlike genres or labels, a colour is
//! not a free-text field you can extend: `djmdColor` is a lookup table of what
//! the hardware can display, and inserting a ninth row would produce a value no
//! CDJ can render. So colour resolution never creates a row — it matches an
//! existing one or declines.

/// One entry in Rekordbox's fixed eight-colour palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteColor {
    pub name: &'static str,
    pub rgb: (u8, u8, u8),
}

/// The eight colours Rekordbox offers, in its own order.
///
/// RGB values are approximations of what Rekordbox renders, good enough to rank
/// nearness. They are not used for display — the UI reads the name.
pub const PALETTE: [PaletteColor; 8] = [
    PaletteColor {
        name: "Pink",
        rgb: (255, 107, 157),
    },
    PaletteColor {
        name: "Red",
        rgb: (229, 72, 77),
    },
    PaletteColor {
        name: "Orange",
        rgb: (247, 107, 21),
    },
    PaletteColor {
        name: "Yellow",
        rgb: (245, 217, 10),
    },
    PaletteColor {
        name: "Green",
        rgb: (70, 167, 88),
    },
    PaletteColor {
        name: "Aqua",
        rgb: (18, 165, 184),
    },
    PaletteColor {
        name: "Blue",
        rgb: (62, 99, 221),
    },
    PaletteColor {
        name: "Purple",
        rgb: (142, 78, 198),
    },
];

/// The palette entry whose name matches, ignoring case and surrounding space.
pub fn by_name(name: &str) -> Option<PaletteColor> {
    let wanted = name.trim();
    PALETTE
        .iter()
        .copied()
        .find(|c| c.name.eq_ignore_ascii_case(wanted))
}

/// Parse `#rrggbb` / `rrggbb`. Returns `None` for anything else.
pub fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let component = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some((component(0)?, component(2)?, component(4)?))
}

/// The palette entry closest to `rgb`, by squared Euclidean distance.
///
/// Plain RGB distance rather than a perceptual space such as CIELAB. The
/// palette has eight widely separated hues, so the two agree on every input
/// that matters here, and a colour-science dependency to break ties between
/// "Pink" and "Red" for a track label is not a trade worth making. Documented
/// so the choice is visible rather than assumed.
pub fn nearest(rgb: (u8, u8, u8)) -> PaletteColor {
    let distance = |c: &PaletteColor| {
        let dr = c.rgb.0 as i32 - rgb.0 as i32;
        let dg = c.rgb.1 as i32 - rgb.1 as i32;
        let db = c.rgb.2 as i32 - rgb.2 as i32;
        dr * dr + dg * dg + db * db
    };
    // `min_by_key` keeps the first on a tie, so palette order breaks ties
    // deterministically — the same input always resolves to the same colour.
    *PALETTE
        .iter()
        .min_by_key(|c| distance(c))
        .expect("palette is never empty")
}

/// What a requested colour resolves to, and whether it was exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The name or hex matched a palette entry outright.
    Exact(PaletteColor),
    /// Mapped to the nearest palette entry. Only produced when the caller opted
    /// in — it changes the user's colour to one they did not choose, and doing
    /// that silently is the failure the Lexicon option exists to prevent.
    Nearest {
        requested: String,
        resolved: PaletteColor,
    },
    /// Recognisable as a colour, but no exact palette match and approximation
    /// was not permitted. Nothing should be written.
    NoExactMatch { requested: String },
    /// Not a colour we can interpret at all — not a palette name, not hex.
    Unrecognised { requested: String },
}

/// Resolve a colour the user asked for against Rekordbox's palette.
///
/// Accepts a palette name (`"Red"`) or a hex string (`"#e5484d"`). With
/// `allow_nearest` off, a hex that does not land exactly on a palette entry
/// yields [`Resolution::NoExactMatch`] and the caller must write nothing —
/// which is exactly what the spec means by "off means no colour is written
/// when there's no exact match".
pub fn resolve(requested: &str, allow_nearest: bool) -> Resolution {
    let trimmed = requested.trim();
    if let Some(exact) = by_name(trimmed) {
        return Resolution::Exact(exact);
    }
    let Some(rgb) = parse_hex(trimmed) else {
        return Resolution::Unrecognised {
            requested: trimmed.to_owned(),
        };
    };
    let candidate = nearest(rgb);
    if candidate.rgb == rgb {
        return Resolution::Exact(candidate);
    }
    if allow_nearest {
        Resolution::Nearest {
            requested: trimmed.to_owned(),
            resolved: candidate,
        }
    } else {
        Resolution::NoExactMatch {
            requested: trimmed.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_names_match_case_insensitively() {
        assert_eq!(by_name("red").map(|c| c.name), Some("Red"));
        assert_eq!(by_name("  AQUA ").map(|c| c.name), Some("Aqua"));
        assert_eq!(by_name("Chartreuse"), None);
    }

    #[test]
    fn hex_parses_with_and_without_the_hash() {
        assert_eq!(parse_hex("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex("ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex("#FF0000"), Some((255, 0, 0)));
    }

    #[test]
    fn malformed_hex_is_rejected_rather_than_guessed() {
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#gg0000"), None);
        assert_eq!(parse_hex("red"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn nearest_picks_the_obvious_neighbour() {
        assert_eq!(nearest((255, 0, 0)).name, "Red");
        assert_eq!(nearest((0, 0, 255)).name, "Blue");
        assert_eq!(nearest((0, 255, 0)).name, "Green");
        assert_eq!(nearest((250, 220, 20)).name, "Yellow");
    }

    #[test]
    fn nearest_is_deterministic_on_a_tie() {
        // Palette order breaks ties, so the same input always resolves the same
        // way. A list that reshuffles between two identical syncs is not usable.
        let midpoint = (0, 0, 0);
        assert_eq!(nearest(midpoint), nearest(midpoint));
    }

    #[test]
    fn an_exact_palette_name_resolves_without_needing_permission() {
        assert_eq!(
            resolve("Red", false),
            Resolution::Exact(by_name("Red").unwrap())
        );
    }

    #[test]
    fn an_exact_hex_is_exact_even_with_nearest_off() {
        // It lands on the palette entry itself; nothing is being approximated.
        assert_eq!(
            resolve("#e5484d", false),
            Resolution::Exact(by_name("Red").unwrap())
        );
    }

    /// The whole point of the option.
    #[test]
    fn an_inexact_colour_writes_nothing_when_nearest_is_off() {
        match resolve("#e04050", false) {
            Resolution::NoExactMatch { requested } => assert_eq!(requested, "#e04050"),
            other => panic!("expected NoExactMatch, got {other:?}"),
        }
    }

    #[test]
    fn an_inexact_colour_maps_when_nearest_is_on() {
        match resolve("#e04050", true) {
            Resolution::Nearest {
                requested,
                resolved,
            } => {
                assert_eq!(requested, "#e04050");
                assert_eq!(resolved.name, "Red");
            }
            other => panic!("expected Nearest, got {other:?}"),
        }
    }

    #[test]
    fn something_that_is_not_a_colour_is_never_approximated() {
        // Even with nearest on. "Chartreuse" is not a failed match, it is not a
        // colour we can read — mapping it to something would be invention.
        assert!(matches!(
            resolve("Chartreuse", true),
            Resolution::Unrecognised { .. }
        ));
    }
}
