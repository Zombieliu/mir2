//! Read-only retained renderer observations for native diagnostics.
//! These are not a GPU fence, authentication, gameplay ACK or acceptance.
use bevy::prelude::Resource;
#[derive(Resource, Default, Debug, Clone)]
pub struct NativeLightingDiagnostics {
    pub updates_observed: u64,
    pub map_file_name: Option<String>,
    pub source_enabled: bool,
    pub effective_light_setting: Option<i32>,
    pub map_dark_light: i32,
    pub map_source_count: usize,
    pub entity_source_count: usize,
    pub material_enabled: bool,
    pub retained_light_layers: usize,
    pub original_textures_loaded: usize,
}
