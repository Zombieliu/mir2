//! Crystal-authored 1024x768 UI specifications for the native Bevy shell.
//!
//! These modules contain presentation data and coordinate transforms only.
//! They do not own login, character, quest, inventory, or gameplay authority.

#[cfg(feature = "native-ui")]
pub mod amount_input;
pub mod assets;
#[cfg(feature = "native-ui")]
pub mod chat;
#[cfg(feature = "native-ui")]
pub mod guild_storage;
#[cfg(feature = "native-ui")]
pub mod hud;
#[cfg(feature = "native-ui")]
mod item_image;
pub mod item_tooltip;
pub mod login;
pub mod metrics;
#[cfg(feature = "native-ui")]
pub mod minimap;
#[cfg(feature = "native-ui")]
pub mod notice;
#[cfg(feature = "native-ui")]
pub mod overlays;
#[cfg(feature = "native-ui")]
pub mod panel_layouts;
pub mod preview_data;
pub mod select;
pub mod spec;
pub mod typography;
pub mod widget;

pub use metrics::CrystalStageTransform;
#[cfg(feature = "native-ui")]
pub use overlays::{NativePlayerUiSet, NativePlayerUiState};
pub use spec::{CrystalButtonSpec, CrystalFrameSpec, CrystalRect};
