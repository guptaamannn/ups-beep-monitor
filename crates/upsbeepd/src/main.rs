//! Headless daemon: listens to the mic, detects UPS beeps, shuts the PC down.
//! TODO: cpal capture loop -> goertzel::tone_ratio -> Detector -> platform shutdown.

use upsbeep_core::config::Config;

fn main() {
    let cfg = Config::default();
    println!("upsbeepd starting with config:\n{}", cfg.to_toml());
}
