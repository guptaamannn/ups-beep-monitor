use std::process::Command;

/// Powers the machine off, or only logs when `dry_run` is set.
pub fn power_off(dry_run: bool) {
    if dry_run {
        eprintln!("[dry-run] would power off now");
        return;
    }
    eprintln!("powering off");
    for (program, args) in commands() {
        match Command::new(program).args(*args).status() {
            Ok(s) if s.success() => return,
            Ok(s) => eprintln!("{program} exited with {s}"),
            Err(e) => eprintln!("could not run {program}: {e}"),
        }
    }
    eprintln!("all shutdown commands failed");
}

#[cfg(windows)]
fn commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[("shutdown", &["/s", "/t", "0"])]
}

#[cfg(not(windows))]
fn commands() -> &'static [(&'static str, &'static [&'static str])] {
    &[("systemctl", &["poweroff"]), ("shutdown", &["-h", "now"])]
}
