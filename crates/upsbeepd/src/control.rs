//! Local control socket: `upsbeepctl` asks for status or sends cancel/pause/resume.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const DEFAULT_PAUSE_MINUTES: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// Don't shut down for the current outage.
    Cancel,
    /// Don't shut down for this long (maintenance mode).
    Pause(Duration),
    /// Undo cancel/pause.
    Resume,
}

#[derive(Debug, PartialEq, Eq)]
enum Request {
    Status,
    Cmd(Cmd),
}

fn parse(line: &str) -> Result<Request, String> {
    let mut words = line.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("status"), None, _) => Ok(Request::Status),
        (Some("cancel"), None, _) => Ok(Request::Cmd(Cmd::Cancel)),
        (Some("resume"), None, _) => Ok(Request::Cmd(Cmd::Resume)),
        (Some("pause"), mins, None) => {
            let mins = match mins {
                None => DEFAULT_PAUSE_MINUTES,
                Some(m) => m
                    .parse::<u64>()
                    .ok()
                    .filter(|m| (1..=24 * 60).contains(m))
                    .ok_or("pause takes minutes between 1 and 1440")?,
            };
            Ok(Request::Cmd(Cmd::Pause(Duration::from_secs(mins * 60))))
        }
        _ => Err("unknown command; use status, cancel, pause [MINUTES], or resume".into()),
    }
}

/// Starts the socket server thread. `status` is the text served for `status`.
pub fn start(path: &Path, status: Arc<Mutex<String>>) -> Result<Receiver<Cmd>, String> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(format!(
                "{} is in use: is upsbeepd already running?",
                path.display()
            ));
        }
        std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let listener = UnixListener::bind(path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("{}: {e}", path.display()))?;

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            serve(conn, &tx, &status);
        }
    });
    Ok(rx)
}

fn serve(conn: UnixStream, tx: &Sender<Cmd>, status: &Mutex<String>) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
    let mut line = String::new();
    if BufReader::new(&conn).read_line(&mut line).is_err() {
        return;
    }
    let reply = match parse(&line) {
        Ok(Request::Status) => status.lock().map(|s| s.clone()).unwrap_or_default(),
        Ok(Request::Cmd(cmd)) => {
            let _ = tx.send(cmd);
            "ok\n".to_string()
        }
        Err(e) => format!("error: {e}\n"),
    };
    let _ = (&conn).write_all(reply.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        assert_eq!(parse("status\n"), Ok(Request::Status));
        assert_eq!(parse("cancel"), Ok(Request::Cmd(Cmd::Cancel)));
        assert_eq!(parse("resume"), Ok(Request::Cmd(Cmd::Resume)));
        assert_eq!(
            parse("pause\n"),
            Ok(Request::Cmd(Cmd::Pause(Duration::from_secs(1800))))
        );
        assert_eq!(
            parse("pause 5"),
            Ok(Request::Cmd(Cmd::Pause(Duration::from_secs(300))))
        );
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "",
            "pause 0",
            "pause abc",
            "pause 5 6",
            "shutdown",
            "status now",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn serves_status_and_commands_over_the_socket() {
        let path = std::env::temp_dir().join(format!("upsbeep-test-{}.sock", std::process::id()));
        let status = Arc::new(Mutex::new("state: mains\n".to_string()));
        let rx = start(&path, status).unwrap();

        let ask = |msg: &str| {
            let mut c = UnixStream::connect(&path).unwrap();
            c.write_all(msg.as_bytes()).unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut c, &mut out).unwrap();
            out
        };
        assert_eq!(ask("status\n"), "state: mains\n");
        assert_eq!(ask("cancel\n"), "ok\n");
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)), Ok(Cmd::Cancel));
        assert!(ask("bogus\n").starts_with("error:"));
        assert!(start(&path, Arc::new(Mutex::new(String::new()))).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
