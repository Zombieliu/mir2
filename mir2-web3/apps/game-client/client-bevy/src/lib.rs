//! Shared Bevy rendering and in-game UI for mir2-web3.
//!
//! `client-bevy` adapts renderer-neutral state from `mir2-client-core` into
//! Bevy types and renders the shared in-game UI (HUD, panels). It must never
//! grant items, XP, currency, ownership or success; it only *presents*
//! authoritative state and emits intents back to the server.
//!
//! Dependency rule (ADR-0001):
//!
//! ```text
//! platform host -> client-bevy -> client-core -> protocol/public content schema
//!                                       X
//!                                       | no server-simulation/platform SDK/DOM
//! ```

#![forbid(unsafe_code)]

pub mod character;
pub mod chat;
pub mod entities;
pub mod hud;
pub mod inventory;
pub mod map;
pub mod read_model;

pub use read_model::{PlayerStats, UiReadModel};
