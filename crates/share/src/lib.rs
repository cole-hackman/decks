//! Share / export — turn a playlist into a file or a clipboard string.
//!
//! Per `docs/lexicon/08-streaming.md §Share / export`. The spec is explicit
//! that this is **not** Sync: sharing produces a file, syncing updates a DJ
//! app. Nothing here touches a database.
//!
//! Five outputs: Quick Copy, Quick Copy with numbers, CSV, M3U, and
//! printer-friendly HTML. The spec's "PDF" is the browser's Save to PDF over
//! that HTML, which is also how Lexicon does it — there is no PDF writer here
//! and there should not be one.
//!
//! **The export mirrors the view**: the caller passes the columns it is
//! showing, in the order it is showing them, and gets exactly those back.

use rekordbox_db::Track;
use serde::{Deserialize, Serialize};

/// A column the exporter knows how to render.
///
/// Deliberately a closed set rather than a free string: an export naming a
/// column that does not exist should fail to parse, not silently produce an
/// empty column that looks like missing data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Title,
    Artist,
    Album,
    Genre,
    Key,
    Bpm,
    Duration,
    Rating,
    Year,
    Comment,
    Bitrate,
    PlayCount,
    Energy,
    Path,
}

impl Column {
    pub fn header(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Genre => "Genre",
            Self::Key => "Key",
            Self::Bpm => "BPM",
            Self::Duration => "Duration",
            Self::Rating => "Rating",
            Self::Year => "Year",
            Self::Comment => "Comment",
            Self::Bitrate => "Bitrate",
            Self::PlayCount => "Plays",
            Self::Energy => "Energy",
            Self::Path => "Path",
        }
    }

    /// The cell value, already stringified. Empty for a field the track does
    /// not carry — an export is a table, and a table needs a cell.
    pub fn value(self, track: &Track) -> String {
        fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
            v.map(|v| v.to_string()).unwrap_or_default()
        }
        match self {
            Self::Title => track.title.clone(),
            Self::Artist => opt(track.artist.as_ref()),
            Self::Album => opt(track.album.as_ref()),
            Self::Genre => opt(track.genre.as_ref()),
            Self::Key => opt(track.musical_key.as_ref()),
            // One decimal: 128 and 128.03 are different tempos, 128.0300003 is
            // not a different fact about the track.
            Self::Bpm => track.bpm.map(|b| format!("{b:.1}")).unwrap_or_default(),
            Self::Duration => track.duration_secs.map(format_duration).unwrap_or_default(),
            Self::Rating => opt(track.rating),
            Self::Year => opt(track.release_year),
            Self::Comment => opt(track.comment.as_ref()),
            Self::Bitrate => opt(track.bit_rate),
            Self::PlayCount => opt(track.dj_play_count),
            Self::Energy => track.energy.map(|e| format!("{e:.0}")).unwrap_or_default(),
            Self::Path => opt(track.folder_path.as_ref()),
        }
    }
}

/// `m:ss`, which is how a DJ reads a track length. Hours are spelled out
/// rather than wrapping, because a 90-minute live set should not read `30:00`.
pub fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return String::new();
    }
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// The default column set — the one the `dj-setlist-builder` skill expects.
pub fn default_columns() -> Vec<Column> {
    vec![
        Column::Title,
        Column::Artist,
        Column::Bpm,
        Column::Key,
        Column::Duration,
    ]
}

// ── Quick copy ───────────────────────────────────────────────────────────────

