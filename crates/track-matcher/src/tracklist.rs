//! Reading a pasted or uploaded tracklist.
//!
//! Per `docs/lexicon/08-streaming.md §Track Matcher`: "Paste or upload a
//! tracklist (`.txt` / `.m3u8`, one entry per line, selectable separator such
//! as ` - `)". Aimed at wedding and event DJs working from a request list,
//! which is why the input is whatever the client actually sent — a pasted
//! email, an exported playlist, a numbered setlist off a forum.
//!
//! CSV already has its own path (`csv_input`, with a column-mapping UI). This
//! module covers the two formats that have no columns to map: plain lines, and
//! `.m3u8`.

use serde::{Deserialize, Serialize};

use crate::MatchInput;

/// How a line separates artist from title.
///
/// The spec calls the separator *selectable* rather than guessed, and the
/// reason shows up immediately in real lists: ` - ` is the common case, but a
/// list written as `Artist – Title` (en dash) or `Title by Artist` is not
/// unusual, and silently mis-splitting produces confident wrong matches rather
/// than obvious failures.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Separator {
    /// ` - `, the default.
    #[default]
    Hyphen,
    /// ` – ` (en dash), which is what a copy-paste from a web page usually
    /// carries.
    EnDash,
    /// ` — ` (em dash).
    EmDash,
    /// `Title by Artist` — reversed, and the one case where the order flips.
    By,
    /// Anything else the user types.
    Custom(String),
    /// No separator: the whole line is a title. The right answer for a list of
    /// titles with no artists, where splitting on a hyphen inside a title
    /// ("Re-Wired") would invent an artist.
    None,
}

impl Separator {
    /// The literal text to split on, and whether the artist comes second.
    fn parts(&self) -> Option<(&str, bool)> {
        match self {
            Separator::Hyphen => Some((" - ", false)),
            Separator::EnDash => Some((" – ", false)),
            Separator::EmDash => Some((" — ", false)),
            Separator::By => Some((" by ", true)),
            Separator::Custom(s) if !s.is_empty() => Some((s.as_str(), false)),
            Separator::Custom(_) | Separator::None => None,
        }
    }
}

/// Strip a leading list index: `1.`, `01)`, `3:`, `#3`.
///
/// Setlists arrive numbered far more often than not, and a leading `1.` makes
/// the artist `1. Daft Punk`, which normalisation does not remove and which
/// pushes a genuine match below the fuzzy threshold.
///
/// **A digit alone is never an index.** `99 Problems` and `1979` are titles,
/// so the number has to be followed by punctuation — or introduced by `#`,
/// which says "index" on its own and is the only form where a bare space is
/// enough of a delimiter.
fn strip_index(line: &str) -> &str {
    let t = line.trim_start();
    let hashed = t.starts_with('#');
    let t = t.strip_prefix('#').unwrap_or(t);
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    // More than three digits is a year or a title, not a position in a list.
    if digits == 0 || digits > 3 {
        return line.trim();
    }
    let rest = &t[digits..];
    let stripped = rest
        .strip_prefix('.')
        .or_else(|| rest.strip_prefix(')'))
        .or_else(|| rest.strip_prefix(':'))
        // `#3 Artist - Title`: the hash already marked this as an index, so a
        // space is delimiter enough. Without the hash it is not — that is what
        // keeps "99 Problems" whole.
        .or_else(|| (hashed && rest.starts_with(' ')).then_some(rest));
    match stripped {
        Some(r) if !r.trim().is_empty() => r.trim(),
        _ => line.trim(),
    }
}

/// Should this line be read as a track at all?
///
/// `.m3u8` is a superset of plain text: it is line-oriented with `#`-prefixed
/// directives, so one reader handles both. `#EXTINF:` is the only directive
/// carrying a title, and it is handled before this is called.
///
/// **`#` is ambiguous between the two formats** — it opens a directive in an
/// `.m3u8` and a list index in a hand-written setlist (`#3 Daft Punk - ...`).
/// What follows it settles the question: directives are always `#` plus
/// letters, indices are always `#` plus a digit. Treating every `#` line as a
/// directive silently swallowed hash-numbered entries.
fn is_noise(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    match t.strip_prefix('#') {
        // `#3` is an index, not a directive.
        Some(rest) => !rest.starts_with(|c: char| c.is_ascii_digit()),
        None => false,
    }
}

/// The `Artist - Title` text out of an `#EXTINF` line.
///
/// The format is `#EXTINF:<seconds>,<title>` — everything after the *first*
/// comma, because titles contain commas and the duration never does.
fn extinf_title(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("#EXTINF:")?;
    let (_, title) = rest.split_once(',')?;
    let title = title.trim();
    (!title.is_empty()).then_some(title)
}

