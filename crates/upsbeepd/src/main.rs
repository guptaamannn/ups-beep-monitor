//! Headless daemon: listens to the mic, detects UPS beeps, shuts the PC down.

mod analyze;
mod audio;
mod shutdown;

use audio::{Capture, Msg};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};
use upsbeep_core::config::{self, Config};
use upsbeep_core::detector::{Action, Detector, State};
use upsbeep_core::gate::BeepGate;
use upsbeep_core::goertzel::tone_ratio;

const RETRY_EVERY: Duration = Duration::from_secs(10);

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut path: Option<PathBuf> = None;
    let mut write_default = false;
    let mut analyze_secs: Option<u64> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => path = Some(args.next().ok_or("--config needs a path")?.into()),
            "--write-default-config" => write_default = true,
            "--list-devices" => return audio::list_devices(),
            "--analyze" => {
                let secs = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or("--analyze needs seconds")?;
                analyze_secs = Some(secs);
            }
            "--help" | "-h" => {
                println!(
                    "usage: upsbeepd [--config PATH] [--list-devices] [--analyze SECS] [--write-default-config]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let path = path
        .or_else(config::default_path)
        .ok_or("cannot determine config path; use --config")?;

    if write_default {
        Config::default().save(&path)?;
        println!("wrote {}", path.display());
        return Ok(());
    }

    let cfg = if path.exists() {
        Config::load(&path)?
    } else {
        eprintln!("no config at {}, using defaults", path.display());
        Config::default()
    };
    if let Some(secs) = analyze_secs {
        return analyze::run(&cfg, secs);
    }
    eprintln!(
        "tone {} Hz, threshold {}, dry_run {}",
        cfg.tone_hz, cfg.threshold, cfg.dry_run
    );
    monitor(&cfg)
}

fn monitor(cfg: &Config) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Msg>(64);
    let mut capture: Option<Capture> = None;
    let mut next_retry = Instant::now();

    let mut gate = BeepGate::default();
    let mut detector = Detector::new(
        Duration::from_secs(cfg.beep_timeout_secs),
        Duration::from_secs(cfg.shutdown_after_secs),
    );
    let mut was_on_battery = false;
    let mut shutdown_done = false;
    let mut last_countdown = Instant::now();

    loop {
        if capture.is_none() && Instant::now() >= next_retry {
            match audio::open(cfg.device.as_deref(), tx.clone()) {
                Ok(c) => capture = Some(c),
                Err(e) => {
                    eprintln!(
                        "cannot open audio input: {e}; retrying in {}s",
                        RETRY_EVERY.as_secs()
                    );
                    next_retry = Instant::now() + RETRY_EVERY;
                }
            }
        }

        let mut beep = false;
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Block(block)) => {
                if let Some(c) = &capture {
                    let ratio = tone_ratio(&block, cfg.tone_hz, c.sample_rate as f32);
                    beep = gate.update(Instant::now(), ratio >= cfg.threshold);
                }
            }
            Ok(Msg::StreamError(e)) => {
                eprintln!("audio stream error: {e}; reopening");
                capture = None;
                next_retry = Instant::now() + RETRY_EVERY;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("audio channel closed".into()),
        }

        let now = Instant::now();
        if beep {
            eprintln!("beep");
        }
        if detector.update(now, beep) == Action::Shutdown && !shutdown_done {
            eprintln!("on battery for {}s, shutting down", cfg.shutdown_after_secs);
            shutdown::power_off(cfg.dry_run);
            if !cfg.dry_run {
                return Ok(());
            }
            // Dry run: report once per outage, not on every loop pass.
            shutdown_done = true;
        }

        match detector.state() {
            State::OnBattery { since } => {
                if !was_on_battery {
                    eprintln!("UPS beeping: assuming power outage");
                    was_on_battery = true;
                    last_countdown = now;
                } else if now.duration_since(last_countdown) >= Duration::from_secs(30) {
                    let left = cfg
                        .shutdown_after_secs
                        .saturating_sub(now.duration_since(since).as_secs());
                    eprintln!("on battery, shutdown in {left}s");
                    last_countdown = now;
                }
            }
            State::Mains => {
                if was_on_battery {
                    eprintln!("beeps stopped: back on mains");
                    was_on_battery = false;
                    shutdown_done = false;
                }
            }
        }
    }
}
