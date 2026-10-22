//! Live microphone view and calibration: level meter, spectrum, and record-and-analyze.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use upsbeep_audio::{self as audio, BLOCK, Capture, Msg};
use upsbeep_core::analyze::{HI, LO, Report, STEP, analyze_blocks};
use upsbeep_core::config::Config;
use upsbeep_core::goertzel::tone_ratio;

const BARS: usize = ((HI - LO) / STEP) as usize + 1;

struct Input {
    capture: Capture,
    rx: Receiver<Msg>,
}

struct Recording {
    blocks: Vec<Vec<f32>>,
    started: Instant,
    total: Duration,
}

pub struct CalibratePane {
    input: Option<Input>,
    tried: bool,
    error: Option<String>,
    /// Peak amplitude with a slow decay, for the meter.
    level: f32,
    /// Tone strength per frequency bar, with a slow decay.
    spectrum: Vec<f32>,
    beep_until: Option<Instant>,
    record_secs: u64,
    recording: Option<Recording>,
    report: Option<Result<Report, String>>,
}

impl CalibratePane {
    pub fn new() -> Self {
        Self {
            input: None,
            tried: false,
            error: None,
            level: 0.0,
            spectrum: vec![0.0; BARS],
            beep_until: None,
            record_secs: 70,
            recording: None,
            report: None,
        }
    }

    /// Opens the microphone while the tab is showing, and releases it otherwise.
    pub fn set_active(&mut self, active: bool, cfg: &Config) {
        if !active {
            self.input = None;
            self.tried = false;
            self.recording = None;
            return;
        }
        if self.input.is_none() && !self.tried {
            self.tried = true;
            let (tx, rx) = mpsc::sync_channel(64);
            match audio::open(cfg.device.as_deref(), tx) {
                Ok(capture) => {
                    self.error = None;
                    self.input = Some(Input { capture, rx });
                }
                Err(e) => self.error = Some(e),
            }
        }
    }

    fn drain(&mut self, cfg: &Config) {
        let Some(input) = &self.input else { return };
        let rate = input.capture.sample_rate as f32;
        let mut stream_error = None;
        while let Ok(msg) = input.rx.try_recv() {
            let block = match msg {
                Msg::Block(b) => b,
                Msg::StreamError(e) => {
                    stream_error = Some(e);
                    break;
                }
            };
            let peak = block.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            self.level = peak.max(self.level * 0.9);
            for (i, bar) in self.spectrum.iter_mut().enumerate() {
                let r = tone_ratio(&block, LO + i as f32 * STEP, rate);
                *bar = r.max(*bar * 0.85);
            }
            if peak > 1e-4 && tone_ratio(&block, cfg.tone_hz, rate) >= cfg.threshold {
                self.beep_until = Some(Instant::now() + Duration::from_millis(300));
            }
            if let Some(rec) = &mut self.recording {
                rec.blocks.push(block);
            }
        }
        if let Some(e) = stream_error {
            self.error = Some(format!("audio stream error: {e}"));
            self.input = None;
            self.recording = None;
        }
    }

    fn finish_recording(&mut self, cfg: &Config) {
        let Some(rec) = &self.recording else { return };
        if rec.started.elapsed() < rec.total {
            return;
        }
        let rec = self.recording.take().expect("checked above");
        let rate = self
            .input
            .as_ref()
            .map_or(48000.0, |i| i.capture.sample_rate as f32);
        self.report = Some(
            analyze_blocks(
                &rec.blocks,
                rate,
                BLOCK,
                Duration::from_millis(cfg.gate_min_on_ms),
                Duration::from_millis(cfg.gate_min_gap_ms),
            )
            .ok_or_else(|| {
                format!(
                    "No steady tone found between {LO} and {HI} Hz. Is the microphone working, and close enough to the UPS?"
                )
            }),
        );
    }

