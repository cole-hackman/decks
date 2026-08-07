/// Parse ANLZ `.DAT` beat-grid files produced by Rekordbox.
///
/// File layout (all integers big-endian):
///
/// ```text
/// File header (28 bytes):
///   [0..4]   magic "PMAI"
///   [4..8]   header length (u32) — typically 28 (0x1C)
///   [8..12]  total file length (u32)
///   [12..28] reserved
///
/// Sections (repeated until EOF):
///   [0..4]   four-char tag ("PQTZ", "PCOB", …)
///   [4..8]   section header length (u32)
///   [8..12]  total section length including header (u32)
///   [12..]   tag-specific content
///
/// PQTZ content (after 12-byte section header):
///   [0..4]   reserved (0x00800000)
///   [4..8]   beat count (u32)
///   [8..12]  reserved
///   then beat_count × 8-byte entries:
///     [0..2]  beat_number within bar (u16, 1–4)
///     [2..4]  tempo = BPM × 100 (u16)
///     [4..8]  time_ms (u32)
/// ```
use crate::types::BeatGridEntry;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Resolve a Rekordbox `analysis_data_path` (e.g. `/PIONEER/USBANLZ/.../ANLZ0000.DAT`)
/// into an on-disk path relative to a library directory.
///
/// macOS Rekordbox 7 typically lays this out under `<lib_dir>/share/PIONEER/...`,
/// older or sandboxed setups omit the `share` prefix. The path is *absolute*
/// in the database, which means a naive `lib_dir.join(analysis_path)` discards
/// `lib_dir` entirely — callers must trim the leading slash first.
///
/// Returns the first candidate that exists, or `None` if none do.
pub fn resolve_anlz_path(lib_dir: &Path, analysis_path: &str) -> Option<PathBuf> {
    let rel = analysis_path.trim_start_matches('/');
    let with_share = lib_dir.join("share").join(rel);
    if with_share.exists() {
        return Some(with_share);
    }
    let without_share = lib_dir.join(rel);
    if without_share.exists() {
        return Some(without_share);
    }
    let absolute = PathBuf::from(analysis_path);
    if absolute.exists() {
        return Some(absolute);
    }
    None
}