/// Parse a pasted or uploaded tracklist into match inputs.
///
/// Handles plain `.txt` and `.m3u8` with one reader, because an `.m3u8` is a
/// text file whose non-track lines start with `#`. For `.m3u8` the `#EXTINF`
/// title is preferred over the URL line beneath it: the URL is a file path,
/// and matching a library by path is what the Relocate flow is for.
pub fn parse(input: &str, separator: &Separator) -> Vec<MatchInput> {
    let mut out = Vec::new();
    let mut pending_extinf: Option<String> = None;

    for line in input.lines() {
        if let Some(t) = extinf_title(line) {
            pending_extinf = Some(t.to_string());
            continue;
        }
        if is_noise(line) {
            continue;
        }
        // An `#EXTINF` names the track; the line beneath it is the location and
        // is skipped, since it carries no better information than the title we
        // already have.
        let text = match pending_extinf.take() {
            Some(t) => t,
            None => line.to_string(),
        };
        if let Some(mi) = line_to_input(&text, separator) {
            out.push(mi);
        }
    }

    // A trailing `#EXTINF` with no location line beneath it still names a
    // track, and dropping it would silently lose the last entry of a truncated
    // file.
    if let Some(t) = pending_extinf {
        if let Some(mi) = line_to_input(&t, separator) {
            out.push(mi);
        }
    }
    out
}

