use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Beep tone frequency in Hz (find it with the UI calibration view).
    pub tone_hz: f32,
    /// Goertzel power ratio (tone power / total power) above which a block counts as a beep.
    pub threshold: f32,
    /// Capture sample rate in Hz.
    pub sample_rate: u32,
    /// Seconds the beeps may be absent before the UPS is considered back on mains.
    pub beep_timeout_secs: u64,
    /// Seconds on battery before shutting down.
    pub shutdown_after_secs: u64,
    /// Input device name; `None` uses the system default.
    pub device: Option<String>,
    /// Log what would happen instead of shutting down.
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tone_hz: 2800.0,
            threshold: 0.5,
            sample_rate: 8000,
            beep_timeout_secs: 180,
            shutdown_after_secs: 300,
            device: None,
            dry_run: true,
        }
    }
}

impl Config {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config serializes")
    }
}
