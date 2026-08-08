//! Energy: how hard a track hits, on a fixed 1–10 scale.
//!
//! Per `docs/lexicon/04-analysis.md §Energy`, the scale is explicitly
//! **absolute**, not per-library relative: "chill tracks should land low and
//! powerful/fast tracks high, on a fixed scale". A track's number must not
//! change because the rest of the library changed around it, which rules out
//! ranking or percentile normalisation — every anchor below is a fixed physical
//! quantity (dBFS, Hz, BPM), so the same file always yields the same number.
//!
//! `GAPS.md` open question 2 asked for a written definition before any
//! implementation, because Lexicon ships two mutually incompatible energy
//! scales (its own analyzer, and Spotify's `audio-features`) and we would
//! otherwise be inventing a third by accident. **ADR-0015 is that definition**;
//! this module is its implementation, and the anchors here are the ones the ADR
//! records. Changing a weight or an anchor changes every stored number, so it
//! is an ADR amendment plus an `ANALYZER_VERSION` bump, not a tweak.
//!
//! Spotify's endpoint is deprecated and closed to new applications, so there is
//! no option to adopt its numbers even if we wanted the compatibility.
//!
//! **No new dependency.** ADR-0012 adopted `libebur128` for loudness, and a
//! gated ITU-R BS.1770 measurement would be a better loudness term than the
//! frame-RMS one below. It is not pulled in here because loudness is one of
//! four terms and the crate is not otherwise in the tree; swapping the loudness
//! term for real LUFS later is a contained change to `loudness_dbfs` plus a
//! version bump. That is recorded in the ADR as a known approximation rather
//! than left as a silent shortcut.

/// Frame length for the RMS envelope, in seconds.
///
/// ~46 ms is short enough to resolve individual kicks at club tempos (a 128 BPM
/// beat is 469 ms, so ten frames) and long enough that the envelope is not
/// tracking the waveform itself.
const FRAME_SECS: f32 = 0.046;

/// dBFS mapped to the bottom and top of the loudness term.
///
/// −24 dBFS is roughly an unmastered or deliberately quiet mix; −7 dBFS is
/// about as loud as a modern master gets before clipping.
const LOUDNESS_FLOOR_DB: f32 = -24.0;
const LOUDNESS_CEIL_DB: f32 = -7.0;

/// Envelope-movement mapped to the bottom and top of the drive term.
///
/// Expressed as a fraction of mean RMS, so it is level-independent: turning a
/// track down does not make it less percussive.
const DRIVE_CEIL: f32 = 0.30;

/// Spectral-centroid proxy mapped to the bottom and top of the brightness term,
/// in Hz. Compared on a log axis, because pitch perception is.
const BRIGHTNESS_FLOOR_HZ: f32 = 400.0;
const BRIGHTNESS_CEIL_HZ: f32 = 5000.0;

/// BPM mapped to the bottom and top of the tempo term.
const TEMPO_FLOOR_BPM: f32 = 70.0;
const TEMPO_CEIL_BPM: f32 = 150.0;

/// Term weights. They sum to 1.0; the assertion in the tests pins that.
const W_LOUDNESS: f32 = 0.35;
const W_DRIVE: f32 = 0.25;
const W_BRIGHTNESS: f32 = 0.25;
const W_TEMPO: f32 = 0.15;

/// The four measurements the score is built from.
///
/// Split out from the scoring so the mapping can be unit-tested as a pure
/// function of physical quantities, without needing audio to exercise it — the
/// container has no audio fixtures (`fixtures/audio/` holds only a `.gitkeep`,
/// per `GAPS.md`), so a design that could only be tested against real files
/// could not be tested here at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyFeatures {
    /// Loudness of the track's louder half, in dBFS. Negative.
    pub loudness_dbfs: f32,
    /// Mean rise in the RMS envelope per frame, as a fraction of mean RMS.
    pub drive: f32,
    /// Spectral-centroid proxy, in Hz.
    pub brightness_hz: f32,
    /// Tempo, in BPM, as detected by the analyzer.
    pub bpm: f32,
}

