use crate::gate::BeepGate;
use crate::goertzel::tone_ratio;
use std::time::{Duration, Instant};

/// Frequency range and resolution searched for the UPS tone.
pub const LO: f32 = 500.0;
pub const HI: f32 = 4000.0;
pub const STEP: f32 = 20.0;

const MIN_RMS: f32 = 0.005;

/// Finds the strongest tone in `lo..=hi` Hz. Returns `(freq, ratio)`, or `None` for near-silence.
pub fn peak_frequency(
    samples: &[f32],
    rate: f32,
    lo: f32,
    hi: f32,
    step: f32,
) -> Option<(f32, f32)> {
    if samples.is_empty() {
        return None;
    }
    let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    if rms < MIN_RMS {
        return None;
    }
    let mut best = (lo, 0.0f32);
    let mut f = lo;
    while f <= hi {
        let r = tone_ratio(samples, f, rate);
        if r > best.1 {
            best = (f, r);
        }
        f += step;
    }
    Some(best)
}

/// Counts strongly tonal blocks per frequency (rounded to `step`), most common first.
pub fn frequency_histogram(peaks: &[(f32, f32)], min_ratio: f32, step: f32) -> Vec<(f32, usize)> {
    let mut counts: Vec<(i64, usize)> = Vec::new();
    for &(f, r) in peaks {
        if r < min_ratio {
            continue;
        }
        let bin = (f / step).round() as i64;
        match counts.iter_mut().find(|(b, _)| *b == bin) {
            Some((_, n)) => *n += 1,
            None => counts.push((bin, 1)),
        }
    }
    counts.sort_by_key(|c| std::cmp::Reverse(c.1));
    counts
        .into_iter()
        .map(|(b, n)| (b as f32 * step, n))
        .collect()
}

/// The most common frequency among strongly tonal blocks.
pub fn dominant_frequency(peaks: &[(f32, f32)], min_ratio: f32, step: f32) -> Option<f32> {
    frequency_histogram(peaks, min_ratio, step)
        .first()
        .map(|&(f, _)| f)
}

/// What a calibration recording found.
pub struct Report {
    pub tone_hz: f32,
    /// Most common tonal frequencies with their block counts.
    pub top: Vec<(f32, usize)>,
    /// Beep start times in seconds from the start of the recording, at `suggested_threshold`.
    pub beeps: Vec<f32>,
    /// Strongest tone ratio seen at `tone_hz`.
    pub max_ratio: f32,
    pub suggested_threshold: f32,
}

/// Analyzes a recording split into equal blocks. `None` if no steady tone was heard.
pub fn analyze_blocks(
    blocks: &[Vec<f32>],
    rate: f32,
    block_len: Duration,
    gate_min_on: Duration,
    gate_min_gap: Duration,
) -> Option<Report> {
    let peaks: Vec<(f32, f32)> = blocks
        .iter()
        .filter_map(|b| peak_frequency(b, rate, LO, HI, STEP))
        .collect();
    let mut top = frequency_histogram(&peaks, 0.5, STEP);
    let tone_hz = top.first()?.0;
    top.truncate(5);
    let ratios: Vec<f32> = blocks
        .iter()
        .map(|b| tone_ratio(b, tone_hz, rate))
        .collect();
    let max_ratio = ratios.iter().cloned().fold(0.0, f32::max);
    let suggested_threshold = (max_ratio * 0.55).clamp(0.15, 0.6);

    let mut gate = BeepGate::new(gate_min_on, gate_min_gap);
    let t0 = Instant::now();
    let beeps = ratios
        .iter()
        .enumerate()
        .filter(|(i, r)| gate.update(t0 + block_len * *i as u32, **r >= suggested_threshold))
        .map(|(i, _)| block_len.as_secs_f32() * i as f32)
        .collect();
    Some(Report {
        tone_hz,
        top,
        beeps,
        max_ratio,
        suggested_threshold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin())
            .collect()
    }

    #[test]
    fn finds_peak() {
        let (f, r) =
            peak_frequency(&sine(3200.0, 48000.0, 2400), 48000.0, 500.0, 4000.0, 20.0).unwrap();
        assert!((f - 3200.0).abs() <= 20.0, "got {f}");
        assert!(r > 0.8);
    }

    #[test]
    fn silence_has_no_peak() {
        assert!(peak_frequency(&[0.0; 2400], 48000.0, 500.0, 4000.0, 20.0).is_none());
    }

    #[test]
    fn dominant_ignores_weak_blocks() {
        let peaks = [
            (3200.0, 0.9),
            (3200.0, 0.8),
            (1000.0, 0.2),
            (1000.0, 0.2),
            (1000.0, 0.2),
        ];
        assert_eq!(dominant_frequency(&peaks, 0.5, 20.0), Some(3200.0));
    }

    #[test]
    fn analyzes_a_recording_with_beeps() {
        let rate = 48000.0;
        let block = 2400; // 50 ms
        let tone = sine(1820.0, rate, block);
        let silence = vec![0.0f32; block];
        // 3 beeps of 6 blocks, 14 blocks (0.7 s) apart.
        let mut blocks = Vec::new();
        for _ in 0..3 {
            blocks.extend(std::iter::repeat_n(tone.clone(), 6));
            blocks.extend(std::iter::repeat_n(silence.clone(), 14));
        }
        let r = analyze_blocks(
            &blocks,
            rate,
            Duration::from_millis(50),
            Duration::from_millis(100),
            Duration::from_millis(200),
        )
        .unwrap();
        assert!((r.tone_hz - 1820.0).abs() <= 20.0, "{}", r.tone_hz);
        assert_eq!(r.beeps.len(), 3);
        assert!((r.beeps[1] - r.beeps[0] - 1.0).abs() < 0.11);
        assert!(r.max_ratio > 0.8);
        assert!((0.15..=0.6).contains(&r.suggested_threshold));
    }

    #[test]
    fn silent_recording_has_no_report() {
        let blocks = vec![vec![0.0f32; 2400]; 20];
        let ms = Duration::from_millis;
        assert!(analyze_blocks(&blocks, 48000.0, ms(50), ms(100), ms(200)).is_none());
    }
}
