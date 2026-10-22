//! Shared logic: config, tone detection, and the on-battery state machine.

pub mod analyze;
pub mod config;
pub mod detector;
pub mod gate;
pub mod goertzel;
#[cfg(unix)]
pub mod ipc;
pub mod watchdog;
