//! End-to-end tests of the detection chain with synthetic audio and a virtual clock.
//!
//! The beep pattern follows the UPS manual: two quick beeps once it starts beeping, then one
//! beep every 60 seconds.

use std::f32::consts::PI;
use std::time::{Duration, Instant};
use upsbeep_core::config::Config;
use upsbeep_core::detector::{Action, State};
use upsbeep_core::pipeline::Pipeline;

const RATE: f32 = 48_000.0;
const BLOCK_SAMPLES: usize = 2400; // 50 ms
const BLOCK_SECS: f32 = 0.05;
const BEEP_SECS: f32 = 0.4;
const TONE: f32 = 1820.0;

fn config() -> Config {
    Config {
        tone_hz: TONE,
        threshold: 0.35,
        beep_timeout_secs: 90,
        shutdown_after_secs: 300,
        ..Config::default()
    }
}

/// Start times (seconds) of the UPS beeps for an outage whose first beep is at 0.
fn ups_beeps(until: f32) -> Vec<f32> {
    let mut times = vec![0.0, 0.7];
    let mut t = 60.0;
    while t < until {
        times.push(t);
        t += 60.0;
    }
    times
}

fn tone(freq: f32, block: usize) -> Vec<f32> {
    (0..BLOCK_SAMPLES)
        .map(|i| {
            let n = block * BLOCK_SAMPLES + i;
            0.5 * (2.0 * PI * freq * n as f32 / RATE).sin()
        })
        .collect()
}

/// Deterministic pseudo-random noise in -amp..amp.
fn noise(seed: &mut u32, amp: f32) -> Vec<f32> {
    (0..BLOCK_SAMPLES)
        .map(|_| {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * amp
        })
        .collect()
}

struct Outcome {
    /// Seconds at which the pipeline first asked for a shutdown.
    shutdown_at: Option<f32>,
    /// Seconds at which each beep was reported.
    beeps: Vec<f32>,
    /// State sampled once per simulated second.
    on_battery: Vec<bool>,
}

/// Runs `secs` of audio produced by `audio(block_index, time_secs)`.
fn run(cfg: &Config, secs: f32, mut audio: impl FnMut(usize, f32) -> Vec<f32>) -> Outcome {
    let mut pipeline = Pipeline::new(cfg);
    let t0 = Instant::now();
    let blocks = (secs / BLOCK_SECS) as usize;
    let mut out = Outcome {
        shutdown_at: None,
        beeps: Vec::new(),
        on_battery: Vec::new(),
    };
    for i in 0..blocks {
        let t = i as f32 * BLOCK_SECS;
        let now = t0 + Duration::from_secs_f32(t);
        let step = pipeline.block(now, &audio(i, t), RATE);
        if step.beep {
            out.beeps.push(t);
        }
        if step.action == Action::Shutdown && out.shutdown_at.is_none() {
            out.shutdown_at = Some(t);
        }
        if i % 20 == 0 {
            out.on_battery
                .push(matches!(pipeline.state(), State::OnBattery { .. }));
        }
    }
    out
}

/// Audio containing the tone at each beep start time, silence elsewhere.
fn beeps_audio(times: Vec<f32>) -> impl FnMut(usize, f32) -> Vec<f32> {
    move |i, t| {
        if times.iter().any(|&b| t >= b && t < b + BEEP_SECS) {
            tone(TONE, i)
        } else {
            vec![0.0; BLOCK_SAMPLES]
        }
    }
}

#[test]
fn long_outage_shuts_down_300s_after_the_first_beep() {
    let out = run(&config(), 400.0, beeps_audio(ups_beeps(400.0)));
    let at = out.shutdown_at.expect("no shutdown requested");
    assert!((300.0..301.0).contains(&at), "shutdown at {at}s");
    // Two quick beeps, then one per minute: every beep must be heard.
    assert!(out.beeps.len() >= 7, "heard {} beeps", out.beeps.len());
}

#[test]
fn mains_return_is_noticed_after_the_beeps_stop() {
    // Beeps up to the one at 120 s, then silence (mains is back).
    let out = run(&config(), 400.0, beeps_audio(vec![0.0, 0.7, 60.0, 120.0]));
    assert_eq!(out.shutdown_at, None);
    assert!(out.on_battery[100], "should be on battery at 100 s");
    assert!(
        !out.on_battery[120 + 92..].iter().any(|b| *b),
        "mains should be back about 90 s after the last beep"
    );
    assert!(out.on_battery[120 + 80], "still waiting at 80 s of silence");
}

#[test]
fn a_missed_beep_is_tolerated_when_the_timeout_spans_two_intervals() {
    let cfg = Config {
        beep_timeout_secs: 130,
        ..config()
    };
    // The beep at 120 s is never heard.
    let mut beeps = ups_beeps(400.0);
    beeps.retain(|&t| t != 120.0);
    let out = run(&cfg, 400.0, beeps_audio(beeps));
    let at = out
        .shutdown_at
        .expect("outage was lost after a missed beep");
    assert!((300.0..301.0).contains(&at), "shutdown at {at}s");
}

#[test]
fn other_sounds_do_not_look_like_an_outage() {
    let mut seed = 1;
    let out = run(&config(), 600.0, |i, t| {
        let sec = t as u32;
        match sec % 10 {
            // A different tone, a click, and broadband noise.
            0..=1 => tone(2500.0, i),
            3 if t.fract() < BLOCK_SECS => tone(TONE, i), // a single 50 ms blip at the right tone
            5..=7 => noise(&mut seed, 0.3),
            _ => noise(&mut seed, 0.001),
        }
    });
    assert!(out.beeps.is_empty(), "false beeps at {:?}", out.beeps);
    assert_eq!(out.shutdown_at, None);
    assert!(out.on_battery.iter().all(|b| !b));
}

#[test]
fn reconfigure_keeps_the_outage_state() {
    let cfg = config();
    let mut pipeline = Pipeline::new(&cfg);
    let t0 = Instant::now();
    for i in 0..10 {
        let now = t0 + Duration::from_secs_f32(i as f32 * BLOCK_SECS);
        pipeline.block(now, &tone(TONE, i), RATE);
    }
    assert!(matches!(pipeline.state(), State::OnBattery { .. }));
    pipeline.reconfigure(&Config {
        shutdown_after_secs: 10,
        ..cfg
    });
    assert!(matches!(pipeline.state(), State::OnBattery { .. }));
    let later = t0 + Duration::from_secs(11);
    assert_eq!(pipeline.tick(later).action, Action::Shutdown);
}
