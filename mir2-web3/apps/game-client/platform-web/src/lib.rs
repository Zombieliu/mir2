//! Separate lightweight Core, presentation and NPC purchase policy packages.
//! Each delegates to shared, platform-neutral client rules without a renderer.

#[cfg(all(feature = "presentation-ui", feature = "npc-purchase-policy"))]
compile_error!("presentation-ui and npc-purchase-policy are mutually exclusive product features");

// This visitor has no WebAssembly exports. Core Mail/Gold and NPC policy share
// the same strict raw JSON boundary without linking each other's adapter ABI.
#[cfg(any(test, not(feature = "presentation-ui")))]
mod strict_json;

#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
mod core_api;
#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub use core_api::*;

#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub mod equipment_pending;
#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub mod auth_ui;
#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub mod mail_compose;
#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub mod mail_parcel;
#[cfg(not(any(feature = "presentation-ui", feature = "npc-purchase-policy")))]
pub mod npc_gold_buy_attempt;
// Keep the existing pure ABI/oracle tests in default test builds. Production
// Core/PUI do not include the unpublished NPC receipt ABI.
#[cfg(any(test, feature = "npc-purchase-policy"))]
pub mod npc_purchase_receipt;
#[cfg(any(test, feature = "npc-purchase-policy"))]
pub use npc_purchase_receipt::{npc_purchase_receipt_abi_version, NpcPurchaseReceiptBridge};

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
pub mod ranking_inspect;
#[cfg(feature = "presentation-ui")]
pub use ranking_inspect::{ranking_inspect_abi_version, ranking_inspect_admission};

#[cfg(feature = "presentation-ui")]
pub mod entity_animation;
#[cfg(feature = "presentation-ui")]
pub use entity_animation::{entity_animation_abi_version, EntityAnimationBridge};

#[cfg(feature = "presentation-ui")]
pub mod npc_pearl_buy;
#[cfg(feature = "presentation-ui")]
pub use npc_pearl_buy::{npc_pearl_buy_abi_version, npc_pearl_buy_plan};
