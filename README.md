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

## Status

Early scaffold. Core logic and tests exist; the daemon capture loop, platform shutdown, config file handling and the UI are still TODO. `dry_run` defaults to `true`, so nothing powers off until it is disabled in the config.
