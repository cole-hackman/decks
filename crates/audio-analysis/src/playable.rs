//! Find Broken Tracks — does this file actually decode?
//!
//! The existing broken-link scan asks whether the path exists. That misses the
//! failure DJs actually meet: the file is right there, and the deck plays two
//! seconds of it and stops. A truncated download, a half-copied file, a `.mp3`
//! that is really an HTML error page — all present, all unplayable.
//!
//! Two depths, because the honest ones cost different amounts:
//!
//! - [`CheckDepth::Header`] probes the container and builds a decoder. Fast,
//!   and catches wrong-format files, unsupported codecs and zero-length files.
//!   It does **not** catch a file that is fine until the last ten seconds.
//! - [`CheckDepth::Full`] decodes every packet and throws the samples away.
//!   This is the one that catches truncation, which is the common real case —
//!   and it costs roughly what analysing the track costs.
//!
//! Nothing here deletes anything. It reports, and the caller decides.
//!
//! See `docs/lexicon/07-health.md §Find Broken Tracks`.

use crate::AnalysisError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

/// How hard to look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckDepth {
    /// Probe the container and build a decoder. Fast; misses late corruption.
    Header,
    /// Decode the whole file, discarding the audio. Catches truncation.
    #[default]
    Full,
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum PlaybackStatus {
    Ok,
    /// The path does not exist. The old existence check, kept as one outcome
    /// among several rather than as the whole feature.
    Missing,
    /// It exists but could not be opened — permissions, a dead network mount.
    Unreadable(String),
    /// Opened, but nothing could make sense of it: wrong format, unsupported
    /// codec, empty file.
    Undecodable(String),
    /// Decoding stopped early. This is what a truncated download looks like.
    Truncated(String),
    /// Decoded end to end, but some packets were bad. Playable, with glitches
    /// — reported rather than passed, because the user may want to replace it.
    Damaged {
        bad_packets: u64,
    },
}

impl PlaybackStatus {
    /// Whether the file needs the user's attention.
    pub fn is_broken(&self) -> bool {
        !matches!(self, PlaybackStatus::Ok)
    }

    /// Whether the file is gone rather than merely bad. Deleting a file that
    /// is already absent is a different (and simpler) fix.
    pub fn is_missing(&self) -> bool {
        matches!(self, PlaybackStatus::Missing)
    }
}

/// Check one file.
///
/// A file with no audio frames at all is `Undecodable` rather than `Ok`: a
/// zero-length "track" plays as silence, which is not a working file however
/// cleanly it parses.
pub fn verify_playable(path: &Path, depth: CheckDepth) -> PlaybackStatus {
    if !path.exists() {
        return PlaybackStatus::Missing;
    }
    match probe(path, depth) {
        Ok(status) => status,
        Err(AnalysisError::Io(detail)) => PlaybackStatus::Unreadable(detail),
        Err(e) => PlaybackStatus::Undecodable(e.to_string()),
    }
}

