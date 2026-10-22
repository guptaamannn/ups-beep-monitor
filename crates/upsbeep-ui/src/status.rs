//! Live view of the running daemon, with cancel/pause/resume.

use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use upsbeep_core::ipc;

pub struct StatusPane {
    latest: Arc<Mutex<Result<String, String>>>,
    pause_minutes: u64,
    reply: Arc<Mutex<String>>,
}

impl StatusPane {
    pub fn new() -> Self {
        let latest = Arc::new(Mutex::new(Err("waiting for the daemon...".to_string())));
        let poll = latest.clone();
        // Polling on a thread keeps a slow or missing daemon from freezing the window.
        thread::spawn(move || {
            loop {
                let result = ipc::request("status");
                if let Ok(mut l) = poll.lock() {
                    *l = result;
                }
                thread::sleep(Duration::from_secs(1));
            }
        });
        Self {
            latest,
            pause_minutes: 30,
            reply: Arc::new(Mutex::new(String::new())),
        }
    }

    fn send(&self, command: String) {
        let reply = self.reply.clone();
        thread::spawn(move || {
            let text = match ipc::request(&command) {
                Ok(r) => r.trim().to_string(),
                Err(e) => e,
            };
            if let Ok(mut r) = reply.lock() {
                *r = text;
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let latest = self
            .latest
            .lock()
            .map(|l| l.clone())
            .unwrap_or_else(|_| Err("poisoned".into()));
        match latest {
            Ok(text) => {
                let on_battery = text.contains("ON BATTERY");
                let (label, color) = if on_battery {
                    ("ON BATTERY", egui::Color32::from_rgb(220, 60, 50))
                } else {
                    ("Mains OK", egui::Color32::from_rgb(60, 170, 90))
                };
                ui.heading(egui::RichText::new(label).color(color));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).monospace());
            }
            Err(e) => {
                ui.heading(egui::RichText::new("Daemon not reachable").color(egui::Color32::GRAY));
                ui.label(e);
                ui.label("Start it with: systemctl --user start upsbeepd");
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Cancel shutdown (this outage)").clicked() {
                self.send("cancel".into());
            }
            if ui.button("Resume").clicked() {
                self.send("resume".into());
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Pause shutdown for").clicked() {
                self.send(format!("pause {}", self.pause_minutes));
            }
            ui.add(
                egui::DragValue::new(&mut self.pause_minutes)
                    .range(1..=1440)
                    .suffix(" min"),
            );
        });
        if let Ok(r) = self.reply.lock()
            && !r.is_empty()
        {
            ui.label(format!("daemon: {r}"));
        }
    }
}
