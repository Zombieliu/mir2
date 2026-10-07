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

#[cfg(feature = "presentation-ui")]
pub mod item_tooltip;
#[cfg(feature = "presentation-ui")]
pub use item_tooltip::{item_tooltip_abi_version, item_tooltip_document};

#[cfg(feature = "presentation-ui")]
pub mod bag_to_belt;
#[cfg(feature = "presentation-ui")]
pub use bag_to_belt::{bag_to_belt_move_abi_version, bag_to_belt_move_plan};

#[cfg(feature = "presentation-ui")]
pub mod fishing_click;
#[cfg(feature = "presentation-ui")]
pub use fishing_click::{fishing_click_abi_version, fishing_click_targets, fishing_click_decision};

#[cfg(feature = "presentation-ui")]
pub mod entity_animation;
#[cfg(feature = "presentation-ui")]
pub use entity_animation::{entity_animation_abi_version, EntityAnimationBridge};

#[cfg(feature = "presentation-ui")]
pub mod npc_pearl_buy;
#[cfg(feature = "presentation-ui")]
pub use npc_pearl_buy::{npc_pearl_buy_abi_version, npc_pearl_buy_plan};
