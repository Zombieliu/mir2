use super::*;

#[test]
fn ordinary_panels_capture_their_rectangles_without_locking_world_actions() {
    for (panel, inside) in [
        (mir2_ui_core::state::UiPanel::Inventory, [20.0, 60.0]),
        (mir2_ui_core::state::UiPanel::Character, [780.0, 120.0]),
        (mir2_ui_core::state::UiPanel::Skill, [780.0, 120.0]),
        (mir2_ui_core::state::UiPanel::Options, [400.0, 300.0]),
        (mir2_ui_core::state::UiPanel::Menu, [1000.0, 400.0]),
        (mir2_ui_core::state::UiPanel::QuestLog, [220.0, 200.0]),
    ] {
        let mut state = NativePlayerUiState::default();
        state.core.panel = panel;
        assert!(
            state.blocks_world_pointer_at(inside[0], inside[1]),
            "{panel:?} pointer leaked"
        );
        assert!(
            !state.blocks_world_pointer_at(700.0, 500.0),
            "{panel:?} froze outside pointer"
        );
        assert!(
            !state.blocks_world_action(false, false),
            "{panel:?} froze keyboard/actions"
        );
        assert!(
            !state.blocks_route_navigation(),
            "{panel:?} cancelled established target/route"
        );
        state.skill_assign.open = true;
        assert!(state.blocks_world_pointer_at(700.0, 500.0));
        assert!(state.blocks_world_action(false, false));
        state.skill_assign.open = false;
        state.core.chat_focused = true;
        assert!(state.blocks_world_action(false, false));
        assert!(state.blocks_route_navigation());
    }
}

#[test]
fn actual_skills_toggle_keeps_character_frame_hit_capture_on_every_page() {
    let mut state = NativePlayerUiState::default();
    state.toggle_skill();
    assert!(state.skill_open());
    assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::Character);
    for page in [
        CharacterPage::Character,
        CharacterPage::Stats1,
        CharacterPage::Stats2,
        CharacterPage::Spells,
    ] {
        state.character_page = page;
        assert!(state.blocks_world_pointer_at(780.0, 120.0));
        assert!(!state.blocks_world_pointer_at(700.0, 500.0));
        assert!(!state.blocks_world_action(false, false));
    }
}

#[test]
fn help_surface_follows_dragged_bounds_and_keeps_drag_modal_capture() {
    let mut state = NativePlayerUiState::default();
    state.help.show();
    assert!(state.blocks_world_pointer_at(300.0, 200.0));
    assert!(!state.blocks_world_pointer_at(900.0, 600.0));
    assert!(!state.blocks_route_navigation());
    state.help.left = 0.0;
    state.help.top = 0.0;
    assert!(state.blocks_world_pointer_at(200.0, 100.0));
    assert!(!state.blocks_world_pointer_at(700.0, 500.0));
    state.help.dragging = true;
    assert!(state.blocks_world_action(false, false));
    assert!(state.blocks_world_pointer_at(700.0, 500.0));
}

#[test]
fn commerce_and_real_prompts_keep_global_action_capture() {
    for panel in [
        mir2_ui_core::state::UiPanel::Storage,
        mir2_ui_core::state::UiPanel::NpcShop,
        mir2_ui_core::state::UiPanel::GameShop,
        mir2_ui_core::state::UiPanel::Trade,
    ] {
        let mut state = NativePlayerUiState::default();
        state.core.panel = panel;
        assert!(state.blocks_world_action(false, false), "{panel:?}");
        assert!(state.blocks_world_pointer_at(700.0, 600.0));
    }
    let mut state = NativePlayerUiState::default();
    state.toggle_skill();
    assert!(state.blocks_world_action(true, false));
    assert!(state.blocks_world_action(false, true));
    state.inventory_window.dragging = true;
    assert!(state.blocks_world_action(false, false));
}
