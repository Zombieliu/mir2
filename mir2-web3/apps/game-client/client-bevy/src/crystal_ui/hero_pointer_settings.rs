//! Host-provided Windows double-click timing, isolated from persistent game settings.
#[derive(Debug, Clone, Copy, bevy::prelude::Resource)]
pub struct HeroPointerSettings {
    pub double_click_ms: u64,
}
impl Default for HeroPointerSettings {
    fn default() -> Self {
        Self {
            double_click_ms: 500,
        }
    }
}
