use crate::goertzel::tone_ratio;

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
    counts.sort_by(|a, b| b.1.cmp(&a.1));
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
}
