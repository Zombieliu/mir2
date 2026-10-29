use super::*;

fn damian() -> CrystalNpcInfoTemplate {
    crystal_npc_info_manifest()
        .npcs
        .into_iter()
        .find(|npc| npc.loaded_object_id == Some(1_358))
        .expect("Crystal manifest should contain travelling merchant Damian")
}

#[test]
fn newcomer_profile_keeps_mandatory_damian_visible_outside_his_crystal_hour() {
    let npc = damian();
    assert!(npc.time_visible);
    let character = crate::SimulationConfig::default().default_character;
    let outside_window = CrystalNpcLocalTime::new(0, 4, 35);

    assert!(!crystal_npc_visible_to_character_for_profile(
        &npc,
        &character,
        &[],
        outside_window,
        false,
    ));
    assert!(crystal_npc_visible_to_character_for_profile(
        &npc,
        &character,
        &[],
        outside_window,
        true,
    ));
}

#[test]
fn ordinary_crystal_profile_retains_damians_original_time_window() {
    let npc = damian();
    let character = crate::SimulationConfig::default().default_character;
    let inside_window = CrystalNpcLocalTime::new(0, 2, 0);
    let finish_boundary = CrystalNpcLocalTime::new(0, 2, 30);

    assert!(crystal_npc_visible_to_character_for_profile(
        &npc,
        &character,
        &[],
        inside_window,
        false,
    ));
    assert!(!crystal_npc_visible_to_character_for_profile(
        &npc,
        &character,
        &[],
        finish_boundary,
        false,
    ));
}
