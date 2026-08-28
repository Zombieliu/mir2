//! Crystal cursor parity for the native world viewport.
//!
//! The authoritative hover id still comes from the alpha-tested native entity
//! presentation. This module only maps that read model to Crystal's original
//! cursor bitmaps; it never changes selection or gameplay state.

use bevy::prelude::{
    AssetServer, ButtonInput, Commands, Entity, Handle, Image, KeyCode, Query, Res, ResMut,
    Resource, Window, With,
};
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage, PrimaryWindow};
use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiState;
use mir2_client_bevy::entities::{EntityKind, EntityModelSet};
use mir2_client_bevy::native_shell::{NativeShellModel, NativeShellScreen};
use mir2_client_bevy::quest_model::NpcDialogModel;
use mir2_client_bevy::read_model::UiReadModel;

use crate::entity_presentation::NativeEntityPresentation;
use crate::input::is_world_click_blocked;

const DEFAULT_CURSOR: &str = "original-ui/Cursors/Cursor_Default.png";
const ATTACK_CURSOR: &str = "original-ui/Cursors/Cursor_Normal_Atk.png";
const ATTACK_RED_CURSOR: &str = "original-ui/Cursors/Cursor_Compulsion_Atk.png";
const NPC_CURSOR: &str = "original-ui/Cursors/Cursor_Npc.png";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrystalCursorKind {
    Default,
    Attack,
    AttackRed,
    NpcTalk,
}

#[derive(Resource)]
pub(crate) struct NativeCrystalCursors {
    default: Handle<Image>,
    attack: Handle<Image>,
    attack_red: Handle<Image>,
    npc_talk: Handle<Image>,
    applied: Option<CrystalCursorKind>,
}

pub(crate) fn load_native_crystal_cursors(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(NativeCrystalCursors {
        default: asset_server.load(DEFAULT_CURSOR),
        attack: asset_server.load(ATTACK_CURSOR),
        attack_red: asset_server.load(ATTACK_RED_CURSOR),
        npc_talk: asset_server.load(NPC_CURSOR),
        applied: None,
    });
}

fn cursor_kind_for_hover(kind: Option<EntityKind>, shift_pressed: bool) -> CrystalCursorKind {
    match kind {
        Some(EntityKind::Monster) => CrystalCursorKind::Attack,
        Some(EntityKind::Npc) => CrystalCursorKind::NpcTalk,
        Some(EntityKind::Player) if shift_pressed => CrystalCursorKind::AttackRed,
        Some(EntityKind::Player | EntityKind::SelfPlayer) | None => CrystalCursorKind::Default,
    }
}

fn custom_cursor(handle: Handle<Image>) -> CursorIcon {
    CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
        handle,
        texture_atlas: None,
        flip_x: false,
        flip_y: false,
        rect: None,
        // All four source .CUR directory entries declare Crystal's original
        // top-left (0,0) hotspot.
        hotspot: (0, 0),
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_native_crystal_cursor(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    shell: Option<Res<NativeShellModel>>,
    player_ui: Option<Res<NativePlayerUiState>>,
    dialog: Option<Res<NpcDialogModel>>,
    ui_read_model: Option<Res<UiReadModel>>,
    entities: Option<Res<EntityModelSet>>,
    presentation: Option<Res<NativeEntityPresentation>>,
    mut cursors: Option<ResMut<NativeCrystalCursors>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
) {
    let Ok((window_entity, window)) = windows.single() else {
        return;
    };
    let Some(cursors) = cursors.as_deref_mut() else {
        return;
    };

    let dialog_open = dialog.as_deref().is_some_and(|dialog| dialog.is_open);
    let dead = ui_read_model
        .as_deref()
        .is_some_and(|model| model.player.max_hp > 0 && model.player.hp <= 0);
    let world_cursor_enabled = window.focused
        && shell
            .as_deref()
            .is_some_and(|shell| shell.screen == NativeShellScreen::InGame)
        && !is_world_click_blocked(player_ui.as_deref(), dialog_open, dead);

    let hovered_kind = world_cursor_enabled
        .then(|| {
            presentation
                .as_deref()
                .and_then(NativeEntityPresentation::hovered_object_id)
                .and_then(|object_id| {
                    entities.as_deref().and_then(|entities| {
                        entities
                            .entities
                            .iter()
                            .find(|entity| entity.object_id == object_id)
                            .map(|entity| entity.kind)
                    })
                })
        })
        .flatten();
    let shift_pressed = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let desired = cursor_kind_for_hover(hovered_kind, shift_pressed);
    if cursors.applied == Some(desired) {
        return;
    }

    let handle = match desired {
        CrystalCursorKind::Default => cursors.default.clone(),
        CrystalCursorKind::Attack => cursors.attack.clone(),
        CrystalCursorKind::AttackRed => cursors.attack_red.clone(),
        CrystalCursorKind::NpcTalk => cursors.npc_talk.clone(),
    };
    commands.entity(window_entity).insert(custom_cursor(handle));
    cursors.applied = Some(desired);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_hover_cursor_matrix_matches_client_source() {
        assert_eq!(
            cursor_kind_for_hover(Some(EntityKind::Monster), false),
            CrystalCursorKind::Attack
        );
        assert_eq!(
            cursor_kind_for_hover(Some(EntityKind::Npc), false),
            CrystalCursorKind::NpcTalk
        );
        assert_eq!(
            cursor_kind_for_hover(Some(EntityKind::Player), true),
            CrystalCursorKind::AttackRed
        );
        assert_eq!(
            cursor_kind_for_hover(Some(EntityKind::Player), false),
            CrystalCursorKind::Default
        );
        assert_eq!(
            cursor_kind_for_hover(Some(EntityKind::SelfPlayer), true),
            CrystalCursorKind::Default
        );
        assert_eq!(
            cursor_kind_for_hover(None, false),
            CrystalCursorKind::Default
        );
    }
}
