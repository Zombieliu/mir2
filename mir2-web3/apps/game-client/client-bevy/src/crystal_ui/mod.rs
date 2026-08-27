//! Crystal-authored 1024x768 UI specifications for the native Bevy shell.
//!
//! These modules contain presentation data and coordinate transforms only.
//! They do not own login, character, quest, inventory, or gameplay authority.

pub mod assets;
pub mod chat;
pub mod hud;
pub mod login;
pub mod metrics;
pub mod minimap;
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
