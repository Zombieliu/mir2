//! Platform-neutral client presentation state.
//!
//! This crate is intentionally free of Bevy, browser, windowing and platform
//! SDK dependencies. It may smooth and present authoritative snapshots, but it
//! must never decide combat, inventory, progression, economy or social state.

#![forbid(unsafe_code)]

pub mod clock;
pub mod equipment_pending;
pub mod intent;
pub mod interpolation;
pub mod mail_compose;
pub mod mail_parcel;
pub mod motion;
pub mod npc_gold_buy_attempt;
pub mod quest;
pub mod reconciliation;
