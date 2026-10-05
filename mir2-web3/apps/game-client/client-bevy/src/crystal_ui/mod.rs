//! Crystal-authored 1024x768 UI specifications for the native Bevy shell.
//!
//! These modules contain presentation data and coordinate transforms only.
//! They do not own login, character, quest, inventory, or gameplay authority.

#[cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
pub mod amount_input;
pub mod assets;
pub mod bag_paint;
#[cfg(feature = "native-ui")]
pub(crate) mod change_password;
pub mod character_materials;
pub mod character_page;
pub mod character_stats;
#[cfg(all(feature = "portable-quest-ui", not(feature = "native-ui")))]
pub mod character_stats_portable;
#[cfg(feature = "native-ui")]
pub mod chat;
#[cfg(feature = "native-ui")]
pub mod guild_storage;
#[cfg(feature = "native-ui")]
pub mod hud;
#[cfg(not(feature = "native-ui"))]
#[path = "hud_portable.rs"]
pub mod hud;
pub mod hud_bar;
pub mod hud_orb;
pub mod item_image;
pub mod item_tooltip;
#[cfg(feature = "native-ui")]
pub mod login;
pub mod metrics;
pub mod mail_page_shared;
#[path = "friend_text_editor.rs"]
pub mod text_editor;
pub mod mail_editor;
pub mod mail_compose_shared;
#[cfg(feature = "native-ui")]
pub mod minimap;
pub mod notice;
#[cfg(feature = "native-ui")]
pub(crate) mod npc_item_quote;
#[cfg(feature = "native-ui")]
pub mod overlays;
#[cfg(not(feature = "native-ui"))]
#[path = "overlays_portable.rs"]
pub mod overlays;
pub mod panel_layouts;
pub mod panel_navigation;
#[cfg(feature = "native-ui")]
pub mod preview_data;
pub mod quest_targets;
#[cfg(feature = "native-ui")]
pub mod select;
pub mod shared_hud;
pub mod skill_page_shared;
pub mod spec;
pub mod storage_paint;
pub mod shop_paint;
#[cfg(feature = "native-ui")]
pub(crate) mod storage_password;
pub mod typography;
pub mod widget;

pub use metrics::CrystalStageTransform;
pub use overlays::{NativePlayerUiSet, NativePlayerUiState};
pub use spec::{CrystalButtonSpec, CrystalFrameSpec, CrystalRect};

#[cfg(feature = "native-ui")]
pub fn quest_key_triggered(
    state: &NativePlayerUiState,
    keys: &bevy::input::ButtonInput<bevy::prelude::KeyCode>,
    function: &str,
) -> bool {
    overlays::keyboard_dialog::host::triggered(&state.keyboard, keys, function)
}

#[cfg(not(feature = "native-ui"))]
pub fn quest_key_triggered(
    _state: &NativePlayerUiState,
    _keys: &bevy::input::ButtonInput<bevy::prelude::KeyCode>,
    _function: &str,
) -> bool {
    // The browser host owns shortcuts and passes explicit edges in its context.
    false
}
