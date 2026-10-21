//! On-demand calibration/settings window. TODO: level meter, spectrum peak, settings editor.

use eframe::egui;
use upsbeep_core::config::Config;

struct App {
    cfg: Config,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("UPS Beep Monitor");
        ui.add(egui::Slider::new(&mut self.cfg.tone_hz, 500.0..=3900.0).text("Tone (Hz)"));
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "UPS Beep Monitor",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(App { cfg: Config::default() }))),
    )
}
