//! Client side of the daemon's control socket (Unix only).

use crate::config;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Sends one command line to the daemon and returns its reply.
pub fn request(line: &str) -> Result<String, String> {
    let path = config::socket_path().ok_or("XDG_RUNTIME_DIR is not set")?;
    let mut conn = UnixStream::connect(&path).map_err(|e| {
        format!(
            "cannot reach upsbeepd at {}: {e} (is the service running?)",
            path.display()
        )
    })?;
    let _ = conn.set_read_timeout(Some(Duration::from_secs(3)));
    conn.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    conn.read_to_string(&mut reply).map_err(|e| e.to_string())?;
    Ok(reply)
}