fn line_to_input(text: &str, separator: &Separator) -> Option<MatchInput> {
    let text = strip_index(text);
    if text.is_empty() {
        return None;
    }
    let Some((sep, reversed)) = separator.parts() else {
        return Some(MatchInput {
            title: text.to_string(),
            artist: None,
        });
    };

    // Only the first occurrence splits. A later one usually separates the title
    // from a version suffix ("A - B (C - D Remix)"), and splitting there would
    // put half the title in the artist field.
    match text.split_once(sep) {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => {
            let (artist, title) = if reversed { (b, a) } else { (a, b) };
            Some(MatchInput {
                title: title.trim().to_string(),
                artist: Some(artist.trim().to_string()),
            })
        }
        // The separator was not found, or one side was empty. Treating the
        // whole line as a title beats inventing an empty artist — the matcher
        // handles a title-only input, and a blank artist would drag the score
        // down on every candidate.
        _ => Some(MatchInput {
            title: text.to_string(),
            artist: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(v: &[MatchInput]) -> Vec<(&str, Option<&str>)> {
        v.iter()
            .map(|m| (m.title.as_str(), m.artist.as_deref()))
            .collect()
    }

    #[test]
    fn a_plain_list_splits_on_the_default_separator() {
        let got = parse(
            "Daft Punk - Around the World\nAphex Twin - Windowlicker",
            &Separator::Hyphen,
        );
        assert_eq!(
            titles(&got),
            vec![
                ("Around the World", Some("Daft Punk")),
                ("Windowlicker", Some("Aphex Twin")),
            ]
        );
    }

    #[test]
    fn blank_lines_are_skipped() {
        let got = parse("A - B\n\n\n C - D \n", &Separator::Hyphen);
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn a_numbered_setlist_does_not_put_the_number_in_the_artist() {
        // Request lists arrive numbered far more often than not, and "1. Daft
        // Punk" is not an artist normalisation will fix.
        let got = parse(
            "1. Daft Punk - Around the World\n02) A - B\n#3 C - D",
            &Separator::Hyphen,
        );
        assert_eq!(
            titles(&got),
            vec![
                ("Around the World", Some("Daft Punk")),
                ("B", Some("A")),
                ("D", Some("C")),
            ]
        );
    }

    #[test]
    fn a_title_that_begins_with_a_number_keeps_it() {
        // "1979" and "99 Problems" are titles, not list indices.
        let got = parse("The Smashing Pumpkins - 1979", &Separator::Hyphen);
        assert_eq!(titles(&got), vec![("1979", Some("The Smashing Pumpkins"))]);
        let got = parse("1979", &Separator::None);
        assert_eq!(titles(&got), vec![("1979", None)]);
    }

    #[test]
    fn a_bare_number_never_counts_as_an_index() {
        // "99 Problems" is a title. Only punctuation after the digits, or a
        // leading `#`, makes a number a list position.
        let got = parse("Jay-Z - 99 Problems", &Separator::Hyphen);
        assert_eq!(titles(&got), vec![("99 Problems", Some("Jay-Z"))]);
        let got = parse("99 Problems", &Separator::None);
        assert_eq!(titles(&got), vec![("99 Problems", None)]);
    }

    #[test]
    fn a_four_digit_index_is_not_an_index() {
        // Guards the strip against eating a year-like title.
        let got = parse("2001. A Space Odyssey", &Separator::None);
        assert_eq!(titles(&got), vec![("2001. A Space Odyssey", None)]);
    }

    #[test]
    fn only_the_first_separator_splits() {
        // A later one usually separates title from version, and splitting there
        // would put half the title in the artist field.
        let got = parse("A - B - C Remix", &Separator::Hyphen);
        assert_eq!(titles(&got), vec![("B - C Remix", Some("A"))]);
    }

    #[test]
    fn an_en_dash_list_needs_the_en_dash_separator() {
        // What a copy-paste off a web page usually carries. With the default
        // separator the line has no split, and falls back to a title.
        let line = "Daft Punk – Around the World";
        assert_eq!(titles(&parse(line, &Separator::Hyphen)), vec![(line, None)]);
        assert_eq!(
            titles(&parse(line, &Separator::EnDash)),
            vec![("Around the World", Some("Daft Punk"))]
        );
    }

    #[test]
    fn the_by_separator_reverses_the_order() {
        // "Title by Artist" is the one form where the sides swap.
        let got = parse("Around the World by Daft Punk", &Separator::By);
        assert_eq!(titles(&got), vec![("Around the World", Some("Daft Punk"))]);
    }

    #[test]
    fn a_custom_separator_is_honoured() {
        let got = parse(
            "Daft Punk :: Around the World",
            &Separator::Custom(" :: ".into()),
        );
        assert_eq!(titles(&got), vec![("Around the World", Some("Daft Punk"))]);
    }

    #[test]
    fn an_empty_custom_separator_behaves_as_none_rather_than_splitting_everywhere() {
        let got = parse("A - B", &Separator::Custom(String::new()));
        assert_eq!(titles(&got), vec![("A - B", None)]);
    }

    #[test]
    fn the_none_separator_keeps_hyphenated_titles_whole() {
        // A list of titles with no artists: splitting "Re-Wired" would invent
        // an artist and lose the match.
        let got = parse("Re-Wired\nWindowlicker", &Separator::None);
        assert_eq!(
            titles(&got),
            vec![("Re-Wired", None), ("Windowlicker", None)]
        );
    }

    #[test]
    fn a_line_with_an_empty_side_is_read_as_a_title() {
        // An empty artist would drag the score down against every candidate.
        let got = parse(" - Around the World", &Separator::Hyphen);
        assert_eq!(titles(&got), vec![("- Around the World", None)]);
    }

    #[test]
    fn an_m3u8_uses_its_extinf_titles_and_ignores_the_paths() {
        // The path is a location; matching a library by path is what Relocate
        // is for. The title is the information worth matching on.
        let m3u = "#EXTM3U\n\
                   #EXTINF:222,Daft Punk - Around the World\n\
                   /Users/dj/Music/track1.mp3\n\
                   #EXTINF:333,Aphex Twin - Windowlicker\n\
                   D:\\Music\\track2.mp3\n";
        assert_eq!(
            titles(&parse(m3u, &Separator::Hyphen)),
            vec![
                ("Around the World", Some("Daft Punk")),
                ("Windowlicker", Some("Aphex Twin")),
            ]
        );
    }

    #[test]
    fn an_extinf_title_containing_a_comma_survives() {
        // Split on the *first* comma only — the duration never contains one,
        // and titles frequently do.
        let m3u = "#EXTINF:222,Tyler, The Creator - Earfquake\n/m/a.mp3";
        assert_eq!(
            titles(&parse(m3u, &Separator::Hyphen)),
            vec![("Earfquake", Some("Tyler, The Creator"))]
        );
    }

    #[test]
    fn a_truncated_m3u8_does_not_lose_its_last_entry() {
        let m3u = "#EXTM3U\n#EXTINF:222,A - B\n/m/a.mp3\n#EXTINF:333,C - D";
        assert_eq!(titles(&parse(m3u, &Separator::Hyphen)).len(), 2);
    }

    #[test]
    fn an_m3u_with_no_extinf_lines_falls_back_to_its_paths() {
        // A bare `.m3u` is just a list of locations. Reading them as text is
        // better than reading nothing, and the matcher normalises filenames
        // reasonably well.
        let m3u = "#EXTM3U\nDaft Punk - Around the World.mp3";
        assert_eq!(
            titles(&parse(m3u, &Separator::Hyphen)),
            vec![("Around the World.mp3", Some("Daft Punk"))]
        );
    }

    #[test]
    fn a_hash_numbered_entry_is_an_index_not_a_directive() {
        // `#` opens a directive in an .m3u8 and a list index in a hand-written
        // setlist. What follows it settles which: letters mean directive,
        // digits mean index. Treating every `#` line as a directive silently
        // swallowed these.
        let got = parse("#EXTM3U\n#3 C - D\n# a comment", &Separator::Hyphen);
        assert_eq!(titles(&got), vec![("D", Some("C"))]);
    }

    #[test]
    fn directive_lines_never_become_tracks() {
        let m3u = "#EXTM3U\n#PLAYLIST:My Set\n#EXTVLCOPT:whatever\nA - B";
        assert_eq!(
            titles(&parse(m3u, &Separator::Hyphen)),
            vec![("B", Some("A"))]
        );
    }

    #[test]
    fn an_empty_input_yields_nothing_rather_than_one_blank_entry() {
        assert!(parse("", &Separator::Hyphen).is_empty());
        assert!(parse("\n\n  \n", &Separator::Hyphen).is_empty());
    }

    #[test]
    fn the_default_separator_is_the_one_the_spec_names() {
        assert_eq!(Separator::default(), Separator::Hyphen);
    }
}
