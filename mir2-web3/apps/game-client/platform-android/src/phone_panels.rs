//! A phone workspace shared by the panel and HUD presentation adapters.
//! All coordinates are Android logical pixels. This does not own game state.
use bevy::prelude::*;
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;

const GUTTER: f32 = 16.0;
const SIDE_WIDTH: f32 = 208.0;
const TAP: f32 = 48.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PanelSidebar {
    pub workspace: Rect,
    pub status: Rect,
    pub belt: Rect,
    pub chat: Rect,
}

pub(crate) fn sidebar_requested(player: &NativePlayerUiState) -> bool {
    // Storage may expose both source windows for transfers. Do not give two
    // independent panels the same workspace, or move its authored drag targets.
    !player.storage_open()
        && !player.trade_dialog.open
        && !player.chat_focused()
        && (player.core.inventory_open() || player.core.equipment_open())
}

/// Preserve a left HUD lane and the actual right action-pad footprint. Use a
/// 3x2 belt in this lane instead of hiding the six shared use targets. Short
/// landscape/IME layouts retain their existing presentation; this leaf does
/// not pretend a desktop panel can meet phone tap sizes at every resolution.
pub(crate) fn panel_sidebar(
    viewport: Vec2,
    safe: Vec4,
    ime: f32,
    requested: bool,
) -> Option<PanelSidebar> {
    if !requested || ime > 0.0 || viewport.x < 760.0 || viewport.y < 368.0 {
        return None;
    }
    let left = safe.x + GUTTER;
    let top = safe.y + 20.0;
    let status = Rect::new(left, top, left + SIDE_WIDTH, top + 76.0);
    let belt_left = left + (SIDE_WIDTH - TAP * 3.0) * 0.5;
    let belt = Rect::new(
        belt_left,
        status.max.y + 6.0,
        belt_left + TAP * 3.0,
        status.max.y + 6.0 + TAP * 2.0,
    );
    let chat = Rect::new(
        left,
        belt.max.y + 6.0,
        left + SIDE_WIDTH,
        belt.max.y + 6.0 + 18.0 + TAP + 8.0,
    );
    let thumbs = crate::mobile_ui::thumb_footprints(viewport.y);
    let joystick_diameter = thumbs.x - 24.0 - GUTTER;
    let joystick_top = viewport.y - safe.w - 20.0 - joystick_diameter;
    let workspace = Rect::new(
        left + SIDE_WIDTH + GUTTER,
        safe.y + GUTTER,
        viewport.x - safe.z - thumbs.y,
        viewport.y - safe.w - GUTTER,
    );
    if workspace.width() < 400.0 || workspace.height() < 320.0 || chat.max.y + 8.0 > joystick_top {
        return None;
    }
    Some(PanelSidebar {
        workspace,
        status,
        belt,
        chat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_phone_workspace_keeps_all_six_belt_targets_and_both_thumbs() {
        for (viewport, safe) in [
            (Vec2::new(851.0, 393.0), Vec4::ZERO),
            (Vec2::new(851.0, 393.0), Vec4::new(12.0, 8.0, 22.0, 12.0)),
            (Vec2::new(960.0, 432.0), Vec4::new(24.0, 12.0, 24.0, 12.0)),
        ] {
            let layout = panel_sidebar(viewport, safe, 0.0, true).unwrap();
            for chrome in [layout.status, layout.belt, layout.chat] {
                assert!(layout.workspace.intersect(chrome).is_empty());
                assert!(chrome.min.x >= safe.x && chrome.min.y >= safe.y);
            }
            assert_eq!(layout.belt.size(), Vec2::new(144.0, 96.0));
            assert!(layout.chat.width() - 3.0 * TAP - 8.0 >= TAP);
            let thumbs = crate::mobile_ui::thumb_footprints(viewport.y);
            let joy = Rect::new(
                safe.x + 24.0,
                viewport.y - safe.w - 20.0 - (thumbs.x - 40.0),
                safe.x + thumbs.x - GUTTER,
                viewport.y - safe.w - 20.0,
            );
            let pad = Rect::new(
                viewport.x - safe.z - thumbs.y + GUTTER,
                viewport.y - safe.w - 128.0,
                viewport.x - safe.z - 20.0,
                viewport.y - safe.w - 20.0,
            );
            assert!(layout.workspace.intersect(joy).is_empty());
            assert!(layout.workspace.intersect(pad).is_empty());
            assert!(layout.chat.intersect(joy).is_empty());
        }
    }

    #[test]
    fn unsupported_compact_keyboard_and_two_window_transfer_remain_explicit() {
        assert!(panel_sidebar(Vec2::new(568.0, 262.0), Vec4::ZERO, 0.0, true).is_none());
        assert!(panel_sidebar(Vec2::new(851.0, 393.0), Vec4::ZERO, 170.0, true).is_none());
        assert!(panel_sidebar(Vec2::new(851.0, 393.0), Vec4::ZERO, 0.0, false).is_none());
        let mut player = NativePlayerUiState::default();
        player.core.screen = mir2_ui_core::state::UiScreen::InGame;
        player.core.panel = mir2_ui_core::state::UiPanel::Character;
        assert!(sidebar_requested(&player));
        player.core.panel = mir2_ui_core::state::UiPanel::Storage;
        assert!(!sidebar_requested(&player));
    }
}
