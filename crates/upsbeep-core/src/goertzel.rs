/// Returns the fraction of a block's energy found at `freq` (0.0..~1.0).
pub fn tone_ratio(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let w = 2.0 * std::f32::consts::PI * freq / sample_rate;
    let coeff = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    let mut energy = 0.0f32;
    for &x in samples {
        let s0 = x + coeff * s1 - s2;
        s2 = s1;
        s1 = s0;
        energy += x * x;
    }
    if energy < 1e-9 {
        return 0.0;
    }
    let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
    // For a pure tone, power ≈ (N/2)^2 * amp^2 and energy ≈ N * amp^2 / 2.
    (power * 2.0 / (samples.len() as f32 * energy)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin())
            .collect()
    }

    #[test]
    fn detects_matching_tone() {
        assert!(tone_ratio(&sine(2800.0, 8000.0, 800), 2800.0, 8000.0) > 0.9);
    }

    #[test]
    fn rejects_other_tone_and_silence() {
        assert!(tone_ratio(&sine(1000.0, 8000.0, 800), 2800.0, 8000.0) < 0.1);
        assert_eq!(tone_ratio(&vec![0.0; 800], 2800.0, 8000.0), 0.0);
    }
}
