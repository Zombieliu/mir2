//! Crystal-authored 1024x768 UI specifications for the native Bevy shell.
//!
//! These modules contain presentation data and coordinate transforms only.
//! They do not own login, character, quest, inventory, or gameplay authority.

pub mod amount_input;
pub mod assets;
pub mod chat;
pub mod guild_storage;
pub mod hud;
mod item_image;
pub mod item_tooltip;
pub mod login;
pub mod metrics;
pub mod minimap;
pub mod notice;
pub mod overlays;
pub mod panel_layouts;
pub mod preview_data;
pub mod select;
pub mod spec;
pub mod typography;
pub mod widget;

pub use metrics::CrystalStageTransform;
pub use overlays::{NativePlayerUiSet, NativePlayerUiState};
pub use spec::{CrystalButtonSpec, CrystalFrameSpec, CrystalRect};