const PMAI_MAGIC: &[u8; 4] = b"PMAI";
const PQTZ_TAG: &[u8; 4] = b"PQTZ";
const PWAV_TAG: &[u8; 4] = b"PWAV";
const PWV3_TAG: &[u8; 4] = b"PWV3";
const PWV4_TAG: &[u8; 4] = b"PWV4";
const PWV5_TAG: &[u8; 4] = b"PWV5";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum WaveformColor {
    Blue(u8),        // 3-bit color value from PWAV/PWV3 (0-7)
    Rgb(u8, u8, u8), // RGB values from PWV4/PWV5
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewPoint {
    pub height: u8,
    pub color: WaveformColor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailPoint {
    pub height: u8,
    pub color: WaveformColor,
}

/// Iterate over all sections in an ANLZ byte slice.
/// The callback receives the 4-byte tag, the header length, and the full section byte slice (including header).
pub fn for_each_section<F>(data: &[u8], mut callback: F) -> Result<()>
where
    F: FnMut(&[u8; 4], usize, &[u8]) -> Result<()>,
{
    ensure!(data.len() >= 12, "ANLZ file too short");
    ensure!(&data[0..4] == PMAI_MAGIC, "ANLZ file missing PMAI magic");

    let header_len = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
    ensure!(
        data.len() >= header_len,
        "ANLZ file shorter than declared header"
    );

    let mut pos = header_len;
    while pos + 12 <= data.len() {
        let tag = &data[pos..pos + 4];
        let section_header_len =
            u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let section_total_len =
            u32::from_be_bytes(data[pos + 8..pos + 12].try_into().unwrap()) as usize;

        // A section smaller than its own header is malformed; stop rather than
        // hand a too-short slice to a parser.
        if section_total_len < 12 {
            break;
        }

        ensure!(
            pos + section_total_len <= data.len(),
            "ANLZ section truncated"
        );

        let section_data = &data[pos..pos + section_total_len];
        callback(tag.try_into().unwrap(), section_header_len, section_data)?;

        pos += section_total_len;
    }

    Ok(())
}

pub fn read_beat_grid(path: &Path) -> Result<Vec<BeatGridEntry>> {
    let data = fs::read(path).with_context(|| format!("reading ANLZ file {}", path.display()))?;
    parse_beat_grid(&data)
}

fn parse_beat_grid(data: &[u8]) -> Result<Vec<BeatGridEntry>> {
    let mut entries = None;
    for_each_section(data, |tag, _header_len, section_data| {
        if tag == PQTZ_TAG && entries.is_none() {
            entries = Some(parse_pqtz_section(section_data)?);
        }
        Ok(())
    })?;
    Ok(entries.unwrap_or_default())
}

pub fn read_preview_waveform(path: &Path) -> Result<Vec<PreviewPoint>> {
    let data = fs::read(path).with_context(|| format!("reading ANLZ file {}", path.display()))?;
    parse_preview_waveform(&data)
}

fn parse_preview_waveform(data: &[u8]) -> Result<Vec<PreviewPoint>> {
    let mut pwv4_entries = None;
    let mut pwav_entries = None;

    for_each_section(data, |tag, _header_len, section_data| {
        if tag == PWV4_TAG && pwv4_entries.is_none() {
            pwv4_entries = Some(parse_pwv4_section(section_data)?);
        } else if tag == PWAV_TAG && pwav_entries.is_none() {
            pwav_entries = Some(parse_pwav_section(section_data)?);
        }
        Ok(())
    })?;

    // Prefer PWV4 (color) over PWAV (monochrome blue)
    if let Some(entries) = pwv4_entries {
        Ok(entries)
    } else if let Some(entries) = pwav_entries {
        Ok(entries)
    } else {
        Ok(vec![])
    }
}

pub fn read_detail_waveform(path: &Path) -> Result<Vec<DetailPoint>> {
    let data = fs::read(path).with_context(|| format!("reading ANLZ file {}", path.display()))?;
    parse_detail_waveform(&data)
}

fn parse_detail_waveform(data: &[u8]) -> Result<Vec<DetailPoint>> {
    let mut pwv5_entries = None;
    let mut pwv3_entries = None;

    for_each_section(data, |tag, _header_len, section_data| {
        if tag == PWV5_TAG && pwv5_entries.is_none() {
            pwv5_entries = Some(parse_pwv5_section(section_data)?);
        } else if tag == PWV3_TAG && pwv3_entries.is_none() {
            pwv3_entries = Some(parse_pwv3_section(section_data)?);
        }
        Ok(())
    })?;

    if let Some(entries) = pwv5_entries {
        Ok(entries)
    } else if let Some(entries) = pwv3_entries {
        Ok(entries)
    } else {
        Ok(vec![])
    }
}

/// Parse a PWV5 section (Detailed Color/3-Band Waveform).
/// 2 bytes per entry, big-endian.
fn parse_pwv5_section(section_data: &[u8]) -> Result<Vec<DetailPoint>> {
    ensure!(section_data.len() >= 8, "PWV5 section truncated");
    let header_len = u32::from_be_bytes(section_data[4..8].try_into().unwrap()) as usize;
    ensure!(
        section_data.len() >= header_len,
        "PWV5 section too short for header"
    );

    let content = &section_data[header_len..];
    let num_entries = content.len() / 2;
    let mut points = Vec::with_capacity(num_entries);

    for i in 0..num_entries {
        let off = i * 2;
        let x = u16::from_be_bytes(content[off..off + 2].try_into().unwrap());

        let red = ((x & 0xE000) >> 13) as u8; // 0-7
        let green = ((x & 0x1C00) >> 10) as u8;
        let blue = ((x & 0x0380) >> 7) as u8;
        let height = ((x & 0x007C) >> 2) as u8; // 0-31

        points.push(DetailPoint {
            height,
            color: WaveformColor::Rgb(red * 36, green * 36, blue * 36),
        });
    }

    Ok(points)
}

/// Parse a PWV3 section (Detailed Monochrome Blue Waveform).
/// 1 byte per entry.
fn parse_pwv3_section(section_data: &[u8]) -> Result<Vec<DetailPoint>> {
    ensure!(section_data.len() >= 8, "PWV3 section truncated");
    let header_len = u32::from_be_bytes(section_data[4..8].try_into().unwrap()) as usize;
    ensure!(
        section_data.len() >= header_len,
        "PWV3 section too short for header"
    );

    let content = &section_data[header_len..];
    let mut points = Vec::with_capacity(content.len());

    for &b in content {
        let height = b & 0x1F;
        let color = b >> 5;

        points.push(DetailPoint {
            height,
            color: WaveformColor::Blue(color * 36),
        });
    }

    Ok(points)
}

/// Parse a PWV4 section (Color Preview Waveform).
/// 6 bytes per entry.
fn parse_pwv4_section(section_data: &[u8]) -> Result<Vec<PreviewPoint>> {
    ensure!(section_data.len() >= 8, "PWV4 section truncated");
    let header_len = u32::from_be_bytes(section_data[4..8].try_into().unwrap()) as usize;
    ensure!(
        section_data.len() >= header_len,
        "PWV4 section too short for header"
    );

    let content = &section_data[header_len..];
    let num_entries = content.len() / 6;
    let mut points = Vec::with_capacity(num_entries);

    for i in 0..num_entries {
        let off = i * 6;
        let r = content[off + 3] & 0x7F; // 0-127
        let g = content[off + 4] & 0x7F;
        let b = content[off + 5] & 0x7F;
        let height = b; // pyrekordbox: `d5` is blue and height of front waveform

        points.push(PreviewPoint {
            height,
            color: WaveformColor::Rgb(r * 2, g * 2, b * 2), // scale to 0-254
        });
    }

    Ok(points)
}

/// Parse a PWAV section (Monochrome Blue Preview Waveform).
/// 1 byte per entry.
fn parse_pwav_section(section_data: &[u8]) -> Result<Vec<PreviewPoint>> {
    ensure!(section_data.len() >= 8, "PWAV section truncated");
    let header_len = u32::from_be_bytes(section_data[4..8].try_into().unwrap()) as usize;
    ensure!(
        section_data.len() >= header_len,
        "PWAV section too short for header"
    );

    let content = &section_data[header_len..];
    let mut points = Vec::with_capacity(content.len());

    for &b in content {
        let height = b & 0x1F;
        let color = b >> 5;

        points.push(PreviewPoint {
            height,
            color: WaveformColor::Blue(color),
        });
    }

    Ok(points)
}

/// Parse a PQTZ section.
/// `section_data` is the entire section including the "PQTZ" tag.
/// Section layout (all big-endian):
/// ```text
///  [0..4]   "PQTZ"
///  [4..8]   section_header_len (u32) = 24
///  [8..12]  section_total_len (u32)
///  [12..16] reserved
///  [16..20] beat_count (u32)
///  [20..24] reserved
///  [24..]   beat entries (8 bytes each)
/// ```
fn parse_pqtz_section(section_data: &[u8]) -> Result<Vec<BeatGridEntry>> {
    ensure!(section_data.len() >= 24, "PQTZ section truncated");
    let header_len = u32::from_be_bytes(section_data[4..8].try_into().unwrap()) as usize;
    ensure!(
        section_data.len() >= header_len,
        "PQTZ section too short for header"
    );
    ensure!(header_len >= 24, "PQTZ header too small");

    let beat_count = u32::from_be_bytes(section_data[16..20].try_into().unwrap()) as usize;

    let beats_start = header_len;

    ensure!(
        beats_start + beat_count * 8 <= section_data.len(),
        "PQTZ beat entries extend beyond section boundary"
    );

    let mut entries = Vec::with_capacity(beat_count);
    for i in 0..beat_count {
        let off = beats_start + i * 8;
        let beat_number = u16::from_be_bytes(section_data[off..off + 2].try_into().unwrap());
        let tempo = u16::from_be_bytes(section_data[off + 2..off + 4].try_into().unwrap());
        let time_ms = u32::from_be_bytes(section_data[off + 4..off + 8].try_into().unwrap());
        entries.push(BeatGridEntry {
            beat_number,
            tempo_bpm_x100: tempo,
            time_ms,
        });
    }
    Ok(entries)
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// Squash a preview waveform down to `bars` columns, for the browser's inline
/// per-row preview.
///
/// Per `docs/lexicon/02-library.md §Browser`. A row is ~28px tall and a few
/// hundred wide, so shipping the full ~400-point preview to draw forty bars
/// would move two orders of magnitude more data than the picture contains —
/// across a four-thousand-track library that is the difference between a
/// feature and a stall.
///
/// **Peak, not mean.** Averaging a bucket flattens exactly what the preview is
/// for: a quiet intro with one stab in it should show the stab. Meaning is more
/// useful than fidelity at this size.
///
/// Returns an empty vector for empty input or `bars == 0`, so a track with no
/// ANLZ renders as nothing rather than as a flat line — "no data" and "silence"
/// must not look the same.
pub fn downsample_preview(preview: &[PreviewPoint], bars: usize) -> Vec<u8> {
    if preview.is_empty() || bars == 0 {
        return Vec::new();
    }
    // Fewer points than bars: keep what there is rather than interpolating
    // detail that was never measured.
    let bars = bars.min(preview.len());

    (0..bars)
        .map(|i| {
            let start = i * preview.len() / bars;
            let end = ((i + 1) * preview.len() / bars).max(start + 1);
            preview[start..end.min(preview.len())]
                .iter()
                .map(|p| p.height)
                .max()
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid ANLZ .DAT byte blob with PQTZ section.
    fn make_anlz(beats: &[(u16, u16, u32)]) -> Vec<u8> {
        let beat_count = beats.len() as u32;
        let section_header_len: u32 = 24; // 12 (tag+lens) + 12 (reserved+count+reserved)
        let section_total_len = section_header_len + beat_count * 8;
        let file_header_len: u32 = 28;
        let file_total_len = file_header_len + section_total_len; // +12 for section header

        let mut buf = Vec::new();

        // File header (28 bytes)
        buf.extend_from_slice(b"PMAI");
        buf.extend_from_slice(&file_header_len.to_be_bytes());
        buf.extend_from_slice(&file_total_len.to_be_bytes());
        buf.extend_from_slice(&[0u8; 16]); // reserved

        // PQTZ section header (12 bytes)
        buf.extend_from_slice(b"PQTZ");
        buf.extend_from_slice(&section_header_len.to_be_bytes());
        buf.extend_from_slice(&section_total_len.to_be_bytes());

        // PQTZ content prefix (12 bytes: reserved, beat_count, reserved)
        buf.extend_from_slice(&[0x00, 0x80, 0x00, 0x00]);
        buf.extend_from_slice(&beat_count.to_be_bytes());
        buf.extend_from_slice(&[0u8; 4]);

        // Beat entries
        for &(beat_num, tempo, time_ms) in beats {
            buf.extend_from_slice(&beat_num.to_be_bytes());
            buf.extend_from_slice(&tempo.to_be_bytes());
            buf.extend_from_slice(&time_ms.to_be_bytes());
        }

        buf
    }

    #[test]
    fn parse_two_beats() {
        let beats = [(1u16, 12800u16, 0u32), (2u16, 12800u16, 468u32)];
        let blob = make_anlz(&beats);
        let entries = parse_beat_grid(&blob).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].beat_number, 1);
        assert_eq!(entries[0].tempo_bpm_x100, 12800);
        assert_eq!(entries[0].time_ms, 0);
        assert_eq!(entries[1].time_ms, 468);
    }

    #[test]
    fn bpm_helper() {
        let entry = BeatGridEntry {
            beat_number: 1,
            tempo_bpm_x100: 13200,
            time_ms: 0,
        };
        assert!((entry.bpm() - 132.0).abs() < 0.001);
    }

    #[test]
    fn empty_beat_grid() {
        let blob = make_anlz(&[]);
        let entries = parse_beat_grid(&blob).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn no_pqtz_section_returns_empty() {
        // Valid ANLZ file but no PQTZ — just a file header.
        let mut buf = Vec::new();
        let header_len: u32 = 28;
        let file_len: u32 = 28;
        buf.extend_from_slice(b"PMAI");
        buf.extend_from_slice(&header_len.to_be_bytes());
        buf.extend_from_slice(&file_len.to_be_bytes());
        buf.extend_from_slice(&[0u8; 16]);
        let entries = parse_beat_grid(&buf).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn bad_magic_returns_error() {
        let blob = b"BADM\x00\x00\x00\x1c\x00\x00\x00\x1c".to_vec();
        assert!(parse_beat_grid(&blob).is_err());
    }

    #[test]
    fn too_short_returns_error() {
        assert!(parse_beat_grid(&[0u8; 4]).is_err());
    }

    #[test]
    fn resolve_anlz_path_with_share_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let rel = "PIONEER/USBANLZ/P000/0001/ANLZ0000.DAT";
        let target = tmp.path().join("share").join(rel);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"x").unwrap();

        let resolved =
            resolve_anlz_path(tmp.path(), "/PIONEER/USBANLZ/P000/0001/ANLZ0000.DAT").unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn resolve_anlz_path_without_share_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let rel = "PIONEER/USBANLZ/P000/0001/ANLZ0000.DAT";
        let target = tmp.path().join(rel);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, b"x").unwrap();

        let resolved =
            resolve_anlz_path(tmp.path(), "/PIONEER/USBANLZ/P000/0001/ANLZ0000.DAT").unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn resolve_anlz_path_missing_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(resolve_anlz_path(tmp.path(), "/PIONEER/NOPE.DAT").is_none());
    }

    #[test]
    fn parse_pqtz_truncated_returns_error_not_panic() {
        // 7 bytes — short enough that section_data[4..8] would panic
        // without the new bounds check.
        let truncated: &[u8] = &[b'P', b'Q', b'T', b'Z', 0, 0, 0];
        assert!(parse_pqtz_section(truncated).is_err());
    }

    #[test]
    fn parse_pwav_truncated_returns_error_not_panic() {
        let truncated: &[u8] = &[b'P', b'W', b'A', b'V', 0, 0, 0];
        assert!(parse_pwav_section(truncated).is_err());
    }

    #[test]
    fn for_each_section_skips_undersized_section() {
        // File: PMAI header (28b) + a single PQTZ section claiming total_len = 4 (< 12).
        // The hardened for_each_section must break out cleanly instead of letting the
        // parser see a truncated slice.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"PMAI");
        buf.extend_from_slice(&28u32.to_be_bytes());
        buf.extend_from_slice(&40u32.to_be_bytes()); // file total len
        buf.extend_from_slice(&[0u8; 16]);
        // Bogus section header
        buf.extend_from_slice(b"PQTZ");
        buf.extend_from_slice(&12u32.to_be_bytes()); // section header len
        buf.extend_from_slice(&4u32.to_be_bytes()); // section total len — too small

        let entries = parse_beat_grid(&buf).unwrap();
        assert!(entries.is_empty(), "should bail without panicking");
    }

    // ── Row-preview downsampling ─────────────────────────────────────────────

    fn preview(heights: &[u8]) -> Vec<PreviewPoint> {
        heights
            .iter()
            .map(|&height| PreviewPoint {
                height,
                color: WaveformColor::Blue(0),
            })
            .collect()
    }

    #[test]
    fn downsampling_keeps_the_peak_of_each_bucket() {
        // Averaging would flatten a quiet intro with one stab in it, which is
        // exactly the shape the preview exists to show.
        let points = preview(&[0, 0, 200, 0, 1, 1, 1, 1]);
        assert_eq!(downsample_preview(&points, 2), vec![200, 1]);
    }

    #[test]
    fn the_output_has_the_requested_number_of_bars() {
        let points = preview(&[1; 400]);
        assert_eq!(downsample_preview(&points, 40).len(), 40);
    }

    #[test]
    fn fewer_points_than_bars_is_not_padded_out() {
        // Interpolating detail that was never measured would be inventing it.
        let points = preview(&[5, 9, 3]);
        assert_eq!(downsample_preview(&points, 40), vec![5, 9, 3]);
    }

    #[test]
    fn no_data_and_silence_do_not_look_the_same() {
        // An empty result renders as nothing; a vector of zeroes would render
        // as a flat line, which is a claim about the audio.
        assert!(downsample_preview(&[], 40).is_empty());
        assert_eq!(downsample_preview(&preview(&[0, 0, 0]), 3), vec![0, 0, 0]);
    }

    #[test]
    fn zero_bars_is_empty_rather_than_a_panic() {
        assert!(downsample_preview(&preview(&[1, 2, 3]), 0).is_empty());
    }

    #[test]
    fn every_input_point_lands_in_exactly_one_bucket() {
        // A gap would silently drop a peak; an overlap would double-count one.
        // 397 is prime, so the buckets cannot divide evenly.
        let points = preview(&(0..397).map(|i| (i % 256) as u8).collect::<Vec<_>>());
        let bars = 40;
        let mut covered = vec![false; points.len()];
        for i in 0..bars {
            let start = i * points.len() / bars;
            let end = ((i + 1) * points.len() / bars).max(start + 1);
            for slot in covered.iter_mut().take(end.min(points.len())).skip(start) {
                assert!(!*slot, "bucket {i} overlaps a previous one");
                *slot = true;
            }
        }
        assert!(
            covered.iter().all(|c| *c),
            "some points were never bucketed"
        );
    }
}
