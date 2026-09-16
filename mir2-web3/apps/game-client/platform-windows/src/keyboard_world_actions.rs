use super::*;
use bevy::prelude::Local;
use mir2_client_bevy::crystal_ui::overlays::{keyboard_dialog, NativePlayerUiIntentQueue};
use mir2_client_bevy::social::SocialModel;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct WorldShortcutState {
    owner: Option<String>,
    pickup_ready: Option<Instant>,
}
impl WorldShortcutState {
    fn pickup(&mut self, now: Instant) -> bool {
        if self.pickup_ready.is_some_and(|ready| now <= ready) {
            return false;
        }
        self.pickup_ready = Some(now + Duration::from_millis(200));
        true
    }
}

pub fn keyboard_world_actions_system(
    keys: Res<ButtonInput<KeyCode>>,
    commands: Res<GatewayCommands>,
    shell: Option<Res<NativeShellModel>>,
    mut ui: Option<ResMut<NativePlayerUiState>>,
    notice: Option<Res<NoticeDialogState>>,
    windows: Query<&Window>,
    entities: Res<EntityModelSet>,
    presentation: Res<NativeEntityPresentation>,
    mut social: Option<ResMut<SocialModel>>,
    mut intents: Option<ResMut<NativePlayerUiIntentQueue>>,
    mut state: Local<WorldShortcutState>,
    mut chat: Option<ResMut<mir2_client_bevy::chat::ChatModel>>,
) {
    if let Some(ui) = ui.as_deref_mut() {
        if let Some(notice) = ui.local_keys.auto_run_notice.take() {
            if let Some(chat) = chat.as_deref_mut() {
                chat.push(mir2_client_bevy::chat::ChatLine {
                    text: notice.into(),
                    channel: "hint".into(),
                });
            }
        }
    }
    if !shell
        .as_deref()
        .is_some_and(|s| s.screen == NativeShellScreen::InGame)
    {
        *state = Default::default();
        return;
    }
    let owner = entities
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::SelfPlayer);
    let identity = owner.map(|e| e.object_id.clone());
    if state.owner != identity {
        *state = WorldShortcutState {
            owner: identity,
            ..Default::default()
        };
    }
    if !gameplay_input_enabled(shell.as_deref(), ui.as_deref(), notice.as_deref(), &windows) {
        return;
    }
    let Some(ui) = ui.as_deref_mut() else {
        return;
    };
    let triggered = |name: &str| keyboard_dialog::host::triggered(&ui.keyboard, &keys, name);
    let group = triggered("AddGroupMember");
    // Keep the source Keylist order, with a shared 200 ms gate for both modes.
    let pickup_modes: Vec<_> = ui
        .keyboard
        .bindings
        .iter()
        .filter_map(|b| {
            let mouse = match b.function.as_str() {
                "CreaturePickup" => true,
                "CreatureAutoPickup" => false,
                _ => return None,
            };
            triggered(&b.function).then_some(mouse)
        })
        .collect();
    for mouse_mode in pickup_modes {
        let Some((x, y)) = presentation.hovered_grid_position() else {
            continue;
        };
        if state.pickup(Instant::now())
            && !commands.send_command(GatewayCommand::Wire(
                NativeOutboundCommand::IntelligentCreaturePickup {
                    mouse_mode,
                    location: mir2_protocol::Point { x, y },
                },
            ))
        {
            state.pickup_ready = None;
        }
    }
    if group {
        let target = presentation.hovered_object_id().and_then(|id| {
            entities.entities.iter().find(|e| {
                e.object_id == id && matches!(e.kind, EntityKind::Player | EntityKind::SelfPlayer)
            })
        });
        if let (Some(target), Some(owner), Some(social), Some(intents)) =
            (target, owner, social.as_deref_mut(), intents.as_deref_mut())
        {
            ui.group_dialog
                .invite_named(target.name.clone(), &owner.name, social, intents);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{App, Update};
    use mir2_client_bevy::crystal_ui::overlays::NativePlayerUiIntent;
    fn app() -> (App, std::sync::mpsc::Receiver<GatewayCommand>) {
        let (mut app, rx) = super::super::tests::input_app();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<SocialModel>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .add_systems(Update, keyboard_world_actions_system);
        app.world_mut()
            .resource_mut::<NativeEntityPresentation>()
            .set_hover_grid_context_for_test((100, 200), (512., 384.));
        (app, rx)
    }
    #[test]
    fn creature_pickup_preserves_original_mouse_mode_and_point() {
        let command = NativeOutboundCommand::IntelligentCreaturePickup {
            mouse_mode: false,
            location: mir2_protocol::Point { x: 42, y: 57 },
        };
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"type":"intelligentCreaturePickup","mouseMode":false,"location":{"x":42,"y":57}})
        );
    }

    #[test]
    fn source_pickup_modes_share_strict_gate() {
        let now = Instant::now();
        let mut state = WorldShortcutState::default();
        assert!(state.pickup(now));
        assert!(!state.pickup(now + Duration::from_millis(200)));
        assert!(state.pickup(now + Duration::from_millis(201)));
    }
    #[test]
    fn pickup_uses_cursor_location_and_modal_does_not_send() {
        let (mut app, rx) = app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyX);
        let location = app
            .world()
            .resource::<NativeEntityPresentation>()
            .hovered_grid_position()
            .unwrap();
        app.update();
        assert!(
            matches!(rx.try_recv(),Ok(GatewayCommand::Wire(NativeOutboundCommand::IntelligentCreaturePickup{mouse_mode:true,location:p})) if (p.x,p.y)==location)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .group_dialog
            .invitation = Some(("Peer".into(), 1));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyX);
        app.update();
        assert!(rx.try_recv().is_err());
    }
    #[test]
    fn group_shortcut_rejects_monsters_and_nonleaders() {
        let (mut app, _) = app();
        let mut target = app.world().resource::<EntityModelSet>().entities[0].clone();
        target.object_id = "peer".into();
        target.name = "Peer".into();
        target.kind = EntityKind::Monster;
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .push(target);
        app.world_mut()
            .resource_mut::<NativeEntityPresentation>()
            .set_hovered_object_id_for_test(Some("peer"));
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyG);
        }
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        app.world_mut().resource_mut::<EntityModelSet>().entities[1].kind = EntityKind::Player;
        app.world_mut()
            .resource_mut::<SocialModel>()
            .group
            .members
            .push(mir2_client_bevy::social::GroupMemberModel {
                name: "OtherLeader".into(),
                ..Default::default()
            });
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .group_dialog
                .notice
                .as_deref(),
            Some("You are not the leader of your group.")
        );
    }

    #[test]
    fn group_shortcut_uses_hovered_player_and_keeps_original_add_packet() {
        let (mut app, _) = app();
        let mut target = app.world().resource::<EntityModelSet>().entities[0].clone();
        target.object_id = "peer".into();
        target.name = "Peer".into();
        target.kind = EntityKind::Player;
        app.world_mut()
            .resource_mut::<EntityModelSet>()
            .entities
            .push(target);
        app.world_mut()
            .resource_mut::<NativeEntityPresentation>()
            .set_hovered_object_id_for_test(Some("peer"));
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyG);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::GroupAddMember {
                name: "Peer".into()
            }]
        );
    }
}
