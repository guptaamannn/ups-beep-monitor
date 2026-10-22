# upsbeep

[![CI](https://github.com/guptaamannn/ups-beep-monitor/actions/workflows/ci.yml/badge.svg)](https://github.com/guptaamannn/ups-beep-monitor/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**Shut your PC down safely during a long power outage, even if your UPS has no data cable.**

Many UPSs have no USB or serial link to the computer, but they all beep while running on battery. `upsbeep` listens for that beep with a microphone, decides there is an outage, and powers the PC off before the battery runs out. It is a small Rust service that uses a few MB of RAM and almost no CPU.

> If your UPS has a USB or serial port, use [NUT](https://networkupstools.org/) or [apcupsd](http://www.apcupsd.org/) instead. They read the real battery state and are far more reliable than listening for a beep. `upsbeep` is for UPSs with no data link.

## How it works

```
microphone ──► tone detector ──► beep gate ──► outage detector ──► shutdown
              (Goertzel filter   (rejects      (beeps = on battery,
               at the beep        clicks)       silence = mains back)
               frequency)
```

1. The microphone is read in 50 ms blocks. A Goertzel filter measures how much of each block's energy sits at your UPS's beep frequency (typically 1–4 kHz).
2. A gate turns strong blocks into one event per beep, ignoring short clicks.
3. Repeated beeps mean "on battery". If they continue for the configured time, the PC powers off. If the beeps stop (no beep for longer than the UPS's beep interval), mains is assumed back and the shutdown is cancelled.

Details are in [docs/architecture.md](docs/architecture.md).

## Requirements

- Linux with PipeWire or ALSA and systemd. Developed and tested on Ubuntu 26.04 (GNOME, Wayland). Windows is not supported yet; see the [roadmap](#status-and-roadmap).
- A microphone within earshot of the UPS. A cheap USB mic or a mic in the motherboard's audio jack works.
- A UPS that beeps periodically while on battery. See [docs/ups-compatibility.md](docs/ups-compatibility.md).

## Read this first

- **A false alarm powers your PC off.** It can happen if other sounds sit at the beep frequency. `upsbeep` ships in **dry-run mode** (it only logs). Run it that way for a few days, check the event log for false beeps, and only then turn dry-run off.
- **A dead microphone means no protection.** The daemon warns you if the mic is silent for 10 minutes or missing for 1 minute, but it cannot shut the PC down if it cannot hear the UPS.
- **Set the shutdown delay below your battery runtime.** Your UPS's runtime depends on the load; a delay of roughly 40% of the runtime leaves a safety margin.
- This software comes with **no warranty** (see the licenses). It is not a substitute for a UPS with a real data connection when data safety matters.

## Quick start (Ubuntu)

1. **Install.** Download the `.deb` from the [latest release](https://github.com/guptaamannn/ups-beep-monitor/releases/latest), then:
   ```bash
   sudo apt install ./upsbeep_*.deb
   ```
2. **Calibrate.** Place the microphone near the UPS and run `upsbeep-ui`. On the *Calibrate* tab, make the UPS beep (cut its mains briefly, or press its self-test button), press *Record and analyze*, then *Use these values and save*.
3. **Start the service** (as your own user):
   ```bash
   systemctl --user enable --now upsbeepd
   upsbeepctl status
   ```
4. **Test in dry-run.** Unplug the UPS from the wall. After it starts beeping, `upsbeepctl status` and `journalctl --user -u upsbeepd -f` should show the outage and a shutdown countdown, ending with `[dry-run] would power off now`. Plug it back in and watch it report mains is back.
5. **Go live.** In the *Settings* tab, untick *Dry run*, set *Shutdown after first beep*, and save. The daemon reloads the file by itself.

To build from source instead, see [Building](#building).

## The window

`upsbeep-ui` (in the app menu as "UPS Beep Monitor") is an on-demand window. It uses no memory while closed.

- **Status:** live daemon state, with cancel / pause / resume buttons.
- **Calibrate:** live level meter and tone spectrum, with the configured tone and threshold marked, plus record-and-analyze. The microphone is only opened while this tab is showing.
- **Settings:** every config option; saving takes effect within a couple of seconds.

## Command line

```bash
upsbeepctl status         # state, shutdown countdown, last beep, mic level, config in use
upsbeepctl cancel         # don't shut down for the current outage (resets when mains returns)
upsbeepctl pause 30       # maintenance mode: no shutdown for 30 minutes (default 30)
upsbeepctl resume         # undo cancel or pause

upsbeepd --analyze 70     # record 70 s and report the beep tone, timing and a suggested threshold
upsbeepd --list-devices   # microphones the daemon can use
```

`upsbeepctl` talks to the daemon over an owner-only socket in `$XDG_RUNTIME_DIR`. A pause that expires during an outage lets the shutdown go ahead right away, so use `pause` before testing the UPS.

## Configuration

`~/.config/upsbeep/config.toml` is re-read automatically within a couple of seconds of saving. A file with errors, including unknown keys, is rejected and the old settings stay active.

| Key | Default | Meaning |
|---|---|---|
| `tone_hz` | 2800 | UPS beep frequency (find it with the *Calibrate* tab or `--analyze`) |
| `threshold` | 0.5 | Tone strength (0–1) that counts as a beep |
| `beep_timeout_secs` | 90 | Silence after which mains is assumed back. Must exceed the UPS beep interval; about twice the interval tolerates one missed beep |
| `shutdown_after_secs` | 300 | Seconds from the first beep to shutdown. Many UPSs only start beeping a while after the outage begins, so the real outage is longer |
| `gate_min_on_ms` / `gate_min_gap_ms` | 100 / 200 | Beep length filter |
| `notify` | true | Desktop notifications (needs `notify-send`) |
| `event_log` | true | Log to `~/.local/state/upsbeep/events.log` (rotates at 1 MB) |
| `device` | system default | Input device name (substring match; see `--list-devices`) |
| `dry_run` | true | Log instead of powering off |

## Troubleshooting

- **Status says "NOT RECEIVING AUDIO".** The mic is missing, muted, or in use exclusively by another app. Check `upsbeepd --list-devices` and the sound settings.
- **No audio right after boot.** The service retries every 10 seconds, so it usually connects within a minute of login.
- **No beeps detected.** Run the *Calibrate* tab and watch the spectrum while the UPS beeps. If you see no peak, move the mic closer or raise its input gain. If the peak is below the orange threshold line, lower `threshold`.
- **Shutdown doesn't happen.** The service runs `systemctl poweroff` as your user, which works for a logged-in local session. Check `journalctl --user -u upsbeepd` for the error.
- **No notifications.** Install `libnotify-bin` (provides `notify-send`).
- **The PC suspends.** The service holds a sleep inhibitor while it runs, so suspend is blocked. A sleeping PC can't hear the UPS.

## Building

Requires Rust 1.95 or newer. On Ubuntu:

```bash
sudo apt install pkg-config libasound2-dev libudev-dev libxkbcommon-dev \
  libwayland-dev libx11-dev libxcb1-dev libgl1-mesa-dev
cargo build --release
cargo test --workspace
```

Install for your user only (binaries in `~/.local/bin`, plus a systemd user service and launcher entry), or build a `.deb`:

```bash
packaging/linux/install.sh      # packaging/linux/uninstall.sh to remove
packaging/linux/build-deb.sh    # writes target/deb/upsbeep_<version>_<arch>.deb
```

| Crate | Purpose |
|---|---|
| `crates/upsbeep-core` | Config, tone detection, beep gate, outage state machine, analysis. No audio or OS calls |
| `crates/upsbeep-audio` | Microphone capture via `cpal` |
| `crates/upsbeepd` | The daemon and the `upsbeepctl` client |
| `crates/upsbeep-ui` | The `egui` window |

## Status and roadmap

The Linux tool is feature-complete and has been verified on real hardware, including a real shutdown. Planned:

- Windows support (startup task, and a control channel that doesn't use Unix sockets)
- More UPS compatibility reports; see [docs/ups-compatibility.md](docs/ups-compatibility.md)

See the [changelog](CHANGELOG.md) for what has shipped.

## Contributing

Contributions are welcome, especially UPS compatibility reports and the Windows port. See [CONTRIBUTING.md](CONTRIBUTING.md) and the [code of conduct](CODE_OF_CONDUCT.md). To report a security issue, see [SECURITY.md](SECURITY.md).

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
