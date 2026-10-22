//! On-demand window: live daemon status and control, microphone calibration, settings.

mod calibrate;
mod settings;
mod status;

use eframe::egui;
use std::path::PathBuf;
use std::time::Duration;
use upsbeep_core::config::{self, Config};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Status,
    Calibrate,
    Settings,
}

struct App {
    tab: Tab,
    cfg: Config,
    cfg_path: PathBuf,
    notice: Option<String>,
    status: status::StatusPane,
    calibrate: calibrate::CalibratePane,
    settings: settings::SettingsPane,
}

impl App {
    fn new() -> Self {
        let cfg_path = config::default_path().unwrap_or_else(|| PathBuf::from("config.toml"));
        let (cfg, notice) = match Config::load(&cfg_path) {
            Ok(c) => (c, None),
            Err(e) if cfg_path.exists() => (Config::default(), Some(format!("config error: {e}"))),
            Err(_) => (Config::default(), None),
        };
        Self {
            tab: Tab::Status,
            cfg,
            cfg_path,
            notice,
            status: status::StatusPane::new(),
            calibrate: calibrate::CalibratePane::new(),
            settings: settings::SettingsPane::new(),
        }
    }

    fn save(&mut self) {
        self.notice = Some(match self.cfg.save(&self.cfg_path) {
            Ok(()) => format!(
                "saved {} (the daemon reloads it within a couple of seconds)",
                self.cfg_path.display()
            ),
            Err(e) => format!("save failed: {e}"),
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(50));

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Status, "Status");
                ui.selectable_value(&mut self.tab, Tab::Calibrate, "Calibrate");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
            });
        });
        egui::Panel::bottom("notice").show(ui, |ui| {
            ui.label(self.notice.as_deref().unwrap_or(""));
        });

        // The microphone is only open while the Calibrate tab is showing.
        self.calibrate
            .set_active(self.tab == Tab::Calibrate, &self.cfg);

        egui::CentralPanel::default_margins().show(ui, |ui| match self.tab {
            Tab::Status => self.status.show(ui),
            Tab::Calibrate => {
                if self.calibrate.show(ui, &mut self.cfg) {
                    self.save();
                }
            }
            Tab::Settings => {
                if self.settings.show(ui, &mut self.cfg) {
                    self.save();
                }
            }
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("UPS Beep Monitor")
            .with_inner_size([640.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "UPS Beep Monitor",
        options,
        Box::new(|_| Ok(Box::new(App::new()))),
    )
}
