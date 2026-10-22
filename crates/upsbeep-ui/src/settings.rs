//! Editor for the daemon's config file.

use eframe::egui;
use upsbeep_audio as audio;
use upsbeep_core::config::Config;

pub struct SettingsPane {
    devices: Vec<String>,
}

impl SettingsPane {
    pub fn new() -> Self {
        Self {
            devices: audio::device_names().map(|(n, _)| n).unwrap_or_default(),
        }
    }

    /// Returns `true` when the user asked to save.
    pub fn show(&mut self, ui: &mut egui::Ui, cfg: &mut Config) -> bool {
        egui::Grid::new("settings")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Beep tone");
                ui.add(
                    egui::DragValue::new(&mut cfg.tone_hz)
                        .range(200.0..=8000.0)
                        .suffix(" Hz"),
                );
                ui.end_row();

                ui.label("Detection threshold");
                ui.add(egui::Slider::new(&mut cfg.threshold, 0.05..=0.95));
                ui.end_row();

                ui.label("Shutdown after first beep");
                ui.add(
                    egui::DragValue::new(&mut cfg.shutdown_after_secs)
                        .range(10..=7200)
                        .suffix(" s"),
                );
                ui.end_row();

                ui.label("Mains back after silence");
                ui.add(
                    egui::DragValue::new(&mut cfg.beep_timeout_secs)
                        .range(10..=3600)
                        .suffix(" s"),
                );
                ui.end_row();

                ui.label("Beep min length");
                ui.add(
                    egui::DragValue::new(&mut cfg.gate_min_on_ms)
                        .range(10..=2000)
                        .suffix(" ms"),
                );
                ui.end_row();

                ui.label("Beep gap");
                ui.add(
                    egui::DragValue::new(&mut cfg.gate_min_gap_ms)
                        .range(10..=2000)
                        .suffix(" ms"),
                );
                ui.end_row();

                ui.label("Microphone");
                let current = cfg
                    .device
                    .clone()
                    .unwrap_or_else(|| "system default".into());
                egui::ComboBox::from_id_salt("device")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut cfg.device, None, "system default");
                        for name in &self.devices {
                            ui.selectable_value(&mut cfg.device, Some(name.clone()), name);
                        }
                    });
                ui.end_row();

                ui.label("Desktop notifications");
                ui.checkbox(&mut cfg.notify, "");
                ui.end_row();

                ui.label("Event log");
                ui.checkbox(&mut cfg.event_log, "");
                ui.end_row();

                ui.label("Dry run");
                ui.checkbox(&mut cfg.dry_run, "only log, don't power off");
                ui.end_row();
            });
        if !cfg.dry_run {
            ui.colored_label(
                egui::Color32::from_rgb(220, 60, 50),
                "Dry run is OFF: the PC will power off after a real outage.",
            );
        }
        ui.add_space(12.0);
        ui.button("Save").clicked()
    }
}
