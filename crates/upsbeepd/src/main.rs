//! Headless daemon: listens to the mic, detects UPS beeps, shuts the PC down.

mod analyze;
mod control;
mod events;
mod shutdown;

use control::Cmd;
use events::Events;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use upsbeep_audio::{self as audio, Capture, Msg};
use upsbeep_core::config::{self, Config};
use upsbeep_core::detector::{Action, State};
use upsbeep_core::pipeline::Pipeline;
use upsbeep_core::watchdog::{MicEvent, MicWatchdog};

const RETRY_EVERY: Duration = Duration::from_secs(10);
const RELOAD_CHECK_EVERY: Duration = Duration::from_secs(2);
const MIC_SILENT_AFTER: Duration = Duration::from_secs(600);
const STATUS_EVERY: Duration = Duration::from_secs(1);
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
    let mut pipeline = Pipeline::new(&cfg);
    let mut watchdog = MicWatchdog::new(MIC_SILENT_AFTER, MIC_DOWN_AFTER);
    let mut was_on_battery = false;
    let mut shutdown_done = false;
    let mut last_countdown = Instant::now();
    let mut config_mtime = mtime(&path);
    let mut next_reload_check = Instant::now() + RELOAD_CHECK_EVERY;

    let status = Arc::new(Mutex::new(String::new()));
    let ctl_rx = match config::socket_path() {
        None => {
            ev.info("control socket disabled: XDG_RUNTIME_DIR is not set");
            None
        }
        Some(p) => match control::start(&p, status.clone()) {
            Ok(rx) => Some(rx),
            Err(e) => {
                ev.info(&format!("control socket disabled: {e}"));
                None
            }
        },
    };
    let mut paused_until: Option<Instant> = None;
    let mut cancelled = false;
    let mut suppress_logged = false;
    let mut last_beep: Option<Instant> = None;
    let mut last_peak = 0.0f32;
    let mut next_status = Instant::now();

    loop {
        let now = Instant::now();

        if let Some(rx) = &ctl_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Cmd::Cancel => {
                        cancelled = true;
                        suppress_logged = false;
                        ev.alert("shutdown cancelled for this outage (by upsbeepctl)");
                    }
                    Cmd::Pause(d) => {
                        paused_until = Some(now + d);
                        suppress_logged = false;
                        ev.alert(&format!(
                            "shutdown paused for {} min (by upsbeepctl)",
                            d.as_secs() / 60
                        ));
                    }
                    Cmd::Resume => {
                        paused_until = None;
                        cancelled = false;
                        suppress_logged = false;
                        ev.info("shutdown resumed (by upsbeepctl)");
                    }
                }
            }
        }

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
                        pipeline.reconfigure(&new);
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

        let mut step = None;
        let mut mic_event = None;
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Block(block)) => {
                if let Some(c) = &capture {
                    step = Some(pipeline.block(Instant::now(), &block, c.sample_rate as f32));
                    let peak = block.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                    last_peak = peak;
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
        let step = step.unwrap_or_else(|| pipeline.tick(now));
        if step.beep {
            ev.info("beep");
            last_beep = Some(now);
        }
        let suppressed = cancelled || paused_until.is_some_and(|t| now < t);
        if step.action == Action::Shutdown && !shutdown_done {
            if suppressed {
                if !suppress_logged {
                    ev.alert("shutdown time reached but it is cancelled or paused; staying up");
                    suppress_logged = true;
                }
            } else {
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
        }

        match pipeline.state() {
            State::OnBattery { since } => {
                if !was_on_battery {
                    ev.alert(&format!(
                        "UPS is beeping: assuming a power outage; shutting down in {}s unless mains returns",
                        cfg.shutdown_after_secs
                    ));
                    was_on_battery = true;
                    last_countdown = now;
                } else if !shutdown_done
                    && !suppressed
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
                    cancelled = false;
                    suppress_logged = false;
                }
            }
        }

        if now >= next_status {
            next_status = now + STATUS_EVERY;
            let view = StatusView {
                cfg: &cfg,
                state: pipeline.state(),
                now,
                last_beep,
                peak: last_peak,
                mic_ok: capture.is_some(),
                paused_until,
                cancelled,
                shutdown_done,
            };
            if let Ok(mut st) = status.lock() {
                *st = view.render();
            }
        }
    }
}

struct StatusView<'a> {
    cfg: &'a Config,
    state: State,
    now: Instant,
    last_beep: Option<Instant>,
    peak: f32,
    mic_ok: bool,
    paused_until: Option<Instant>,
    cancelled: bool,
    shutdown_done: bool,
}

impl StatusView<'_> {
    fn render(&self) -> String {
        let cfg = self.cfg;
        let (state, shutdown) = match self.state {
            State::Mains => ("mains OK".to_string(), "none pending".to_string()),
            State::OnBattery { since } => {
                let elapsed = self.now.duration_since(since).as_secs();
                let left = cfg.shutdown_after_secs.saturating_sub(elapsed);
                let shutdown = if self.cancelled {
                    "CANCELLED for this outage".to_string()
                } else if let Some(t) = self.paused_until.filter(|t| self.now < *t) {
                    let mins = (t - self.now).as_secs().div_ceil(60);
                    format!("PAUSED for ~{mins} more min (would fire in {left}s)")
                } else if self.shutdown_done {
                    "dry-run: already reported".to_string()
                } else {
                    format!("in {left}s")
                };
                (format!("ON BATTERY (beeping for {elapsed}s)"), shutdown)
            }
        };
        let beep = self.last_beep.map_or("never".to_string(), |t| {
            format!("{}s ago", self.now.duration_since(t).as_secs())
        });
        let mic = if self.mic_ok {
            format!("ok (peak {:.3})", self.peak)
        } else {
            "NOT RECEIVING AUDIO: the UPS is not being monitored".to_string()
        };
        let paused = match self.paused_until.filter(|t| self.now < *t) {
            Some(t) => format!("paused ~{} more min", (t - self.now).as_secs().div_ceil(60)),
            None => "active".to_string(),
        };
        format!(
            "state:    {state}\nshutdown: {shutdown}\nlast beep: {beep}\nmic:      {mic}\nmonitor:  {paused}, {}\nconfig:   tone {} Hz, threshold {}, mains-back after {}s silence, shutdown {}s after first beep\n",
            if cfg.dry_run { "dry-run" } else { "LIVE" },
            cfg.tone_hz,
            cfg.threshold,
            cfg.beep_timeout_secs,
            cfg.shutdown_after_secs
        )
    }
}
