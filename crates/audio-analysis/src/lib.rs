// Adapted from reklawdbox src/audio.rs (MIT, Ryan Voitiskis).
// Symphonia decode + stratum-dsp analysis pipeline.
// See NOTICE at the workspace root for full attribution.

pub mod playable;

use std::path::Path;

use serde::{Deserialize, Serialize};
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::{get_codecs, get_probe};

/// Analyzer version used as the cache key. Bump this string when the
/// stratum-dsp crate is updated in a way that changes output values.
pub const ANALYZER_VERSION: &str = "stratum-dsp-v1";

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("io: {0}")]
    Io(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("unsupported format: {0}")]
    Unsupported(String),
    #[error("analysis: {0}")]
    Analysis(String),
    #[error("cache: {0}")]
    Cache(String),
}

type Result<T> = std::result::Result<T, AnalysisError>;

/// Analysis result returned to callers. Uses standard Camelot key notation
/// (e.g., "8A" = A minor, "8B" = C major) matching Rekordbox/Mixed In Key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub bpm: f64,
    pub musical_key: String,
    /// Combined confidence (0.0–1.0). Average of BPM + key confidence.
    pub confidence: f64,
    /// BPM confidence from stratum-dsp (0.0–1.0).
    pub bpm_confidence: f64,
    /// Key confidence from stratum-dsp (0.0–1.0).
    pub key_confidence: f64,
    /// Whether this result was served from cache (not re-analyzed).
    pub cached: bool,
}

// ---------------------------------------------------------------------------
// Audio decoding (adapted from reklawdbox audio.rs decode_to_samples)
// ---------------------------------------------------------------------------

/// Decode an audio file to a mono f32 sample buffer.
/// Returns (samples, sample_rate).
fn decode_to_samples(path: &Path) -> Result<(Vec<f32>, u32)> {
    let file = std::fs::File::open(path)
        .map_err(|e| AnalysisError::Io(format!("cannot open {:?}: {e}", path)))?;
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
        .map_err(|e| AnalysisError::Decode(format!("format probe failed: {e}")))?;

    let mut format_reader = probed.format;

    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| AnalysisError::Decode("no audio track found".to_string()))?;

    let track_id = track.id;
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| AnalysisError::Decode("track has no sample rate".to_string()))?;
    let n_frames_hint = track.codec_params.n_frames;

    let mut decoder = get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AnalysisError::Decode(format!("cannot create decoder: {e}")))?;

    let mut samples: Vec<f32> = Vec::with_capacity(n_frames_hint.unwrap_or(0) as usize);
    let mut skip_count = 0u64;

    loop {
        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(AnalysisError::Decode(format!("packet read error: {e}"))),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                skip_count += 1;
                continue;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                match decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(symphonia::core::errors::Error::DecodeError(_)) => {
                        skip_count += 1;
                        continue;
                    }
                    Err(e) => {
                        return Err(AnalysisError::Decode(format!("decode after reset: {e}")))
                    }
                }
            }
            Err(e) => return Err(AnalysisError::Decode(format!("decode error: {e}"))),
        };

        samples.extend_from_slice(&buf_to_mono(&decoded));
    }

    if skip_count > 0 {
        tracing::debug!("{:?}: skipped {skip_count} malformed frames", path);
    }

    if samples.is_empty() {
        return Err(AnalysisError::Decode("decoded zero samples".to_string()));
    }

    Ok((samples, sample_rate))
}

fn buf_to_mono(buf: &AudioBufferRef) -> Vec<f32> {
    match buf {
        AudioBufferRef::F32(b) => downmix(b.planes().planes(), |&v| v),
        AudioBufferRef::F64(b) => downmix(b.planes().planes(), |&v| v as f32),
        AudioBufferRef::S8(b) => downmix(b.planes().planes(), |&v| v as f32 / 128.0),
        AudioBufferRef::S16(b) => downmix(b.planes().planes(), |&v| v as f32 / 32768.0),
        AudioBufferRef::S24(b) => downmix(b.planes().planes(), |v| v.inner() as f32 / 8388608.0),
        AudioBufferRef::S32(b) => downmix(b.planes().planes(), |&v| v as f32 / 2147483648.0),
        AudioBufferRef::U8(b) => downmix(b.planes().planes(), |&v| (v as f32 - 128.0) / 128.0),
        AudioBufferRef::U16(b) => downmix(b.planes().planes(), |&v| (v as f32 - 32768.0) / 32768.0),
        AudioBufferRef::U24(b) => downmix(b.planes().planes(), |v| {
            (v.inner() as f32 - 8388608.0) / 8388608.0
        }),
        AudioBufferRef::U32(b) => downmix(b.planes().planes(), |&v| {
            (v as f64 - 2147483648.0) as f32 / 2147483648.0
        }),
    }
}

