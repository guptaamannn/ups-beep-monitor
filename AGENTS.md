# AGENTS.md

## Project
Rust workspace that detects a UPS's beeps through a microphone and shuts the PC down after a configured time on battery. Linux (Ubuntu) is supported; Windows is planned, so keep platform code behind `cfg`. Idle footprint of the daemon must stay tiny (a few MB RAM, near-zero CPU).

## Layout
- `crates/upsbeep-core`: pure logic (config, `goertzel`, `gate`, `detector`, `pipeline`, `analyze`, `watchdog`). No audio or OS calls; keep it unit-testable.
- `crates/upsbeep-audio`: microphone capture via cpal.
- `crates/upsbeepd`: headless daemon plus the `upsbeepctl` client. Must not depend on any GUI crate.
- `crates/upsbeep-ui`: egui calibration/settings app, run on demand.

## Commands
- `cargo test` — run all tests
- `cargo clippy --workspace --all-targets -- -D warnings` — lint (CI treats warnings as errors)
- `cargo fmt --all` — format
- `cargo build --release` — size-optimised build (lto, strip, panic=abort)

## Conventions
- Edition 2024, MSRV 1.95 (set by egui); version, edition, license and MSRV come from `[workspace.package]`.
- Licensed MIT OR Apache-2.0. See CONTRIBUTING.md for the full guidelines.
- Add dependencies with `cargo add` and keep the daemon's dependency tree small.
- Platform-specific code goes behind `#[cfg(target_os = ...)]` and must compile on both Linux and Windows.
- Put detection and timing logic in `upsbeep-core` with tests; keep I/O in the binaries.
- Config is TOML via `upsbeep_core::config::Config`.

## Safety
- `dry_run` defaults to `true`. Never make the code shut down a machine in tests or by default.
- Any change to shutdown behaviour must keep dry-run logging working.
