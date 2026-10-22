//! Event reporting: stderr (the journal under systemd), a rotating log file, and notifications.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use upsbeep_core::config::{self, Config};

const MAX_LOG_BYTES: u64 = 1_000_000;

pub struct Events {
    log: Option<PathBuf>,
    notify: bool,
}

impl Events {
    pub fn new(cfg: &Config) -> Self {
        Self {
            log: cfg.event_log.then(config::default_log_path).flatten(),
            notify: cfg.notify,
        }
    }

    /// Routine event: stderr and the log file.
    pub fn info(&self, msg: &str) {
        eprintln!("{msg}");
        if let Some(path) = &self.log
            && let Err(e) = append(path, msg)
        {
            eprintln!("cannot write {}: {e}", path.display());
        }
    }

    /// Important event: also a desktop notification.
    pub fn alert(&self, msg: &str) {
        self.info(msg);
        if self.notify {
            let msg = msg.to_string();
            // Waiting in a thread reaps the child; a missing notify-send is not an error.
            thread::spawn(move || {
                let _ = Command::new("notify-send")
                    .args(["-a", "upsbeepd", "-u", "critical", "UPS monitor", &msg])
                    .status();
            });
        }
    }
}

fn append(path: &PathBuf, msg: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        fs::rename(path, path.with_extension("log.1"))?;
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{} {msg}", iso_utc(secs))
}

/// Formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }
}
