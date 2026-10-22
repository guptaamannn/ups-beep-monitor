//! `--analyze`: record for a while and report the beep tone, level and timing.

use std::sync::mpsc;
use std::time::{Duration, Instant};
use upsbeep_audio::{self as audio, BLOCK, Msg};
use upsbeep_core::analyze::{HI, LO, analyze_blocks};
use upsbeep_core::config::Config;

pub fn run(cfg: &Config, secs: u64) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Msg>(64);
    let capture = audio::open(cfg.device.as_deref(), tx)?;
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

    let report = analyze_blocks(
        &blocks,
        capture.sample_rate as f32,
        BLOCK,
        Duration::from_millis(cfg.gate_min_on_ms),
        Duration::from_millis(cfg.gate_min_gap_ms),
    );
    let Some(r) = report else {
        println!(
            "no steady tone found between {LO} and {HI} Hz. Is the mic working and the UPS close enough?"
        );
        return Ok(());
    };
    println!("dominant tone: about {} Hz", r.tone_hz);
    let top: Vec<String> = r.top.iter().map(|(f, n)| format!("{f} Hz x{n}")).collect();
    println!("top tones (blocks): {}", top.join(", "));
    println!(
        "beeps detected at the suggested threshold {:.2}: {}",
        r.suggested_threshold,
        r.beeps.len()
    );
    for w in r.beeps.windows(2) {
        println!("  gap {:.1}s", w[1] - w[0]);
    }
    println!("strongest block ratio: {:.2}", r.max_ratio);
    println!(
        "set in config: tone_hz = {}, threshold = {:.2}",
        r.tone_hz, r.suggested_threshold
    );
    Ok(())
}