/// Linear interpolation of `v` between `lo` and `hi`, clamped to 0..=1.
fn ramp(v: f32, lo: f32, hi: f32) -> f32 {
    if !v.is_finite() || hi <= lo {
        return 0.0;
    }
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// Combine the four terms into the stored 0.1–1.0 energy value.
///
/// The stored range starts at 0.1 rather than 0.0 so that the display mapping
/// every consumer already uses — `(energy * 10.0).round()`, in
/// `sync_mappings.rs` and `write_tags.rs` — lands in **1–10** rather than
/// 0–10. Lexicon's scale has no zero, and a silent file is a 1, not an absence.
pub fn score(f: &EnergyFeatures) -> f32 {
    let loudness = ramp(f.loudness_dbfs, LOUDNESS_FLOOR_DB, LOUDNESS_CEIL_DB);
    let drive = ramp(f.drive, 0.0, DRIVE_CEIL);
    // Log axis: 400→800 Hz is the same perceptual step as 2000→4000 Hz.
    let brightness = ramp(
        f.brightness_hz.max(1.0).log2(),
        BRIGHTNESS_FLOOR_HZ.log2(),
        BRIGHTNESS_CEIL_HZ.log2(),
    );
    let tempo = ramp(f.bpm, TEMPO_FLOOR_BPM, TEMPO_CEIL_BPM);

    let combined =
        W_LOUDNESS * loudness + W_DRIVE * drive + W_BRIGHTNESS * brightness + W_TEMPO * tempo;

    (0.1 + 0.9 * combined).clamp(0.1, 1.0)
}

/// The 1–10 integer a user sees, from a stored 0.1–1.0 value.
///
/// Kept here next to `score` so the two halves of the scale cannot drift, and
/// so the round-trip is testable in one place.
pub fn to_display(energy: f32) -> u8 {
    ((energy * 10.0).round() as i32).clamp(1, 10) as u8
}

/// RMS of each fixed-length frame of `samples`.
fn frame_rms(samples: &[f32], frame_len: usize) -> Vec<f32> {
    if frame_len == 0 || samples.is_empty() {
        return Vec::new();
    }
    samples
        .chunks(frame_len)
        .map(|c| {
            let sum: f64 = c.iter().map(|&s| (s as f64) * (s as f64)).sum();
            (sum / c.len() as f64).sqrt() as f32
        })
        .collect()
}

/// Loudness of the louder half of the track, in dBFS.
///
/// The quiet half is discarded rather than averaged in, because a long ambient
/// intro or a fade-out otherwise drags a genuinely loud track down — the number
/// should describe the track as it plays on a floor, not its mean amplitude.
fn loudness_dbfs(envelope: &[f32]) -> f32 {
    if envelope.is_empty() {
        return LOUDNESS_FLOOR_DB;
    }
    let mut sorted: Vec<f32> = envelope.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let loud = &sorted[sorted.len() / 2..];
    let mean = loud.iter().map(|&v| v as f64).sum::<f64>() / loud.len() as f64;
    if mean <= 1e-9 {
        return LOUDNESS_FLOOR_DB;
    }
    (20.0 * mean.log10()) as f32
}

/// Mean frame-to-frame *rise* in the envelope, as a fraction of mean RMS.
///
/// Only rises count. A kick pushes the envelope up sharply and it decays back
/// down; counting the decay as well would score a slow fade the same as a hit,
/// and the sum of signed differences over a whole track is ~0 by construction.
fn drive(envelope: &[f32]) -> f32 {
    if envelope.len() < 2 {
        return 0.0;
    }
    let mean = envelope.iter().map(|&v| v as f64).sum::<f64>() / envelope.len() as f64;
    if mean <= 1e-9 {
        return 0.0;
    }
    let rises: f64 = envelope
        .windows(2)
        .map(|w| ((w[1] - w[0]) as f64).max(0.0))
        .sum();
    (rises / (envelope.len() - 1) as f64 / mean) as f32
}

/// Spectral-centroid proxy, in Hz, from the first-difference energy ratio.
///
/// For a sinusoid at frequency `f`, `rms(diff(x)) / rms(x) = 2·sin(π·f/fs)`
/// exactly, so inverting gives the frequency back. For real signals it lands
/// near the centroid, and it is monotonic in brightness, which is all the
/// mapping above needs. The point of doing it this way rather than with an FFT
/// is that it costs one pass over the samples and no dependency.
///
/// Dividing by `fs` before the inversion is what makes the answer a frequency
/// rather than a number that changes with the file's sample rate.
fn brightness_hz(samples: &[f32], sample_rate: u32) -> f32 {
    if samples.len() < 2 || sample_rate == 0 {
        return BRIGHTNESS_FLOOR_HZ;
    }
    let energy: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    if energy <= 1e-12 {
        return BRIGHTNESS_FLOOR_HZ;
    }
    let diff_energy: f64 = samples
        .windows(2)
        .map(|w| {
            let d = (w[1] - w[0]) as f64;
            d * d
        })
        .sum();
    let ratio = (diff_energy / energy).sqrt();
    // asin is undefined past 1.0, which a ratio above 2.0 (i.e. content above
    // Nyquist/2 dominating, plus rounding) can produce.
    let arg = (ratio / 2.0).clamp(0.0, 1.0);
    (sample_rate as f64 / std::f64::consts::PI * arg.asin()) as f32
}

/// Measure the four terms from decoded mono audio.
///
/// `bpm` comes from the analyzer that already ran; tempo is not re-detected
/// here.
pub fn extract(samples: &[f32], sample_rate: u32, bpm: f32) -> EnergyFeatures {
    let frame_len = ((sample_rate as f32 * FRAME_SECS) as usize).max(1);
    let envelope = frame_rms(samples, frame_len);
    EnergyFeatures {
        loudness_dbfs: loudness_dbfs(&envelope),
        drive: drive(&envelope),
        brightness_hz: brightness_hz(samples, sample_rate),
        bpm,
    }
}

/// Decoded audio in, stored 0.1–1.0 energy out.
pub fn analyze(samples: &[f32], sample_rate: u32, bpm: f32) -> f32 {
    score(&extract(samples, sample_rate, bpm))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, sample_rate: u32, secs: f32, amp: f32) -> Vec<f32> {
        let n = (sample_rate as f32 * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                amp * (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect()
    }

    #[test]
    fn the_weights_sum_to_one() {
        // Otherwise the top of the scale is unreachable or the clamp is doing
        // the work, and neither is what the anchors claim.
        assert!((W_LOUDNESS + W_DRIVE + W_BRIGHTNESS + W_TEMPO - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_floor_of_every_term_is_a_one() {
        let f = EnergyFeatures {
            loudness_dbfs: LOUDNESS_FLOOR_DB,
            drive: 0.0,
            brightness_hz: BRIGHTNESS_FLOOR_HZ,
            bpm: TEMPO_FLOOR_BPM,
        };
        assert_eq!(to_display(score(&f)), 1);
    }

    #[test]
    fn the_ceiling_of_every_term_is_a_ten() {
        let f = EnergyFeatures {
            loudness_dbfs: LOUDNESS_CEIL_DB,
            drive: DRIVE_CEIL,
            brightness_hz: BRIGHTNESS_CEIL_HZ,
            bpm: TEMPO_CEIL_BPM,
        };
        assert_eq!(to_display(score(&f)), 10);
    }

    #[test]
    fn the_scale_has_no_zero_and_no_eleven() {
        // Lexicon's scale is 1–10. Anything outside the anchors clamps into it
        // rather than escaping — a track quieter than the floor is still a 1.
        let below = EnergyFeatures {
            loudness_dbfs: -80.0,
            drive: -1.0,
            brightness_hz: 1.0,
            bpm: 0.0,
        };
        let above = EnergyFeatures {
            loudness_dbfs: 0.0,
            drive: 5.0,
            brightness_hz: 20_000.0,
            bpm: 300.0,
        };
        assert_eq!(to_display(score(&below)), 1);
        assert_eq!(to_display(score(&above)), 10);
    }

    #[test]
    fn louder_is_never_lower() {
        let base = EnergyFeatures {
            loudness_dbfs: -20.0,
            drive: 0.1,
            brightness_hz: 1500.0,
            bpm: 124.0,
        };
        let louder = EnergyFeatures {
            loudness_dbfs: -10.0,
            ..base
        };
        assert!(score(&louder) > score(&base));
    }

    #[test]
    fn faster_brighter_and_punchier_all_raise_the_score() {
        let base = EnergyFeatures {
            loudness_dbfs: -16.0,
            drive: 0.1,
            brightness_hz: 1000.0,
            bpm: 100.0,
        };
        for changed in [
            EnergyFeatures { bpm: 140.0, ..base },
            EnergyFeatures {
                brightness_hz: 3000.0,
                ..base
            },
            EnergyFeatures {
                drive: 0.25,
                ..base
            },
        ] {
            assert!(
                score(&changed) > score(&base),
                "expected {changed:?} to outscore {base:?}"
            );
        }
    }

    #[test]
    fn a_nan_measurement_does_not_poison_the_score() {
        // Division by a near-zero mean can produce NaN upstream; a NaN that
        // propagated would be stored and would then compare false against every
        // smartlist rule, silently removing the track from energy filters.
        let f = EnergyFeatures {
            loudness_dbfs: f32::NAN,
            drive: f32::NAN,
            brightness_hz: f32::NAN,
            bpm: f32::NAN,
        };
        let s = score(&f);
        assert!(s.is_finite());
        assert_eq!(to_display(s), 1);
    }

    #[test]
    fn silence_bottoms_out_every_term_it_can() {
        // Not a 1, and deliberately so: the tempo term is a measurement of the
        // file that silence does not invalidate, and it is 15% of the score. No
        // single term can carry the number — that is what the weights buy — so
        // no single term can sink it either.
        let f = extract(&vec![0.0f32; 44_100], 44_100, 128.0);
        assert_eq!(f.loudness_dbfs, LOUDNESS_FLOOR_DB);
        assert_eq!(f.drive, 0.0);
        assert_eq!(f.brightness_hz, BRIGHTNESS_FLOOR_HZ);
        assert_eq!(to_display(analyze(&vec![0.0f32; 44_100], 44_100, 128.0)), 2);
        // With nothing at all to go on, it is a 1.
        assert_eq!(to_display(analyze(&vec![0.0f32; 44_100], 44_100, 0.0)), 1);
    }

    #[test]
    fn an_empty_buffer_does_not_panic() {
        assert_eq!(to_display(analyze(&[], 44_100, 0.0)), 1);
        assert_eq!(to_display(analyze(&[], 0, 128.0)), 2);
    }

    #[test]
    fn brightness_recovers_the_frequency_of_a_sine() {
        // The inversion is exact for a sinusoid, which is what makes this term
        // a frequency rather than an arbitrary index.
        for freq in [440.0f32, 1000.0, 4000.0] {
            let s = sine(freq, 44_100, 1.0, 0.5);
            let got = brightness_hz(&s, 44_100);
            assert!(
                (got - freq).abs() / freq < 0.05,
                "expected ~{freq} Hz, got {got}"
            );
        }
    }

    #[test]
    fn brightness_does_not_move_with_the_sample_rate() {
        // The same tone in a 48 kHz file must not read as a different track.
        let a = brightness_hz(&sine(1000.0, 44_100, 1.0, 0.5), 44_100);
        let b = brightness_hz(&sine(1000.0, 48_000, 1.0, 0.5), 48_000);
        assert!((a - b).abs() < 30.0, "44.1k gave {a}, 48k gave {b}");
    }

    #[test]
    fn brightness_does_not_move_with_the_volume() {
        // Both terms scale with amplitude, so the ratio must not.
        let loud = brightness_hz(&sine(1000.0, 44_100, 1.0, 0.9), 44_100);
        let quiet = brightness_hz(&sine(1000.0, 44_100, 1.0, 0.05), 44_100);
        assert!((loud - quiet).abs() < 1.0);
    }

    #[test]
    fn loudness_reads_dbfs_off_a_known_amplitude() {
        // A full-scale sine is −3 dBFS RMS; half-scale is −9.
        let full = loudness_dbfs(&frame_rms(&sine(200.0, 44_100, 2.0, 1.0), 2028));
        let half = loudness_dbfs(&frame_rms(&sine(200.0, 44_100, 2.0, 0.5), 2028));
        assert!((full - -3.0).abs() < 0.5, "got {full}");
        assert!((half - -9.0).abs() < 0.5, "got {half}");
    }

    #[test]
    fn a_quiet_intro_does_not_drag_the_loudness_down() {
        // Half the frames silent, half at full scale: taking the louder half
        // reports the loud part, where a plain mean would report ~6 dB lower.
        let mut env = vec![0.0f32; 100];
        env.extend(std::iter::repeat_n(std::f32::consts::FRAC_1_SQRT_2, 100));
        assert!((loudness_dbfs(&env) - -3.0).abs() < 0.5);
    }

    #[test]
    fn a_steady_tone_has_no_drive_and_a_pulse_train_does() {
        let steady = vec![0.5f32; 200];
        assert!(drive(&steady) < 1e-6);

        // Alternating loud/quiet frames is what a kick pattern looks like in
        // the envelope.
        let pulsed: Vec<f32> = (0..200)
            .map(|i| if i % 2 == 0 { 0.9 } else { 0.1 })
            .collect();
        assert!(drive(&pulsed) > 0.5, "got {}", drive(&pulsed));
    }

    #[test]
    fn drive_does_not_move_with_the_volume() {
        // Normalising by mean RMS is what makes this a measure of how the track
        // is *shaped* rather than how loud it was mastered.
        let loud: Vec<f32> = (0..200)
            .map(|i| if i % 2 == 0 { 0.9 } else { 0.1 })
            .collect();
        let quiet: Vec<f32> = loud.iter().map(|v| v * 0.01).collect();
        assert!((drive(&loud) - drive(&quiet)).abs() < 1e-3);
    }

    #[test]
    fn the_same_audio_always_scores_the_same() {
        // The absolute-scale promise: no library state, no randomness, no
        // dependence on what else has been analysed.
        let s = sine(1000.0, 44_100, 2.0, 0.4);
        assert_eq!(analyze(&s, 44_100, 128.0), analyze(&s, 44_100, 128.0));
    }

    #[test]
    fn a_loud_fast_bright_pulse_outscores_a_quiet_slow_sine() {
        // The end-to-end sanity check the anchors exist to produce.
        let quiet = sine(120.0, 44_100, 4.0, 0.03);
        let mut busy = sine(3000.0, 44_100, 4.0, 0.7);
        // Gate it into a pulse so the drive term has something to see.
        for (i, s) in busy.iter_mut().enumerate() {
            if (i / 2000) % 2 == 1 {
                *s *= 0.05;
            }
        }
        let low = to_display(analyze(&quiet, 44_100, 80.0));
        let high = to_display(analyze(&busy, 44_100, 145.0));
        assert!(low <= 3, "quiet slow sine scored {low}");
        assert!(high >= 8, "loud fast bright pulse scored {high}");
    }

    #[test]
    fn display_covers_the_whole_range_without_gaps() {
        // Every integer 1..=10 has to be reachable, or the scale is narrower
        // than the UI claims and rules like `energy = 7` never match.
        let seen: std::collections::BTreeSet<u8> = (0..=100)
            .map(|i| to_display(0.1 + 0.9 * (i as f32 / 100.0)))
            .collect();
        assert_eq!(
            seen.into_iter().collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
    }
}