    /// Returns `true` when the user asked to save the config.
    pub fn show(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> bool {
        self.drain(cfg);
        self.finish_recording(cfg);
        let mut save = false;

        if let Some(e) = self.error.clone() {
            ui.colored_label(Color32::from_rgb(220, 60, 50), e);
            if ui.button("Retry").clicked() {
                self.tried = false;
            }
            return false;
        }

        // Level meter and beep lamp.
        let db = 20.0 * self.level.max(1e-5).log10();
        ui.horizontal(|ui| {
            ui.add(
                egui::ProgressBar::new(((db + 80.0) / 80.0).clamp(0.0, 1.0))
                    .desired_width(300.0)
                    .text(format!("{db:.0} dB")),
            );
            let on = self.beep_until.is_some_and(|t| Instant::now() < t);
            let (text, color) = if on {
                ("BEEP", Color32::from_rgb(60, 170, 90))
            } else {
                ("quiet", Color32::GRAY)
            };
            ui.label(egui::RichText::new(text).color(color).strong());
        });
        ui.add_space(6.0);

        self.draw_spectrum(ui, cfg);
        if let Some((i, v)) = self
            .spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .filter(|(_, v)| **v > 0.05)
        {
            ui.label(format!(
                "Strongest tone now: {:.0} Hz (strength {v:.2})",
                LO + i as f32 * STEP
            ));
        } else {
            ui.label("Strongest tone now: none");
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label("Make the UPS beep (cut its mains or press its test button), then record:");
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.record_secs, 20..=180).suffix(" s"));
            let idle = self.recording.is_none() && self.input.is_some();
            if ui
                .add_enabled(idle, egui::Button::new("Record and analyze"))
                .clicked()
            {
                self.report = None;
                self.recording = Some(Recording {
                    blocks: Vec::new(),
                    started: Instant::now(),
                    total: Duration::from_secs(self.record_secs),
                });
            }
        });
        if let Some(rec) = &self.recording {
            let done = rec.started.elapsed().as_secs_f32() / rec.total.as_secs_f32();
            ui.add(egui::ProgressBar::new(done.min(1.0)).text("recording..."));
        }

        match &self.report {
            Some(Ok(r)) => {
                ui.add_space(6.0);
                ui.label(format!("Tone: {:.0} Hz", r.tone_hz));
                let top: Vec<String> = r
                    .top
                    .iter()
                    .map(|(f, n)| format!("{f:.0} Hz x{n}"))
                    .collect();
                ui.label(format!("Strongest tones (blocks): {}", top.join(", ")));
                ui.label(format!("Strongest signal: {:.2}", r.max_ratio));
                ui.label(format!("Suggested threshold: {:.2}", r.suggested_threshold));
                let gaps: Vec<String> = r
                    .beeps
                    .windows(2)
                    .map(|w| format!("{:.0}s", w[1] - w[0]))
                    .collect();
                ui.label(format!(
                    "Beeps heard: {}{}",
                    r.beeps.len(),
                    if gaps.is_empty() {
                        String::new()
                    } else {
                        format!(" (gaps {})", gaps.join(", "))
                    }
                ));
                if ui.button("Use these values and save").clicked() {
                    cfg.tone_hz = r.tone_hz;
                    cfg.threshold = r.suggested_threshold;
                    save = true;
                }
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(220, 60, 50), e);
            }
            None => {}
        }
        save
    }

    fn draw_spectrum(&self, ui: &mut egui::Ui, cfg: &Config) {
        let size = Vec2::new(ui.available_width().min(600.0), 170.0);
        let (response, painter) = ui.allocate_painter(size, Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 2.0, Color32::from_gray(24));

        let plot = Rect::from_min_max(
            Pos2::new(rect.left() + 18.0, rect.top() + 4.0),
            Pos2::new(rect.right() - 18.0, rect.bottom() - 18.0),
        );
        let x_of = |hz: f32| plot.left() + (hz - LO) / (HI - LO) * plot.width();
        let y_of = |v: f32| plot.bottom() - v.clamp(0.0, 1.0) * plot.height();

        let bar_w = (plot.width() / BARS as f32).max(1.0);
        for (i, v) in self.spectrum.iter().enumerate() {
            let x = x_of(LO + i as f32 * STEP);
            let bar =
                Rect::from_min_max(Pos2::new(x, y_of(*v)), Pos2::new(x + bar_w, plot.bottom()));
            let color = if *v >= cfg.threshold {
                Color32::from_rgb(60, 170, 90)
            } else {
                Color32::from_rgb(80, 130, 200)
            };
            painter.rect_filled(bar, 0.0, color);
        }
        // Threshold (horizontal) and configured tone (vertical).
        painter.line_segment(
            [
                Pos2::new(plot.left(), y_of(cfg.threshold)),
                Pos2::new(plot.right(), y_of(cfg.threshold)),
            ],
            Stroke::new(1.0, Color32::from_rgb(230, 170, 50)),
        );
        if (LO..=HI).contains(&cfg.tone_hz) {
            let x = x_of(cfg.tone_hz);
            painter.line_segment(
                [Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())],
                Stroke::new(2.0, Color32::from_rgb(235, 70, 60)),
            );
        }
        for hz in (LO as u32..=HI as u32).step_by(500) {
            painter.text(
                Pos2::new(x_of(hz as f32), rect.bottom() - 2.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{hz}"),
                egui::FontId::proportional(10.0),
                Color32::GRAY,
            );
        }
        ui.label(
            egui::RichText::new(
                "red line: configured tone, orange line: threshold, bars: live tone strength",
            )
            .small()
            .weak(),
        );
    }
}
