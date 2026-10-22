//! Command-line client for the upsbeepd control socket.

use std::process::ExitCode;
use upsbeep_core::ipc::request;

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
