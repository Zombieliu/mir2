//! Separate lightweight policy and presentation WebAssembly packages.
//! Both delegate to shared, platform-neutral client rules without a renderer.

#[cfg(not(feature = "presentation-ui"))]
mod core_api;
#[cfg(not(feature = "presentation-ui"))]
pub use core_api::*;

#[cfg(not(feature = "presentation-ui"))]
pub mod equipment_pending;
#[cfg(not(feature = "presentation-ui"))]
pub mod auth_ui;
#[cfg(not(feature = "presentation-ui"))]
pub mod mail_compose;
#[cfg(not(feature = "presentation-ui"))]
pub mod mail_parcel;
#[cfg(not(feature = "presentation-ui"))]
pub mod npc_gold_buy_attempt;

#[cfg(feature = "presentation-ui")]
pub mod map_route;
#[cfg(feature = "presentation-ui")]
pub mod chat_ui;
#[cfg(feature = "presentation-ui")]
pub mod cash_preview;
#[cfg(feature = "presentation-ui")]
pub mod npc_repair_quote;

#[cfg(feature = "presentation-ui")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn client_presentation_abi_version() -> u32 { 1 }
