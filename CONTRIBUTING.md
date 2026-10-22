# Contributing to upsbeep

Thanks for helping. The most useful contributions right now:

- **UPS compatibility reports.** Tell us which UPS you tested and what its beep looks like; see [docs/ups-compatibility.md](docs/ups-compatibility.md).
- **The Windows port.** The detection logic is portable; what's missing is a startup task, a control channel that doesn't use Unix sockets, and testing.
- **Bug reports and fixes**, especially false or missed beeps.

Please read the [code of conduct](CODE_OF_CONDUCT.md) first. For anything larger than a small fix, open an issue before starting so we can agree on the approach.

## Development setup

You need Rust 1.95 or newer and, on Ubuntu:

```bash
sudo apt install pkg-config libasound2-dev libudev-dev libxkbcommon-dev \
  libwayland-dev libx11-dev libxcb1-dev libgl1-mesa-dev
```

Before opening a pull request, run what CI runs:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Design principles

- **Small and quiet.** The daemon's footprint is a few MB of RAM and near-zero CPU. Think twice before adding a dependency to `upsbeepd`, and never let it depend on a GUI crate.
- **Logic in `upsbeep-core`, I/O at the edges.** Detection, timing and state live in `upsbeep-core` with no audio or OS calls, so they can be tested with synthetic audio and a virtual clock (see `crates/upsbeep-core/tests/outage.rs`). Audio and the OS live in the other crates.
- **Safe by default.** `dry_run` defaults to `true`. Tests and defaults must never power a machine off. Any change to shutdown behaviour must keep dry-run logging working.
- **Platform code behind `cfg`.** Platform-specific code goes behind `#[cfg(...)]` and must not break the other platform's build.

## Testing without a UPS

Most behaviour is covered by tests that need no hardware. To try the whole daemon without a UPS, play a synthetic beep through your speakers into the microphone:

```bash
python3 - <<'PY'
import wave, struct, math
rate = 48000
w = wave.open("beep.wav", "wb"); w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate)
frames = []
for _ in range(3):
    frames += [int(32000 * math.sin(2 * math.pi * 1820 * i / rate)) for i in range(int(rate * 0.4))]
    frames += [0] * int(rate * 0.4)
w.writeframes(b"".join(struct.pack("<h", x) for x in frames)); w.close()
PY
cargo run -p upsbeepd -- --analyze 10 &   # then: aplay beep.wav
```

Use a config with short timings and `dry_run = true` to watch the outage logic react.

## Pull requests

- Keep changes focused; describe what changed and how you tested it. The PR template has a checklist.
- Add or update tests for behaviour changes, and update the README or `CHANGELOG.md` when users would notice.
- Pull requests are squash-merged, so individual commit messages don't need to be perfect, but the PR title should say what the change does.
- By submitting a contribution you agree to license it under the project's terms: [MIT OR Apache-2.0](README.md#license), with no additional terms.
