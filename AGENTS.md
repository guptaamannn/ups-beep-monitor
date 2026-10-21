# AGENTS.md

## Project
Rust workspace that detects a UPS's beeps through a microphone and shuts the PC down after a configured time on battery. Targets Ubuntu and Windows 11 from one codebase. Idle footprint of the daemon must stay tiny (a few MB RAM, near-zero CPU).

## Layout
- `crates/upsbeep-core`: pure logic (config, `goertzel`, `detector`). No audio or OS calls; keep it unit-testable.
- `crates/upsbeepd`: headless daemon. Must not depend on any GUI crate.
- `crates/upsbeep-ui`: egui calibration/settings app, run on demand.

## Commands
- `cargo test` — run all tests
- `cargo clippy --workspace --all-targets` — lint
- `cargo fmt --all` — format
- `cargo build --release` — size-optimised build (lto, strip, panic=abort)

## Conventions
- Edition 2024; version and edition are inherited from `[workspace.package]`.
- Add dependencies with `cargo add` and keep the daemon's dependency tree small.
- Platform-specific code goes behind `#[cfg(target_os = ...)]` and must compile on both Linux and Windows.
- Put detection and timing logic in `upsbeep-core` with tests; keep I/O in the binaries.
- Config is TOML via `upsbeep_core::config::Config`.

## Safety
- `dry_run` defaults to `true`. Never make the code shut down a machine in tests or by default.
- Any change to shutdown behaviour must keep dry-run logging working.
