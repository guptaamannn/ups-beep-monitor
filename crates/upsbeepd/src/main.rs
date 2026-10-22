//! Headless daemon: listens to the mic, detects UPS beeps, shuts the PC down.

mod analyze;
mod audio;
mod events;
mod shutdown;

use audio::{Capture, Msg};
use events::Events;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};
use upsbeep_core::config::{self, Config};
use upsbeep_core::detector::{Action, Detector, State};
use upsbeep_core::gate::BeepGate;
use upsbeep_core::goertzel::tone_ratio;
use upsbeep_core::watchdog::{MicEvent, MicWatchdog};

const RETRY_EVERY: Duration = Duration::from_secs(10);
const RELOAD_CHECK_EVERY: Duration = Duration::from_secs(2);
const MIC_SILENT_AFTER: Duration = Duration::from_secs(600);
const MIC_DOWN_AFTER: Duration = Duration::from_secs(60);

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
    Events::new(&cfg).info(&format!(
        "started: tone {} Hz, threshold {}, dry_run {}",
        cfg.tone_hz, cfg.threshold, cfg.dry_run
    ));
    monitor(cfg, path)
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn monitor(mut cfg: Config, path: PathBuf) -> Result<(), String> {
    let (tx, rx) = mpsc::sync_channel::<Msg>(64);
    let mut capture: Option<Capture> = None;
    let mut next_retry = Instant::now();

    let mut ev = Events::new(&cfg);
    let mut gate = BeepGate::default();
    gate.set_timing(
        Duration::from_millis(cfg.gate_min_on_ms),
        Duration::from_millis(cfg.gate_min_gap_ms),
    );
    let mut detector = Detector::new(
        Duration::from_secs(cfg.beep_timeout_secs),
        Duration::from_secs(cfg.shutdown_after_secs),
    );
    let mut watchdog = MicWatchdog::new(MIC_SILENT_AFTER, MIC_DOWN_AFTER);
    let mut was_on_battery = false;
    let mut shutdown_done = false;
    let mut last_countdown = Instant::now();
    let mut config_mtime = mtime(&path);
    let mut next_reload_check = Instant::now() + RELOAD_CHECK_EVERY;

    loop {
        let now = Instant::now();

        if now >= next_reload_check {
            next_reload_check = now + RELOAD_CHECK_EVERY;
            let latest = mtime(&path);
            if latest.is_some() && latest != config_mtime {
                config_mtime = latest;
                match Config::load(&path) {
                    Ok(new) => {
                        if new.device != cfg.device {
                            capture = None;
                            next_retry = now;
                        }
                        gate.set_timing(
                            Duration::from_millis(new.gate_min_on_ms),
                            Duration::from_millis(new.gate_min_gap_ms),
                        );
                        detector.set_durations(
                            Duration::from_secs(new.beep_timeout_secs),
                            Duration::from_secs(new.shutdown_after_secs),
                        );
                        cfg = new;
                        ev = Events::new(&cfg);
                        ev.info(&format!(
                            "config reloaded: tone {} Hz, threshold {}, dry_run {}",
                            cfg.tone_hz, cfg.threshold, cfg.dry_run
                        ));
                    }
                    Err(e) => ev.info(&format!("config reload failed, keeping old settings: {e}")),
                }
            }
        }

        if capture.is_none() && now >= next_retry {
            match audio::open(cfg.device.as_deref(), tx.clone()) {
                Ok(c) => capture = Some(c),
                Err(e) => {
                    ev.info(&format!(
                        "cannot open audio input: {e}; retrying in {}s",
                        RETRY_EVERY.as_secs()
                    ));
                    next_retry = now + RETRY_EVERY;
                }
            }
        }

        let mut beep = false;
        let mut mic_event = None;
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Block(block)) => {
                if let Some(c) = &capture {
                    let ratio = tone_ratio(&block, cfg.tone_hz, c.sample_rate as f32);
                    beep = gate.update(Instant::now(), ratio >= cfg.threshold);
                    let peak = block.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                    mic_event = watchdog.block(Instant::now(), peak);
                }
            }
            Ok(Msg::StreamError(e)) => {
                ev.info(&format!("audio stream error: {e}; reopening"));
                capture = None;
                next_retry = Instant::now() + RETRY_EVERY;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("audio channel closed".into()),
        }
        if capture.is_none() {
            mic_event = mic_event.or(watchdog.down(Instant::now()));
        }
        match mic_event {
            Some(MicEvent::Silent) => ev.alert(
                "microphone has been silent for 10 minutes (muted or unplugged?); the UPS is NOT being monitored",
            ),
            Some(MicEvent::Down) => ev.alert(
                "no audio input available for 60 s; the UPS is NOT being monitored",
            ),
            Some(MicEvent::Recovered) => ev.alert("microphone is working again"),
            None => {}
        }

        let now = Instant::now();
        if beep {
            ev.info("beep");
        }
        if detector.update(now, beep) == Action::Shutdown && !shutdown_done {
            ev.alert(&format!(
                "on battery for {}s, shutting down now",
                cfg.shutdown_after_secs
            ));
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
                    ev.alert(&format!(
                        "UPS is beeping: assuming a power outage; shutting down in {}s unless mains returns",
                        cfg.shutdown_after_secs
                    ));
                    was_on_battery = true;
                    last_countdown = now;
                } else if !shutdown_done
                    && now.duration_since(last_countdown) >= Duration::from_secs(30)
                {
                    let left = cfg
                        .shutdown_after_secs
                        .saturating_sub(now.duration_since(since).as_secs());
                    ev.info(&format!("on battery, shutdown in {left}s"));
                    last_countdown = now;
                }
            }
            State::Mains => {
                if was_on_battery {
                    ev.alert("beeps stopped: back on mains, shutdown cancelled");
                    was_on_battery = false;
                    shutdown_done = false;
                }
            }
        }
    }
}