/// `Artist - Title` per line, optionally numbered.
///
/// A track with no artist renders as the bare title rather than `- Title`: a
/// leading dash reads as a missing field the reader is meant to notice, and
/// what is actually missing is nothing they can act on.
pub fn quick_copy(tracks: &[Track], numbered: bool) -> String {
    tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let line = match t.artist.as_deref().filter(|a| !a.trim().is_empty()) {
                Some(artist) => format!("{artist} - {}", t.title),
                None => t.title.clone(),
            };
            if numbered {
                format!("{}. {line}", i + 1)
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── CSV ──────────────────────────────────────────────────────────────────────

/// Quote a field iff it needs it, per RFC 4180.
///
/// A leading `=`, `+`, `-` or `@` is also quoted **and prefixed with a single
/// quote**: spreadsheets treat those as formulas, and a comment field reading
/// `=cmd|...` is a live CSV-injection payload the moment someone opens the
/// export in Excel. The prefix is visible, which is the point — silently
/// mangling the value would be worse.
fn csv_field(value: &str) -> String {
    let injection = value
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@'));
    let body = if injection {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if injection || body.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", body.replace('"', "\"\""))
    } else {
        body
    }
}

/// CSV with a header row: exactly the columns given, in the order given.
pub fn csv(tracks: &[Track], columns: &[Column]) -> String {
    let mut out = columns
        .iter()
        .map(|c| csv_field(c.header()))
        .collect::<Vec<_>>()
        .join(",");
    for track in tracks {
        out.push('\n');
        out.push_str(
            &columns
                .iter()
                .map(|c| csv_field(&c.value(track)))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

// ── M3U ──────────────────────────────────────────────────────────────────────

/// What an M3U export could not include.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct M3uExport {
    pub content: String,
    /// Titles of tracks with no file path. An M3U is a list of paths, so a
    /// track without one cannot be in it — and the caller has to be able to
    /// say so rather than hand over a quietly short playlist.
    pub skipped: Vec<String>,
}

/// Extended M3U: `#EXTINF:<seconds>,<artist> - <title>` then the path.
pub fn m3u(tracks: &[Track]) -> M3uExport {
    let mut content = String::from("#EXTM3U\n");
    let mut skipped = Vec::new();
    for track in tracks {
        let Some(path) = track
            .folder_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
        else {
            skipped.push(track.title.clone());
            continue;
        };
        // -1 is the M3U convention for "unknown length".
        let secs = track.duration_secs.unwrap_or(-1);
        let label = match track.artist.as_deref().filter(|a| !a.trim().is_empty()) {
            Some(artist) => format!("{artist} - {}", track.title),
            None => track.title.clone(),
        };
        content.push_str(&format!("#EXTINF:{secs},{label}\n{path}\n"));
    }
    M3uExport { content, skipped }
}

// ── HTML ─────────────────────────────────────────────────────────────────────

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Printer-friendly HTML. The spec's PDF is the browser's Save to PDF over
/// this, which is how Lexicon does it too — a PDF writer here would be a large
/// dependency to reimplement a print dialog.
///
/// Self-contained: the CSS is inline and there are no external references, so
/// the file works from a USB stick with no network.
pub fn html(tracks: &[Track], columns: &[Column], title: &str) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str(&format!("<title>{}</title>\n", escape_html(title)));
    out.push_str(
        "<style>\n\
         body{font:13px/1.4 system-ui,sans-serif;margin:2em;color:#111}\n\
         h1{font-size:1.3em;margin:0 0 .2em}\n\
         p.meta{color:#666;margin:0 0 1em}\n\
         table{border-collapse:collapse;width:100%}\n\
         th,td{text-align:left;padding:.3em .6em;border-bottom:1px solid #ddd}\n\
         th{border-bottom:2px solid #999}\n\
         td.num{text-align:right;font-variant-numeric:tabular-nums}\n\
         @media print{body{margin:0}tr{break-inside:avoid}}\n\
         </style>\n</head>\n<body>\n",
    );
    out.push_str(&format!("<h1>{}</h1>\n", escape_html(title)));
    out.push_str(&format!(
        "<p class=\"meta\">{} track{}</p>\n",
        tracks.len(),
        if tracks.len() == 1 { "" } else { "s" }
    ));
    out.push_str("<table>\n<thead>\n<tr><th>#</th>");
    for c in columns {
        out.push_str(&format!("<th>{}</th>", escape_html(c.header())));
    }
    out.push_str("</tr>\n</thead>\n<tbody>\n");
    for (i, track) in tracks.iter().enumerate() {
        out.push_str(&format!("<tr><td class=\"num\">{}</td>", i + 1));
        for c in columns {
            out.push_str(&format!("<td>{}</td>", escape_html(&c.value(track))));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: Option<&str>) -> Track {
        Track {
            id: title.into(),
            title: title.into(),
            artist: artist.map(str::to_string),
            album: None,
            genre: None,
            musical_key: Some("8A".into()),
            bpm: Some(128.0),
            duration_secs: Some(365),
            rating: Some(4),
            comment: None,
            folder_path: Some(format!("/music/{title}.mp3")),
            analysis_data_path: None,
            file_type: None,
            sample_rate: None,
            bit_rate: Some(320),
            release_year: Some(2024),
            dj_play_count: Some(3),
            energy: Some(7.0),
        }
    }

    // ── Duration ─────────────────────────────────────────────────────────

    #[test]
    fn durations_read_the_way_a_dj_reads_them() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(59), "0:59");
        assert_eq!(format_duration(365), "6:05");
        // A 90-minute live set must not read as 30:00.
        assert_eq!(format_duration(5400), "1:30:00");
        assert_eq!(format_duration(-1), "");
    }

    // ── Quick copy ───────────────────────────────────────────────────────

    #[test]
    fn quick_copy_numbers_on_request() {
        let tracks = vec![track("One", Some("A")), track("Two", Some("B"))];
        assert_eq!(quick_copy(&tracks, false), "A - One\nB - Two");
        assert_eq!(quick_copy(&tracks, true), "1. A - One\n2. B - Two");
    }

    #[test]
    fn a_missing_artist_does_not_leave_a_dangling_dash() {
        // "- Title" reads as a field the reader is meant to notice.
        let tracks = vec![track("Bootleg", None), track("Blank", Some("  "))];
        assert_eq!(quick_copy(&tracks, false), "Bootleg\nBlank");
    }

    #[test]
    fn quick_copy_of_nothing_is_empty() {
        assert_eq!(quick_copy(&[], true), "");
    }

    // ── CSV ──────────────────────────────────────────────────────────────

    #[test]
    fn csv_exports_exactly_the_columns_given_in_order() {
        let tracks = vec![track("One", Some("A"))];
        let out = csv(&tracks, &[Column::Bpm, Column::Title]);
        assert_eq!(out, "BPM,Title\n128.0,One");
    }

    #[test]
    fn csv_quotes_only_what_needs_quoting() {
        let mut t = track("Plain", Some("A"));
        t.comment = Some("has, comma".into());
        let out = csv(&[t], &[Column::Title, Column::Comment]);
        assert_eq!(out, "Title,Comment\nPlain,\"has, comma\"");
    }

    #[test]
    fn csv_doubles_embedded_quotes() {
        let mut t = track("Plain", Some("A"));
        t.comment = Some("she said \"go\"".into());
        let out = csv(&[t], &[Column::Comment]);
        assert_eq!(out, "Comment\n\"she said \"\"go\"\"\"");
    }

    #[test]
    fn csv_quotes_fields_containing_newlines() {
        let mut t = track("Plain", Some("A"));
        t.comment = Some("line one\nline two".into());
        let out = csv(&[t], &[Column::Comment]);
        assert_eq!(out, "Comment\n\"line one\nline two\"");
    }

    #[test]
    fn csv_defuses_spreadsheet_formula_injection() {
        // A comment field is free text a DJ pasted from somewhere. Opened in
        // Excel, a leading = makes it executable.
        let mut t = track("Plain", Some("A"));
        t.comment = Some("=1+1".into());
        assert_eq!(csv(&[t], &[Column::Comment]), "Comment\n\"'=1+1\"");

        for prefix in ['+', '-', '@'] {
            let mut t = track("Plain", Some("A"));
            t.comment = Some(format!("{prefix}danger"));
            assert!(
                csv(&[t], &[Column::Comment]).contains(&format!("\"'{prefix}danger\"")),
                "{prefix} should be defused"
            );
        }
    }

    #[test]
    fn csv_of_no_tracks_is_still_a_header() {
        // A file with no header is not a CSV anyone can load.
        assert_eq!(csv(&[], &[Column::Title, Column::Bpm]), "Title,BPM");
    }

    #[test]
    fn missing_values_export_as_empty_cells() {
        let mut t = track("Sparse", None);
        t.bpm = None;
        t.musical_key = None;
        t.duration_secs = None;
        let out = csv(
            &[t],
            &[Column::Artist, Column::Bpm, Column::Key, Column::Duration],
        );
        assert_eq!(out, "Artist,BPM,Key,Duration\n,,,");
    }

    #[test]
    fn the_default_columns_are_what_the_setlist_skill_wants() {
        // The dj-setlist-builder skill reads title/artist/BPM/key.
        let cols = default_columns();
        assert!(cols.contains(&Column::Title));
        assert!(cols.contains(&Column::Artist));
        assert!(cols.contains(&Column::Bpm));
        assert!(cols.contains(&Column::Key));
    }

    // ── M3U ──────────────────────────────────────────────────────────────

    #[test]
    fn m3u_carries_extended_info() {
        let out = m3u(&[track("One", Some("A"))]);
        assert_eq!(
            out.content,
            "#EXTM3U\n#EXTINF:365,A - One\n/music/One.mp3\n"
        );
        assert!(out.skipped.is_empty());
    }

    #[test]
    fn m3u_reports_tracks_it_could_not_include() {
        // An M3U is a list of paths. A streaming track has none, and handing
        // back a quietly short playlist is how a set goes missing on the night.
        let mut pathless = track("Streaming", Some("A"));
        pathless.folder_path = None;
        let mut blank = track("Blank", Some("B"));
        blank.folder_path = Some("   ".into());

        let out = m3u(&[track("Real", Some("R")), pathless, blank]);
        assert_eq!(out.skipped, vec!["Streaming", "Blank"]);
        assert_eq!(
            out.content.lines().filter(|l| l.starts_with('/')).count(),
            1
        );
    }

    #[test]
    fn an_unknown_duration_uses_the_m3u_convention() {
        let mut t = track("One", Some("A"));
        t.duration_secs = None;
        assert!(m3u(&[t]).content.contains("#EXTINF:-1,A - One"));
    }

    // ── HTML ─────────────────────────────────────────────────────────────

    #[test]
    fn html_escapes_track_metadata() {
        let mut t = track("Rock & <Roll>", Some("A \"DJ\""));
        t.comment = Some("<script>alert(1)</script>".into());
        let out = html(
            &[t],
            &[Column::Title, Column::Artist, Column::Comment],
            "Set",
        );
        assert!(out.contains("Rock &amp; &lt;Roll&gt;"));
        assert!(out.contains("A &quot;DJ&quot;"));
        assert!(!out.contains("<script>"));
        assert!(out.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_escapes_the_title_too() {
        let out = html(&[], &[Column::Title], "Friday <b>Night</b>");
        assert!(out.contains("Friday &lt;b&gt;Night&lt;/b&gt;"));
        assert!(!out.contains("<b>Night</b>"));
    }

    #[test]
    fn html_is_self_contained() {
        // It has to work off a USB stick with no network.
        let out = html(&[track("One", Some("A"))], &default_columns(), "Set");
        assert!(!out.contains("http://"));
        assert!(!out.contains("https://"));
        assert!(!out.contains("<link"));
        assert!(!out.contains("<script"));
    }

    #[test]
    fn html_numbers_the_rows_and_counts_them() {
        let out = html(
            &[track("One", Some("A")), track("Two", Some("B"))],
            &[Column::Title],
            "Set",
        );
        assert!(out.contains("2 tracks"));
        assert!(out.contains("<td class=\"num\">1</td>"));
        assert!(out.contains("<td class=\"num\">2</td>"));

        // And gets the singular right, because "1 tracks" is the tell that
        // nobody read the output.
        let one = html(&[track("One", Some("A"))], &[Column::Title], "Set");
        assert!(one.contains("1 track<"));
    }

    #[test]
    fn columns_round_trip_through_json() {
        // The renderer sends these by name; an unknown one must fail to parse
        // rather than become a blank column that looks like missing data.
        let json = serde_json::to_string(&default_columns()).unwrap();
        assert_eq!(json, r#"["title","artist","bpm","key","duration"]"#);
        assert!(serde_json::from_str::<Column>("\"remixer\"").is_err());
    }
}
