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
pub mod bag_ui;
pub mod big_map;
#[cfg(feature = "native-ui")]
pub mod character;
pub mod chat;
#[cfg(feature = "native-ui")]
pub mod chat_settings_effects;
pub mod combat_input;
#[cfg(feature = "native-ui")]
pub use crystal_ui::overlays::combat_mode_keys;
#[cfg(all(any(feature = "portable-quest-ui", feature = "item-tooltip"), not(feature = "native-ui")))]
#[path = "crystal_ui/combat_mode_keys.rs"]
pub mod combat_mode_keys;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod crystal_ui;
#[cfg(all(feature = "item-tooltip", not(any(feature = "native-ui", feature = "portable-quest-ui"))))]
pub mod crystal_ui {
    // Reuse the same read-only Native algorithm without enabling Bevy UI.
    #[path = "item_tooltip.rs"]
    pub mod item_tooltip;
    #[path = "character_stats.rs"]
    pub mod character_stats;
}
pub mod entities;
pub mod game_shop;
pub mod hero_model;
pub mod hero_ingress;
#[cfg(feature = "native-ui")]
pub mod hud;
pub mod inventory;
pub mod mail;
pub mod mail_service;
pub mod map;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod native_shell;
#[cfg(feature = "native-ui")]
pub mod native_shell_ui;
#[cfg(feature = "native-ui")]
pub mod options_effects;
pub mod pending_operations;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui", feature = "item-tooltip"))]
pub mod player_text;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_bag_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_character_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_hero_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_experience_bar_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_hp_orb_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_hud_bar_draw_plan;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_quest_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_spells_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_storage_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_npc_shop_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_mail_ui;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod portable_weight_bar_ui;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_destination;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_guidance;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_hunt_regions;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_intents;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_journey;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_model;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_practice;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_presentation_text;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_route_search;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_supplies;
#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod quest_ui;
pub mod read_model;
pub mod shop;
pub mod npc_shop_buy;
pub mod npc_shop_ui;
pub mod npc_gold_buy_attempt;
#[cfg(feature = "native-ui")]
pub mod skill_binding_persistence;
pub mod skill_binding_ui;
pub mod skill_model;
pub mod skill_page_state;
pub mod social;
pub mod storage;
pub mod storage_interaction;

pub use read_model::{PlayerStats, UiReadModel};
// Platform hosts consume shared fishing policy through the established Bevy adapter edge.
pub use mir2_client_core::fishing_click;
