use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Beep tone frequency in Hz (find it with the UI calibration view).
    pub tone_hz: f32,
    /// Goertzel power ratio (tone power / total power) above which a block counts as a beep.
    pub threshold: f32,
    /// Seconds without a beep before the UPS is considered back on mains. Must exceed the
    /// UPS beep interval (60 s on this UPS) with some margin for a missed beep.
    pub beep_timeout_secs: u64,
    /// Seconds from the first beep before shutting down. The UPS only starts beeping about
    /// 60 s after the outage begins, so the real outage is this plus ~60 s when shutdown fires.
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
            beep_timeout_secs: 90,
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

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_toml(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(path, self.to_toml()).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config serializes")
    }
}

/// Per-OS default config location, or `None` if the home directory is unknown.
pub fn default_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.map(|b| b.join("upsbeep").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let cfg = Config {
            tone_hz: 3100.0,
            dry_run: false,
            ..Config::default()
        };
        let back = Config::from_toml(&cfg.to_toml()).unwrap();
        assert_eq!(back.tone_hz, 3100.0);
        assert!(!back.dry_run);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let cfg = Config::from_toml("tone_hz = 2000.0").unwrap();
        assert_eq!(cfg.tone_hz, 2000.0);
        assert_eq!(cfg.threshold, Config::default().threshold);
    }

    #[test]
    fn invalid_toml_is_an_error() {
        assert!(Config::from_toml("tone_hz = [").is_err());
    }

    #[test]
    fn save_then_load() {
        let dir = std::env::temp_dir().join(format!("upsbeep-test-{}", std::process::id()));
        let path = dir.join("nested").join("config.toml");
        Config::default().save(&path).unwrap();
        assert_eq!(
            Config::load(&path).unwrap().tone_hz,
            Config::default().tone_hz
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
