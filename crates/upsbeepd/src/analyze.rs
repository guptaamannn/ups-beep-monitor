//! `--analyze`: record for a while and report the beep tone, level and timing.

use crate::audio::{self, BLOCK, Msg};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use upsbeep_core::analyze::{dominant_frequency, peak_frequency};
use upsbeep_core::config::Config;
use upsbeep_core::gate::BeepGate;
use upsbeep_core::goertzel::tone_ratio;

const LO: f32 = 500.0;
const HI: f32 = 4000.0;
const STEP: f32 = 20.0;

pub fn run(cfg: &Config, secs: u64) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Msg>(64);
    let capture = audio::open(cfg.device.as_deref(), tx)?;
    let rate = capture.sample_rate as f32;
    eprintln!("recording {secs}s: make the UPS beep (press its test button, or pull the plug)...");

    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut blocks: Vec<Vec<f32>> = Vec::new();
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Block(b)) => blocks.push(b),
            Ok(Msg::StreamError(e)) => return Err(format!("audio stream error: {e}")),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("audio channel closed".into()),
        }
    }

    let peaks: Vec<(f32, f32)> = blocks
        .iter()
        .filter_map(|b| peak_frequency(b, rate, LO, HI, STEP))
        .collect();
    let Some(freq) = dominant_frequency(&peaks, 0.5, STEP) else {
        println!(
            "no steady tone found between {LO} and {HI} Hz. Is the mic working and the UPS close enough?"
        );
        return Ok(());
    };
    println!("dominant tone: about {freq} Hz");

    // Replay the recording through the same gate the daemon uses.
    let mut gate = BeepGate::default();
    let t0 = Instant::now();
    let mut beeps: Vec<f32> = Vec::new();
    let mut ratios: Vec<f32> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        let r = tone_ratio(b, freq, rate);
        ratios.push(r);
        let now = t0 + BLOCK * i as u32;
        if gate.update(now, r >= cfg.threshold) {
            beeps.push(BLOCK.as_secs_f32() * i as f32);
        }
    }
    println!(
        "beeps detected with threshold {}: {}",
        cfg.threshold,
        beeps.len()
    );
    for w in beeps.windows(2) {
        println!("  gap {:.1}s", w[1] - w[0]);
    }
    let max_ratio = ratios.iter().cloned().fold(0.0, f32::max);
    println!(
        "strongest block ratio: {max_ratio:.2} (keep `threshold` well below this and above background noise)"
    );
    println!("set in config: tone_hz = {freq}");
    Ok(())
}
