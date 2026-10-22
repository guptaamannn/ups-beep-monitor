# ups-beep-monitor

Shuts a PC down safely during a long power outage by listening for the beep of a UPS that has no data connection to the computer. Works on Linux and Windows.

## How it works

1. A microphone near the UPS feeds a small daemon.
2. The daemon runs a Goertzel filter on the UPS buzzer's tone frequency (typically 2–4 kHz).
3. A state machine treats periodic beeps as "on battery". If that lasts longer than the configured time, it shuts the PC down. If the beeps stop, it goes back to "mains OK".

If your UPS has a USB or serial port, prefer NUT or apcupsd; this tool is for UPSs with no data link.

## Layout

| Crate | Purpose |
|---|---|
| `crates/upsbeep-core` | Config, Goertzel tone detection, on-battery state machine (no I/O) |
| `crates/upsbeepd` | Headless daemon: audio capture via `cpal`, shutdown |
| `crates/upsbeep-ui` | On-demand `egui` window for calibration and settings |

## Build

```bash
cargo build --release
cargo test
```

Linux build dependencies:

```bash
sudo apt install pkg-config libasound2-dev libudev-dev libssl-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libxcb1-dev libgl1-mesa-dev
```

Windows needs Rust (MSVC toolchain) and Visual Studio Build Tools with the "Desktop development with C++" workload.

## Install on Ubuntu

```bash
packaging/linux/install.sh      # builds, installs to ~/.local/bin, starts a systemd user service
journalctl --user -u upsbeepd -f
packaging/linux/uninstall.sh
```

The service blocks system suspend while running (a sleeping PC cannot hear the UPS). Config lives at `~/.config/upsbeep/config.toml`; find your UPS's tone first with `upsbeepd --analyze 70` while it is beeping. Restart after editing: `systemctl --user restart upsbeepd`.

## Window

`upsbeep-ui` (also in the app menu as "UPS Beep Monitor") is an on-demand window; it uses no memory while closed.

- **Status:** live daemon state with cancel / pause / resume buttons.
- **Calibrate:** live level meter and tone spectrum with the configured tone and threshold marked. Make the UPS beep, press "Record and analyze", then "Use these values and save".
- **Settings:** edit every config option and save; the daemon reloads the file automatically.

The microphone is only opened while the Calibrate tab is showing.

## Control

`upsbeepctl` talks to the running daemon over a socket in `$XDG_RUNTIME_DIR` (owner-only):

```bash
upsbeepctl status         # state, shutdown countdown, last beep, mic level, config in use
upsbeepctl cancel         # don't shut down for the current outage (resets when mains returns)
upsbeepctl pause 30       # maintenance mode: no shutdown for 30 minutes (default 30)
upsbeepctl resume         # undo cancel or pause
```

A pause that expires during an outage lets the shutdown go ahead right away. Use `pause` before testing the UPS.

## Configuration

`~/.config/upsbeep/config.toml` is re-read automatically within a couple of seconds of saving; a file with errors (including unknown keys) is rejected and the old settings stay active. Options:

| Key | Default | Meaning |
|---|---|---|
| `tone_hz` | 2800 | UPS beep frequency (find it with `--analyze`) |
| `threshold` | 0.5 | Tone strength that counts as a beep |
| `beep_timeout_secs` | 90 | Silence after which mains is assumed back; must exceed the UPS beep interval |
| `shutdown_after_secs` | 300 | Seconds from the first beep to shutdown |
| `gate_min_on_ms` / `gate_min_gap_ms` | 100 / 200 | Beep length filter |
| `notify` | true | Desktop notifications (needs `notify-send`) |
| `event_log` | true | Log to `~/.local/state/upsbeep/events.log` (rotates at 1 MB) |
| `device` | system default | Input device name (substring match, see `--list-devices`) |
| `dry_run` | true | Log instead of powering off |

The daemon also warns (log and notification) if the microphone is silent for 10 minutes or unavailable for 1 minute, since that means the UPS is not being monitored.

## Status

The daemon (capture, detection, shutdown, `--analyze`) and the Linux service work. The Windows startup entry and the UI are still TODO. `dry_run` defaults to `true`, so nothing powers off until it is disabled in the config.
