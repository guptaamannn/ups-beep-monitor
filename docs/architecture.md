# Architecture

## Crates

| Crate | Role |
|---|---|
| `upsbeep-core` | Everything that decides things: config, tone detection, beep gate, outage detector, the `Pipeline` that joins them, recording analysis, mic watchdog, and the control-socket client. No audio or OS-specific calls. |
| `upsbeep-audio` | Microphone capture on top of `cpal`. Delivers fixed 50 ms blocks of mono `f32` samples. |
| `upsbeepd` | The daemon (main loop, event log, notifications, control socket server, shutdown) and the `upsbeepctl` client. |
| `upsbeep-ui` | The `egui` window. Separate binary, run on demand. |

The daemon never depends on a GUI crate, which keeps it small (a few MB of RAM, near-zero CPU).

## Detection chain

`Pipeline` in `upsbeep-core` is the whole decision path, with no I/O:

1. **Tone strength.** For each 50 ms block, a Goertzel filter measures energy at the configured frequency. The result, `tone_ratio`, is that energy divided by the block's total energy, so it is 1.0 for a pure tone and near 0 for noise, whatever the volume.
2. **Beep gate.** A block with `tone_ratio >= threshold` is "active". `BeepGate` fires once per beep, after the tone has lasted `gate_min_on_ms`, and ignores gaps shorter than `gate_min_gap_ms`. A one-block click never fires.
3. **Outage detector.** The first beep puts the detector in `OnBattery`. Further beeps keep it there. If no beep arrives for `beep_timeout_secs`, it returns to `Mains`. After `shutdown_after_secs` on battery it returns `Action::Shutdown`.

The audio callback only forwards blocks over a channel; the main thread does all the analysis. There is no async runtime.

## The daemon's main loop

Each iteration (at most one second apart):

1. Apply control commands (`cancel`, `pause`, `resume`).
2. Reload the config file if its modification time changed. Invalid files are rejected and the old settings stay.
3. (Re)open the microphone if needed; retry every 10 seconds.
4. Take one audio block or time out, feed the `Pipeline` and the `MicWatchdog`.
5. On `Action::Shutdown`: skip it if cancelled or paused, otherwise power off (or only log, in dry-run).
6. Publish a status snapshot for `upsbeepctl status` and the window.

## Control socket

A text protocol over a Unix socket at `$XDG_RUNTIME_DIR/upsbeep.sock` (mode 0600): one command line in, one reply out. Commands: `status`, `cancel`, `pause [MINUTES]`, `resume`. The server runs on its own thread and hands commands to the main loop through a channel, so the loop is the only place state changes.

## Calibration

`analyze_blocks` in `upsbeep-core` scans 500–4000 Hz in 20 Hz steps over a recording, finds the dominant tone, replays the recording through the gate to count beeps, and suggests a threshold (55% of the strongest signal, clamped). Both `upsbeepd --analyze` and the window's Calibrate tab use it.

## Testing

- Unit tests sit next to the code in `upsbeep-core`.
- `crates/upsbeep-core/tests/outage.rs` drives the `Pipeline` with synthetic audio and a virtual clock, using a UPS-like beep pattern: a long outage, mains returning, a missed beep, other sounds, and reconfiguring mid-outage.
- The socket protocol has a real round-trip test in `upsbeepd`.
- Hardware behaviour is checked by hand; see [CONTRIBUTING.md](../CONTRIBUTING.md) for testing without a UPS.
