use super::*;
use crate::crystal_ui::skill_page_shared::{SkillPageAction, assignment_label};
pub use crate::skill_page_state::{SkillAssignUi, SkillAuthorityUi};
impl SkillAssignUi {
    // Native alone retains the existing process-wide player/Hero namespace.
    pub fn save(&mut self, queue: &mut NativePlayerUiIntentQueue) {
        if !self.open || self.pending {
            return;
        }
        self.request_id = crate::skill_model::next_skill_key_request_id();
        let accepted = queue.push_intent(NativePlayerUiIntent::MagicKey {
            request_id: self.request_id,
            spell: self.spell.clone(),
            key: self.key,
            old_key: self.old_key,
        });
        self.queued(accepted);
    }
}
fn apply_source_key(skills: &mut SkillModel, id: u32, key: u8) {
    if let Some(spell) = skills.binding_for(id).spell {
        crate::skill_page_state::apply_source_key(skills, id, &spell, key);
    }
}
pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut binding: ResMut<SkillBindingUi>,
    mut skills: ResMut<SkillModel>,
    shell: Res<NativeShellModel>,
) {
    if shell.screen != NativeShellScreen::InGame {
        state.skill_assign = Default::default();
        state.skill_authority = Default::default();
        return;
    }
    if !state.skill_assign.open {
        return;
    }
    if !state.skill_assign.valid(&skills) {
        state.skill_assign = Default::default();
        binding.set_assign_key(false);
        binding.refresh(&skills);
        return;
    }
    let Some(success) = state.skill_assign.result.take() else {
        return;
    };
    state.skill_assign.pending = false;
    if !success {
        state.skill_assign.notice = Some("Unable to send. Please try again.".into());
        return;
    }
    let draft = state.skill_assign.clone();
    state.skill_authority.begin(&skills, draft.clone());
    crate::skill_page_state::apply_source_key(&mut skills, draft.skill_id, &draft.spell, draft.key);
    binding.refresh(&skills);
    binding.set_assign_key(false);
    state.skill_assign = Default::default();
}
#[derive(Component)]
pub(super) struct AssignRoot;
pub(super) fn map_action(a: SkillPageAction) -> OverlayButton {
    match a {
        SkillPageAction::Prev => OverlayButton::SkillPagePrev,
        SkillPageAction::Next => OverlayButton::SkillPageNext,
        SkillPageAction::Select(id) => OverlayButton::SelectSkill(id),
        SkillPageAction::Choose(key) => OverlayButton::AssignSkillKey(key),
        SkillPageAction::Clear => OverlayButton::ClearSkillBinding,
        SkillPageAction::Save => OverlayButton::CloseSkillAssign,
    }
}
pub(super) fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<AssignRoot>>,
    state: Res<NativePlayerUiState>,
    skills: Res<SkillModel>,
    assets: Option<Res<AssetServer>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let (true, Some(assets), Ok(root)) =
        (state.skill_assign.valid(&skills), assets, roots.single())
    else {
        return;
    };
    commands.entity(root).with_children(|p| {
        crate::crystal_ui::skill_page_shared::paint_assignment(
            p,
            &assets,
            &skills,
            &state.skill_assign,
            None,
            AssignRoot,
            map_action,
        );
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assignment_labels_match_original_fixed_two_line_strings() {
        assert_eq!(assignment_label(1), "F1");
        assert_eq!(assignment_label(8), "F8");
        assert_eq!(assignment_label(9), "Ctrl\nF1");
        assert_eq!(assignment_label(16), "Ctrl\nF8");
        assert_eq!(assignment_label(0), "");
    }
    #[test]
    fn assignment_render_uses_original_frame_and_button_dimensions() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        let skills = model();
        let mut state = NativePlayerUiState::default();
        state.skill_assign.show(1, &skills);
        app.insert_resource(skills).insert_resource(state);
        app.world_mut().spawn((Node::default(), OverlayRoot));
        app.world_mut().run_system_once(render).unwrap();
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<AssignRoot>>();
        let root = roots.single(world).unwrap();
        assert_eq!(
            (root.left, root.top, root.width, root.height),
            (Val::Px(322.), Val::Px(312.), Val::Px(380.), Val::Px(144.))
        );
        let texts: Vec<_> = world
            .query::<&Text>()
            .iter(world)
            .map(|v| v.0.clone())
            .collect();
        assert!(texts.contains(&"Ctrl\nF1".into()));
        assert!(!texts.contains(&"Ctrl+F1".into()));
        for (action, node) in world.query::<(&OverlayButton, &Node)>().iter(world) {
            match action {
                OverlayButton::AssignSkillKey(_) => {
                    assert_eq!((node.width, node.height), (Val::Px(32.), Val::Px(32.)))
                }
                OverlayButton::ClearSkillBinding => {
                    assert_eq!((node.width, node.height), (Val::Px(76.), Val::Px(25.)))
                }
                OverlayButton::CloseSkillAssign => {
                    assert_eq!((node.width, node.height), (Val::Px(60.), Val::Px(25.)))
                }
                _ => {}
            }
        }
    }
    fn model() -> SkillModel {
        serde_json::from_value(serde_json::json!({"skills":[{"id":1,"name":"FireBall","spell":"FireBall","hotkey":1,"castKind":"target","icon":0},{"id":2,"name":"Healing","spell":"Healing","hotkey":16,"castKind":"self"}]})).unwrap()
    }
    #[test]
    fn only_exact_processed_receipt_retires_overlay_not_any_count_of_snapshots() {
        let base = model();
        let mut draft = SkillAssignUi::default();
        draft.show(1, &base);
        draft.key = 16;
        draft.request_id = 41;
        let mut pending = SkillAuthorityUi::default();
        pending.begin(&base, draft);
        for serial in 1..=100 {
            let mut stale = base.clone();
            stale.authority.snapshot_serial = serial;
            assert!(pending.reconcile(&mut stale).is_none());
            assert_eq!(stale.binding_for(1).hotkey, Some(16));
            assert!(pending.pending.is_some());
        }
        let mut wrong = base.clone();
        wrong.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
            request_id: 40,
            spell: "FireBall".into(),
            key: 16,
            old_key: 1,
            accepted: true,
        });
        pending.reconcile(&mut wrong);
        assert!(pending.pending.is_some());
        let mut accepted = base.clone();
        apply_source_key(&mut accepted, 1, 16);
        accepted.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
            request_id: 41,
            spell: "FireBall".into(),
            key: 16,
            old_key: 1,
            accepted: true,
        });
        assert!(pending.reconcile(&mut accepted).is_none());
        assert!(pending.pending.is_none());
        let mut later = base.clone();
        pending.reconcile(&mut later);
        assert_eq!(
            later.binding_for(1).hotkey,
            Some(1),
            "once settled, every later server key is authoritative"
        );
    }
    #[test]
    fn rejected_receipt_restores_exact_draft_and_epoch_change_drops_old_request() {
        let base = model();
        let mut draft = SkillAssignUi::default();
        draft.show(1, &base);
        draft.key = 16;
        draft.request_id = 51;
        let mut pending = SkillAuthorityUi::default();
        pending.begin(&base, draft.clone());
        let mut rejected = base.clone();
        rejected.skill_key_ack = Some(crate::skill_model::SkillKeyAck {
            request_id: 51,
            spell: "FireBall".into(),
            key: 16,
            old_key: 1,
            accepted: false,
        });
        let restored = pending.reconcile(&mut rejected).unwrap();
        assert!(restored.open);
        assert_eq!(restored.key, 16);
        assert!(!restored.pending);
        assert_eq!(rejected.binding_for(1).hotkey, Some(1));
        pending.begin(&base, draft);
        let mut other = base.clone();
        other.authority.session_epoch += 1;
        assert!(pending.reconcile(&mut other).is_none());
        assert!(pending.pending.is_none());
        assert_eq!(other.binding_for(1).hotkey, Some(1));
    }
    #[test]
    fn selection_is_a_draft_save_sends_source_oldkey_and_failed_send_retains_it() {
        let skills = model();
        let mut ui = SkillAssignUi::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        ui.show(1, &skills);
        ui.choose(16);
        assert_eq!(skills.binding_for(1).hotkey, Some(1));
        ui.save(&mut queue);
        ui.save(&mut queue);
        assert_eq!(
            queue.drain_intents(),
            vec![NativePlayerUiIntent::MagicKey {
                request_id: ui.request_id,
                spell: "FireBall".into(),
                key: 16,
                old_key: 1
            }]
        );
        ui.dispatched(ui.request_id, "Healing", 16, 1, true);
        assert_eq!(ui.result, None);
        ui.dispatched(ui.request_id, "FireBall", 16, 1, false);
        assert_eq!(ui.result, Some(false));
        assert!(ui.open);
        assert_eq!(ui.key, 16);
    }
    #[test]
    fn committing_slot16_clears_occupant_without_swapping_and_none_is_explicit_zero() {
        let mut app = App::new();
        app.insert_resource(model())
            .init_resource::<SkillBindingUi>()
            .init_resource::<NativePlayerUiState>()
            .insert_resource(SkillBindingPersistenceRuntime::with_config_path(
                std::env::temp_dir().join(format!("mir2-skill16-test-{}.json", std::process::id())),
            ))
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .add_systems(Update, process);
        let skills = app.world().resource::<SkillModel>().clone();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.skill_assign.show(1, &skills);
            state.skill_assign.key = 16;
            state.skill_assign.result = Some(true);
        }
        app.update();
        assert_eq!(
            app.world().resource::<SkillModel>().binding_for(1).hotkey,
            Some(16)
        );
        assert_eq!(
            app.world().resource::<SkillModel>().binding_for(2).hotkey,
            Some(0)
        );
        assert!(
            app.world()
                .resource::<SkillModel>()
                .skill_for_shortcut(1)
                .is_none()
        );
        let skills = app.world().resource::<SkillModel>().clone();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.skill_assign.show(1, &skills);
            state.skill_assign.choose(0);
            state.skill_assign.result = Some(true);
        }
        app.update();
        assert_eq!(
            app.world().resource::<SkillModel>().binding_for(1).hotkey,
            Some(0)
        );
        assert!(
            app.world()
                .resource::<SkillModel>()
                .skill_for_shortcut(16)
                .is_none()
        );
        let path = app
            .world()
            .resource::<SkillBindingPersistenceRuntime>()
            .config_path
            .clone();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.bak"));
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));
    }
    #[test]
    fn sixteen_player_bindings_roundtrip_without_hero_slots() {
        let mut model = model();
        model.skills.clear();
        model.bindings.clear();
        for slot in 1..=16u8 {
            model.skills.push(crate::skill_model::SkillEntry {
                id: u32::from(slot),
                name: format!("Skill{slot}"),
                level: 1,
                key: None,
                cooldown_ms: 0,
                mp_cost: 0,
            });
        }
        let mut ui = SkillBindingUi::default();
        ui.set_assign_key(true);
        for slot in 1..=16u8 {
            assert!(ui.assign_key_to_skill(u32::from(slot), slot, &model));
        }
        let loaded: SkillBindingUi =
            serde_json::from_str(&serde_json::to_string(&ui).unwrap()).unwrap();
        assert_eq!(loaded.bindings.len(), 16);
        assert_eq!(loaded.skill_for_hotkey(16), Some(16));
        assert!(loaded.skill_for_hotkey(17).is_none());
    }
}
