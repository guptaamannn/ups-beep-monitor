# Daemon build plan (`upsbeepd`)

Goal: a headless, tiny-footprint daemon that listens to a microphone, detects the UPS beep, and shuts the PC down after a configured time on battery. Linux (Ubuntu) and Windows 11 from one codebase.

Already done: `upsbeep-core` has `Config`, `goertzel::tone_ratio`, and the `Detector` state machine, all unit-tested.

## Design decisions

- **Audio:** `cpal` input stream, mono, 8 kHz if the device supports it. Otherwise take the nearest supported rate and downmix/convert in the callback.
- **Threading:** the `cpal` callback only pushes samples into a lock-free or `std::sync::mpsc` channel. A single worker thread frames the samples, runs Goertzel, and drives the `Detector`. No async runtime.
- **Block size:** ~50 ms (400 samples at 8 kHz). A block counts as a beep when `tone_ratio >= threshold`. Require 2 consecutive blocks to reject clicks.
- **Beep vs. state:** the core `Detector` takes one `bool` per beep event, not per block. Add a small beep-edge debouncer in core (`BeepGate`) that turns block results into one event per beep, with tests.
- **Shutdown:** behind a `Shutdown` trait. Linux runs `systemctl poweroff`, falling back to `shutdown -h now`. Windows runs `shutdown /s /t 0`. A `DryRun` impl only logs.
- **Config path:** `dirs`-style lookup done by hand to avoid a dependency: `$XDG_CONFIG_HOME/upsbeep/config.toml` or `~/.config/upsbeep/config.toml` on Linux, `%APPDATA%\upsbeep\config.toml` on Windows. `--config <path>` overrides. A missing file means defaults plus a log line.
- **Logging:** plain `eprintln!` with timestamps, no logging crate. Under systemd it goes to the journal.
- **CLI:** hand-parsed arguments, no `clap`, to keep the binary small.
- **Footprint target:** under 3 MB resident, under 0.5% CPU idle.

## Milestones

### 1. Config file loading
- Add `Config::load(path)` and `default_path()` (cfg-gated per OS).
- Write the default config to disk on `upsbeepd --write-default-config`.
- Tests: round-trip, missing fields use defaults, invalid TOML gives a clear error.

### 2. Beep gating in core
- Add `BeepGate` in `upsbeep-core`: input is block results with timestamps, output is one beep event per tone burst (min burst duration, min gap).
- Tests with synthetic block sequences: click rejected, long tone is one event, two bursts are two events.

### 3. Shutdown abstraction
- `Shutdown` trait plus Linux, Windows and DryRun implementations in `upsbeepd`.
- Pick the real or dry-run implementation from `config.dry_run`.
- Never run the real implementation in tests.

### 4. Audio capture
- Select the device by name or default; list devices with `--list-devices`.
- Build the input stream, handle sample formats (f32, i16, u16), downmix to mono, forward blocks to the worker.
- Handle stream errors: log, retry opening the device every 10 s (mic unplugged, suspend/resume).

### 5. Main loop wiring
- Worker: frame blocks, `tone_ratio`, `BeepGate`, `Detector::update`.
- Also call `update(now, false)` on a 1 s tick so timeouts fire while silent.
- On `Action::Shutdown`: log, call `Shutdown`, then exit.
- Log state transitions (mains to battery, battery to mains) and a countdown to shutdown.

### 6. Calibration helper (daemon side)
- `upsbeepd --analyze <seconds>`: record, then print the peak frequency, level, and detected beep timestamps and intervals.
- This is how you find `tone_hz`, `threshold` and `beep_timeout_secs` for your UPS before trusting it. The UI will reuse the same logic later.

### 7. Service installation
- Ubuntu: a systemd unit. Check first whether the mic is reachable from a system service under PipeWire; if not, ship it as a `systemd --user` unit with lingering enabled.
- Windows: a Task Scheduler entry that runs at startup as the user, with an install/uninstall script. Check the mic privacy setting for desktop apps.
- Document both in the README.

### 8. Verification
- Unit tests for core (gate, config).
- Manual test: play a recorded UPS beep through a speaker, in `dry_run`, and confirm the transitions and countdown in the log.
- Real test: pull the UPS from the wall in `dry_run` for one full on-battery cycle, then once with `dry_run = false` while nothing important is open.
- Measure RSS and CPU with `ps`, then Task Manager on Windows.
- Run `cargo clippy` and `cargo fmt`, and confirm `cargo build --release` works on both OSes.

## Risks and open questions

- Does the PC have a microphone, or is a USB mic needed? This decides whether the tool is usable at all. -> Not yet, but I'll connect a microphone into one of the audio jack of the motherboard.
- The beep pattern of your UPS model is unknown: tone, interval, and whether the pattern changes at low battery.
- A system service may not have mic access on either OS, which affects milestone 7.
- A suspended or hibernating PC cannot listen, so power settings may need changing.
- A false positive causes an unwanted shutdown. Mitigation: dry-run by default and the `BeepGate` plus sustained-time rules.

## Definition of done

The daemon starts at boot on both OSes, detects the real UPS beep, shuts the machine down after the configured time on battery, cancels if mains returns, stays inside the footprint target, and has been verified in a real power-pull test.
