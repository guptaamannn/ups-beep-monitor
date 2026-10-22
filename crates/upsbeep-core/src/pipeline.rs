//! The full detection chain, from audio blocks to a shutdown decision, with no I/O.
//!
//! The daemon feeds it real microphone blocks; tests feed it synthetic audio and a virtual clock.

use crate::config::Config;
use crate::detector::{Action, Detector, State};
use crate::gate::BeepGate;
use crate::goertzel::tone_ratio;
use std::time::{Duration, Instant};

/// Result of one update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// A beep was completed by this block.
    pub beep: bool,
    pub action: Action,
}

pub struct Pipeline {
    tone_hz: f32,
    threshold: f32,
    gate: BeepGate,
    detector: Detector,
}

impl Pipeline {
    pub fn new(cfg: &Config) -> Self {
        let mut p = Self {
            tone_hz: cfg.tone_hz,
            threshold: cfg.threshold,
            gate: BeepGate::default(),
            detector: Detector::new(Duration::ZERO, Duration::ZERO),
        };
        p.reconfigure(cfg);
        p
    }

    /// Applies new settings without losing the current outage state.
    pub fn reconfigure(&mut self, cfg: &Config) {
        self.tone_hz = cfg.tone_hz;
        self.threshold = cfg.threshold;
        self.gate.set_timing(
            Duration::from_millis(cfg.gate_min_on_ms),
            Duration::from_millis(cfg.gate_min_gap_ms),
        );
        self.detector.set_durations(
            Duration::from_secs(cfg.beep_timeout_secs),
            Duration::from_secs(cfg.shutdown_after_secs),
        );
    }

    /// Processes one block of mono samples captured at `rate` Hz.
    pub fn block(&mut self, now: Instant, samples: &[f32], rate: f32) -> Step {
        let active = tone_ratio(samples, self.tone_hz, rate) >= self.threshold;
        let beep = self.gate.update(now, active);
        Step {
            beep,
            action: self.detector.update(now, beep),
        }
    }

    /// Advances time when no audio arrived, so silence timeouts still fire.
    pub fn tick(&mut self, now: Instant) -> Step {
        Step {
            beep: false,
            action: self.detector.update(now, false),
        }
    }

    pub fn state(&self) -> State {
        self.detector.state()
    }
}
