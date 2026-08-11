//! Platform-neutral client presentation state.
//!
//! This crate is intentionally free of Bevy, browser, windowing and platform
//! SDK dependencies. It may smooth and present authoritative snapshots, but it
//! must never decide combat, inventory, progression, economy or social state.

#![forbid(unsafe_code)]

pub mod clock;
pub mod intent;
pub mod interpolation;
pub mod motion;
pub mod reconciliation;