fn downmix<T, F>(planes: &[&[T]], convert: F) -> Vec<f32>
where
    F: Fn(&T) -> f32,
{
    if planes.is_empty() {
        return Vec::new();
    }
    let n_ch = planes.len();
    let n_frames = planes.iter().map(|ch| ch.len()).min().unwrap_or(0);

    if n_ch == 1 {
        return planes[0].iter().take(n_frames).map(&convert).collect();
    }

    let weight = 1.0 / n_ch as f32;
    (0..n_frames)
        .map(|i| {
            let sum: f32 = planes.iter().map(|ch| convert(&ch[i])).sum();
            sum * weight
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Key conversion: stratum-dsp notation → standard Camelot
// (adapted from reklawdbox audio.rs stratum_notation_to_camelot)
// ---------------------------------------------------------------------------

// stratum-dsp Key::numerical() returns "1A"–"12A" for major and "1B"–"12B"
// for minor, where A=major and 1=C. Standard Camelot (Rekordbox/Mixed In Key)
// uses A=minor and the circle of fifths starting at C minor = 5A.
//
// Conversion: flip suffix A↔B; camelot_num = (stratum_num + 6) % 12 + 1.
fn stratum_to_camelot(stratum: &str) -> String {
    let suffix = stratum.chars().last();
    let num_str = &stratum[..stratum.len().saturating_sub(1)];

    let (suffix, num) = match (suffix, num_str.parse::<u32>()) {
        (Some(s @ ('A' | 'B')), Ok(n)) if (1..=12).contains(&n) => (s, n),
        _ => return stratum.to_string(),
    };

    let camelot_num = (num + 6) % 12 + 1;
    let camelot_suffix = if suffix == 'A' { 'B' } else { 'A' };
    format!("{camelot_num}{camelot_suffix}")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode an audio file and produce `target_bars` peak amplitudes in [0, 1].
///
/// Each bar is the max absolute sample over the corresponding window of the
/// decoded mono waveform. Returned values are normalized so the loudest bar
/// is `1.0` — empty/silent audio returns zeroed bars.
pub fn extract_waveform_peaks(path: &Path, target_bars: usize) -> Result<Vec<f32>> {
    if target_bars == 0 {
        return Ok(Vec::new());
    }
    let (samples, _sample_rate) = decode_to_samples(path)?;
    Ok(downsample_peaks(&samples, target_bars))
}

fn downsample_peaks(samples: &[f32], target_bars: usize) -> Vec<f32> {
    if target_bars == 0 || samples.is_empty() {
        return vec![0.0; target_bars];
    }
    let mut peaks = vec![0.0f32; target_bars];
    let len = samples.len();
    for (i, peak) in peaks.iter_mut().enumerate() {
        let start = (i * len) / target_bars;
        let end = (((i + 1) * len) / target_bars).max(start + 1);
        let end = end.min(len);
        let mut max = 0.0f32;
        for &s in &samples[start..end] {
            let a = s.abs();
            if a > max {
                max = a;
            }
        }
        *peak = max;
    }
    let max_peak = peaks.iter().copied().fold(0.0f32, f32::max);
    if max_peak > f32::EPSILON {
        for p in peaks.iter_mut() {
            *p /= max_peak;
        }
    }
    peaks
}

/// Decode + analyze an audio file. No caching — caller decides.
pub fn analyze_file(path: &Path) -> Result<AnalysisResult> {
    let (samples, sample_rate) = decode_to_samples(path)?;

    let result = stratum_dsp::analyze_audio(
        &samples,
        sample_rate,
        stratum_dsp::AnalysisConfig::default(),
    )
    .map_err(|e| AnalysisError::Analysis(e.to_string()))?;

    let stratum_key = result.key.numerical();
    let camelot_key = stratum_to_camelot(&stratum_key);

    let bpm_conf = result.bpm_confidence as f64;
    let key_conf = result.key_confidence as f64;

    Ok(AnalysisResult {
        bpm: result.bpm as f64,
        musical_key: camelot_key,
        confidence: (bpm_conf + key_conf) / 2.0,
        bpm_confidence: bpm_conf,
        key_confidence: key_conf,
        cached: false,
    })
}

/// Decode an audio file and extract a compact binary chromagram fingerprint.
/// The fingerprint is 128 bytes long. Each byte represents the dominant pitch class (0-11)
/// in the corresponding time slice of the track.
pub fn extract_audio_fingerprint(path: &Path) -> Result<Vec<u8>> {
    let (samples, sample_rate) = decode_to_samples(path)?;
    if samples.is_empty() {
        return Ok(vec![0; 128]);
    }

    // Large hop size for fast extraction. We just need the general harmonic shape.
    let frame_size = 4096;
    let hop_size = 4096;

    let chroma_matrix = stratum_dsp::features::chroma::extractor::extract_chroma(
        &samples,
        sample_rate,
        frame_size,
        hop_size,
    )
    .map_err(|e| AnalysisError::Analysis(format!("chroma extraction failed: {}", e)))?;

    if chroma_matrix.is_empty() {
        return Ok(vec![0; 128]);
    }

    let target_len = 128;
    let mut fingerprint = vec![0u8; target_len];
    let frames_per_bin = (chroma_matrix.len() as f64 / target_len as f64).max(1.0);

    for (i, slot) in fingerprint.iter_mut().enumerate().take(target_len) {
        let start_idx = (i as f64 * frames_per_bin) as usize;
        let mut end_idx = ((i + 1) as f64 * frames_per_bin) as usize;
        if start_idx >= chroma_matrix.len() {
            break;
        }
        end_idx = end_idx.min(chroma_matrix.len()).max(start_idx + 1);

        let mut avg_chroma = [0.0f32; 12];
        for frame in &chroma_matrix[start_idx..end_idx] {
            for (bin, &val) in frame.iter().enumerate() {
                avg_chroma[bin] += val;
            }
        }

        let mut max_val = -1.0f32;
        let mut max_idx = 0;
        for (bin, &val) in avg_chroma.iter().enumerate() {
            if val > max_val {
                max_val = val;
                max_idx = bin;
            }
        }
        *slot = max_idx as u8;
    }

    Ok(fingerprint)
}

/// Check cache, analyze on miss, upsert result, return.
///
/// `track_uri` is used as the cache key — typically the absolute file path.
pub fn analyze_file_cached(
    path: &Path,
    track_uri: &str,
    cache: &cache::CacheDb,
) -> Result<AnalysisResult> {
    // Cache hit?
    if let Some(cached) = cache
        .get_audio_features(track_uri, ANALYZER_VERSION)
        .map_err(|e| AnalysisError::Cache(e.to_string()))?
    {
        if let (Some(bpm), Some(key)) = (cached.bpm, cached.musical_key) {
            return Ok(AnalysisResult {
                bpm,
                musical_key: key,
                confidence: 0.0, // confidence not stored in cache
                bpm_confidence: 0.0,
                key_confidence: 0.0,
                cached: true,
            });
        }
    }

    // Cache miss — run full analysis.
    let result = analyze_file(path)?;

    cache
        .upsert_audio_features(
            track_uri,
            ANALYZER_VERSION,
            Some(result.bpm),
            Some(&result.musical_key),
            None,
            None,
        )
        .map_err(|e| AnalysisError::Cache(e.to_string()))?;

    Ok(result)
}

/// Check cache for fingerprint, extract on miss, save, return.
pub fn extract_fingerprint_cached(
    path: &Path,
    track_uri: &str,
    cache_db: &cache::CacheDb,
) -> Result<Vec<u8>> {
    if let Ok(mut all_fps) = cache_db.get_all_fingerprints() {
        for (uri, hash) in all_fps.drain(..) {
            if uri == track_uri {
                return Ok(hash);
            }
        }
    }

    let fp = extract_audio_fingerprint(path)?;
    let _ = cache_db.save_audio_fingerprint(track_uri, &fp);
    Ok(fp)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stratum_to_camelot_c_major() {
        // stratum-dsp "1A" = C major → standard Camelot "8B"
        assert_eq!(stratum_to_camelot("1A"), "8B");
    }

    #[test]
    fn stratum_to_camelot_a_minor() {
        // stratum-dsp "1B" = A minor → standard Camelot "8A"
        assert_eq!(stratum_to_camelot("1B"), "8A");
    }

    #[test]
    fn stratum_to_camelot_g_major() {
        // stratum-dsp "2A" = G major → standard Camelot "9B"
        assert_eq!(stratum_to_camelot("2A"), "9B");
    }

    #[test]
    fn downsample_peaks_buckets_and_normalizes() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32) / 999.0).collect();
        let peaks = downsample_peaks(&samples, 10);
        assert_eq!(peaks.len(), 10);
        // last bucket contains highest absolute sample → 1.0 after normalization
        assert!((peaks[9] - 1.0).abs() < 1e-6);
        // monotonically non-decreasing for this ramp
        for w in peaks.windows(2) {
            assert!(w[1] >= w[0] - 1e-6);
        }
    }

    #[test]
    fn downsample_peaks_silence_is_zeros() {
        let peaks = downsample_peaks(&vec![0.0f32; 200], 8);
        assert_eq!(peaks, vec![0.0; 8]);
    }

    #[test]
    fn stratum_to_camelot_passthrough_invalid() {
        assert_eq!(stratum_to_camelot("Cm"), "Cm");
        assert_eq!(stratum_to_camelot(""), "");
    }

    #[test]
    fn stratum_to_camelot_full_wheel() {
        // Verify all 12 major keys map to correct Camelot B-suffixed positions.
        // C=8B G=9B D=10B A=11B E=12B B=1B F#=2B C#=3B G#=4B D#=5B A#=6B F=7B
        let expected_major = [
            ("1A", "8B"),  // C
            ("2A", "9B"),  // G
            ("3A", "10B"), // D
            ("4A", "11B"), // A
            ("5A", "12B"), // E
            ("6A", "1B"),  // B
            ("7A", "2B"),  // F#
            ("8A", "3B"),  // C#
            ("9A", "4B"),  // G#
            ("10A", "5B"), // D#
            ("11A", "6B"), // A#
            ("12A", "7B"), // F
        ];
        for (stratum, camelot) in expected_major {
            assert_eq!(stratum_to_camelot(stratum), camelot, "stratum {stratum}");
        }
    }
}
