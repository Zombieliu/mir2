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

#[cfg(feature = "native-ui")]
pub mod audio;
pub mod big_map;
#[cfg(feature = "native-ui")]
pub mod character;
pub mod chat;
#[cfg(feature = "native-ui")]
pub mod chat_settings_effects;
#[cfg(feature = "native-ui")]
pub mod crystal_ui;
pub mod entities;
pub mod game_shop;
#[cfg(feature = "native-ui")]
pub mod hud;
pub mod inventory;
pub mod mail;
pub mod map;
#[cfg(feature = "native-ui")]
pub mod native_shell;
#[cfg(feature = "native-ui")]
pub mod native_shell_ui;
#[cfg(feature = "native-ui")]
pub mod options_effects;
pub mod pending_operations;
#[cfg(feature = "native-ui")]
pub mod quest_model;
#[cfg(feature = "native-ui")]
pub mod quest_ui;
pub mod read_model;
pub mod shop;
#[cfg(feature = "native-ui")]
pub mod skill_binding_persistence;
pub mod skill_binding_ui;
pub mod skill_model;
pub mod social;
pub mod storage;

pub use read_model::{PlayerStats, UiReadModel};
