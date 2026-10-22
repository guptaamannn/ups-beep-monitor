# Changelog

All notable changes are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-10-22

First release. Linux only.

### Added
- `upsbeepd` daemon: reads the microphone, detects the UPS beep with a Goertzel filter, tracks outages, and powers the PC off after a configurable time on battery. Dry-run by default.
- Beep gate that rejects clicks, and an outage state machine that notices mains returning when the beeps stop.
- Microphone watchdog that warns when the mic is silent or missing.
- Event log, desktop notifications, and automatic config reload with validation (unknown keys are rejected).
- `upsbeepctl` with `status`, `cancel`, `pause` and `resume`, over an owner-only control socket.
- `upsbeep-ui` window with live status, calibration (level meter, spectrum, record-and-analyze), and settings.
- `upsbeepd --analyze` and `--list-devices`.
- systemd user service, home-directory installer, and a `.deb` package.
- CI (format, clippy, tests, package build) and a release workflow.

[Unreleased]: https://github.com/guptaamannn/ups-beep-monitor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/guptaamannn/ups-beep-monitor/releases/tag/v0.1.0