fn probe(path: &Path, depth: CheckDepth) -> Result<PlaybackStatus, AnalysisError> {
    let file =
        std::fs::File::open(path).map_err(|e| AnalysisError::Io(format!("cannot open: {e}")))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AnalysisError::Decode(format!("not a recognised audio file: {e}")))?;

    let mut reader = probed.format;
    let track = reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| AnalysisError::Decode("no audio track".into()))?;
    let track_id = track.id;
    // What the header says the file should contain. Raw PCM has no framing to
    // fail on, so a truncated WAV decodes cleanly and simply ends early — the
    // only way to notice is to compare what arrived against what was promised.
    // Compressed formats usually error mid-stream instead, and are caught below.
    let declared_frames = track.codec_params.n_frames;

    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AnalysisError::Decode(format!("no decoder for this codec: {e}")))?;

    let mut frames = 0u64;
    let mut bad_packets = 0u64;

    loop {
        match reader.next_packet() {
            Ok(packet) => {
                if packet.track_id() != track_id {
                    continue;
                }
                match decoder.decode(&packet) {
                    Ok(buf) => {
                        frames += buf.frames() as u64;
                        // Header depth stops at the first packet that produced
                        // audio: the decoder built and produced samples, which
                        // is all this depth claims to check.
                        if depth == CheckDepth::Header && frames > 0 {
                            break;
                        }
                    }
                    Err(symphonia::core::errors::Error::DecodeError(_)) => bad_packets += 1,
                    Err(symphonia::core::errors::Error::ResetRequired) => decoder.reset(),
                    Err(e) => {
                        return Ok(PlaybackStatus::Truncated(format!("decode failed: {e}")));
                    }
                }
            }
            // A clean end of stream.
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => decoder.reset(),
            // Anything else mid-stream means the file stops before it should.
            Err(e) => {
                return Ok(PlaybackStatus::Truncated(format!(
                    "stream ended early: {e}"
                )));
            }
        }
    }

    if frames == 0 {
        return Ok(PlaybackStatus::Undecodable(
            "the file contains no audio".into(),
        ));
    }
    // Only at full depth: a header check stops after the first packet, so it
    // is short by design and comparing it to the declared length would report
    // every file as truncated.
    if depth == CheckDepth::Full {
        if let Some(declared) = declared_frames.filter(|d| *d > 0) {
            // A 1% tolerance, because encoder padding and gapless trimming can
            // legitimately leave a decode a hair short of the declared length.
            if frames * 100 < declared * 99 {
                let got = frames as f64 / declared as f64 * 100.0;
                return Ok(PlaybackStatus::Truncated(format!(
                    "only {got:.0}% of the audio the header promises is present"
                )));
            }
        }
    }
    if bad_packets > 0 {
        return Ok(PlaybackStatus::Damaged { bad_packets });
    }
    Ok(PlaybackStatus::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A minimal 16-bit mono PCM WAV.
    ///
    /// Built by hand rather than committed as a fixture: `fixtures/audio/` is
    /// gitignored by design, and a check that only ran against real files
    /// could not be tested at all.
    fn wav(samples: u32) -> Vec<u8> {
        let data_len = samples * 2;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&44_100u32.to_le_bytes());
        out.extend_from_slice(&88_200u32.to_le_bytes()); // byte rate
        out.extend_from_slice(&2u16.to_le_bytes()); // block align
        out.extend_from_slice(&16u16.to_le_bytes()); // bits
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..samples {
            out.extend_from_slice(&((i as i16).wrapping_mul(97)).to_le_bytes());
        }
        out
    }

    fn write(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        (dir, path)
    }

    #[test]
    fn a_file_that_is_not_there_is_missing_not_undecodable() {
        // The distinction matters: one is fixed by relocating, the other by
        // re-downloading.
        let status = verify_playable(std::path::Path::new("/nope/never.mp3"), CheckDepth::Full);
        assert_eq!(status, PlaybackStatus::Missing);
        assert!(status.is_missing());
    }

    #[test]
    fn a_good_file_passes_at_both_depths() {
        let (_dir, path) = write("ok.wav", &wav(4096));
        assert_eq!(verify_playable(&path, CheckDepth::Full), PlaybackStatus::Ok);
        assert_eq!(
            verify_playable(&path, CheckDepth::Header),
            PlaybackStatus::Ok
        );
    }

    #[test]
    fn an_html_error_page_saved_as_an_mp3_is_undecodable() {
        // The classic: a download that returned a 404 body with the right name.
        let (_dir, path) = write("track.mp3", b"<html><body>Not Found</body></html>");
        assert!(matches!(
            verify_playable(&path, CheckDepth::Full),
            PlaybackStatus::Undecodable(_)
        ));
    }

    #[test]
    fn an_empty_file_is_undecodable_not_ok() {
        let (_dir, path) = write("empty.wav", b"");
        assert!(matches!(
            verify_playable(&path, CheckDepth::Full),
            PlaybackStatus::Undecodable(_)
        ));
    }

    #[test]
    fn a_header_with_no_audio_is_undecodable_not_ok() {
        // It parses cleanly and plays as silence, which is not a working file.
        let (_dir, path) = write("silent.wav", &wav(0));
        assert!(matches!(
            verify_playable(&path, CheckDepth::Full),
            PlaybackStatus::Undecodable(_)
        ));
    }

    #[test]
    fn a_truncated_file_is_caught_by_a_full_check() {
        // The common real case: the download stopped part-way, so the header
        // promises more audio than the file holds.
        let mut bytes = wav(8192);
        bytes.truncate(bytes.len() / 2);
        let (_dir, path) = write("cut.wav", &bytes);
        match verify_playable(&path, CheckDepth::Full) {
            // The message names the shortfall, so the user can tell a
            // half-finished download from a codec they cannot play.
            PlaybackStatus::Truncated(why) => assert!(why.contains('%'), "{why}"),
            other => panic!("a half-length file should not pass: {other:?}"),
        }
    }

    #[test]
    fn a_whole_file_is_not_called_truncated_by_encoder_padding() {
        // The 1% tolerance exists so gapless trimming does not mark every
        // legitimate file broken.
        let (_dir, path) = write("ok.wav", &wav(44_100));
        assert_eq!(verify_playable(&path, CheckDepth::Full), PlaybackStatus::Ok);
    }

    #[test]
    fn a_header_check_passes_a_file_a_full_check_rejects() {
        // This is the trade the two depths exist to express, and the reason
        // the UI has to say which one it ran.
        let mut bytes = wav(8192);
        bytes.truncate(bytes.len() / 2);
        let (_dir, path) = write("cut.wav", &bytes);
        assert_eq!(
            verify_playable(&path, CheckDepth::Header),
            PlaybackStatus::Ok
        );
    }

    #[test]
    fn broken_is_anything_that_is_not_ok() {
        assert!(!PlaybackStatus::Ok.is_broken());
        assert!(PlaybackStatus::Missing.is_broken());
        assert!(PlaybackStatus::Damaged { bad_packets: 1 }.is_broken());
        // Damaged is broken but not missing — it can still be played.
        assert!(!PlaybackStatus::Damaged { bad_packets: 1 }.is_missing());
    }

    #[test]
    fn statuses_round_trip_through_json() {
        let all = vec![
            PlaybackStatus::Ok,
            PlaybackStatus::Missing,
            PlaybackStatus::Unreadable("permission denied".into()),
            PlaybackStatus::Undecodable("not audio".into()),
            PlaybackStatus::Truncated("ended early".into()),
            PlaybackStatus::Damaged { bad_packets: 3 },
        ];
        let json = serde_json::to_string(&all).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<PlaybackStatus>>(&json).unwrap(),
            all
        );
    }
}
