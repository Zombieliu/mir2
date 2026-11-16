//! Offline GPU evidence tool, not the game executable. It does not install a gateway or authentication worker.
mod native_fonts;
mod packed_assets;
mod resource_status;
mod pc_hud;
mod pc_hud_visual_tests;
fn main() { pc_hud_visual_tests::capture_pc_hud_fixture(); }
