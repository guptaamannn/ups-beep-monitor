//! Command-line client for the upsbeepd control socket.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use upsbeep_core::config;

const USAGE: &str = "usage: upsbeepctl status | cancel | pause [MINUTES] | resume

  status           show the current state and shutdown countdown
  cancel           don't shut down for the current outage
  pause [MINUTES]  don't shut down for a while (default 30), e.g. while testing the UPS
  resume           undo cancel or pause";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        println!("{USAGE}");
        return ExitCode::from(u8::from(args.is_empty()) * 2);
    }
    match request(&args.join(" ")) {
        Ok(reply) => {
            print!("{reply}");
            ExitCode::from(u8::from(reply.starts_with("error:")))
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn request(line: &str) -> Result<String, String> {
    let path = config::socket_path().ok_or("XDG_RUNTIME_DIR is not set")?;
    let mut conn = UnixStream::connect(&path).map_err(|e| {
        format!(
            "cannot reach upsbeepd at {}: {e} (is the service running?)",
            path.display()
        )
    })?;
    conn.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    conn.read_to_string(&mut reply).map_err(|e| e.to_string())?;
    Ok(reply)
}
