use super::*;
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupDialogUi {
    pub remove: bool,
    pub invitation: Option<(String, u64)>,
    pub answered: Option<(String, u64)>,
    pub editor: friend_dialog::FriendDialogUi,
    pub position: Option<Vec2>,
    pub consumed: bool,
    pub notice: Option<String>,
    pub submitted: Option<(bool, String)>,
}
impl GroupDialogUi {
    pub fn restore_invitation(
        &mut self,
        identity: &(String, u64),
        group: &crate::social::GroupModel,
    ) {
        if self.answered.as_ref() != Some(identity) {
            return;
        }
        self.answered = None;
        if self.invitation.is_none()
            && group.pending_invite_from.as_ref() == Some(&identity.0)
            && group.pending_invite_epoch == identity.1
        {
            self.invitation = Some(identity.clone());
        }
    }
    pub fn modal(&self) -> bool {
        self.editor.modal.is_some() || self.invitation.is_some()
    }
    pub fn can_manage(group: &crate::social::GroupModel, owner: &str) -> bool {
        group.members.first().is_none_or(|m| m.name == owner)
    }
    /// Crystal GroupDialog.AddMember(string), used by the hovered-player key.
    pub fn invite_named(
        &mut self,
        name: String,
        owner: &str,
        social: &mut crate::social::SocialModel,
        intents: &mut NativePlayerUiIntentQueue,
    ) -> bool {
        if social.group.members.len() >= crate::social::MAX_GROUP_MEMBERS {
            self.notice = Some("Your group already has the maximum number of members.".into());
            return false;
        }
        if !Self::can_manage(&social.group, owner) {
            self.notice = Some("You are not the leader of your group.".into());
            return false;
        }
        intents.push_social_pending(social, NativePlayerUiIntent::GroupAddMember { name })
    }
    pub fn show_input(&mut self, remove: bool, group: &crate::social::GroupModel, owner: &str) {
        if !remove && group.members.len() >= crate::social::MAX_GROUP_MEMBERS {
            self.notice = Some("Your group already has the maximum number of members.".into());
            return;
        }
        if !Self::can_manage(group, owner) {
            self.notice = Some("You are not the leader of your group.".into());
            return;
        }
        self.submitted = None;
        self.remove = remove;
        self.editor = Default::default();
        self.editor.open = true;
        self.editor.modal = Some(friend_dialog::FriendModal::Add {
            blocked: false,
            text: String::new(),
        });
        self.editor.sync_editor();
    }
    pub fn rect(&self) -> CrystalRect {
        let mut r = CRYSTAL_GROUP_PANEL_RECT;
        if let Some(p) = self.position {
            r.left = p.x;
            r.top = p.y;
        }
        r
    }
    pub fn submit(
        &mut self,
        intents: &mut NativePlayerUiIntentQueue,
        social: &mut crate::social::SocialModel,
    ) {
        let Some(text) = self.editor.editor.as_ref().map(|e| e.text().to_owned()) else {
            return;
        };
        let retained = text.clone();
        let intent = if self.remove {
            NativePlayerUiIntent::GroupRemoveMember { name: text }
        } else {
            NativePlayerUiIntent::GroupAddMember { name: text }
        };
        if intents.push_social_pending(social, intent) {
            self.submitted = Some((self.remove, retained));
            self.editor.cancel_modal();
            self.consumed = true;
        } else {
            self.editor.edit_notice = Some("Unable to send. Please try again.".into());
        }
    }
    pub fn restore_unsent(&mut self, remove: bool, text: &str) {
        if self.submitted.as_ref() != Some(&(remove, text.to_owned())) {
            return;
        }
        self.submitted = None;
        if self.modal() {
            return;
        }
        self.remove = remove;
        self.editor = Default::default();
        self.editor.open = true;
        self.editor.modal = Some(friend_dialog::FriendModal::Add {
            blocked: false,
            text: text.into(),
        });
        self.editor.sync_editor();
        self.editor.edit_notice = Some("Unable to send. Please try again.".into());
    }
    pub fn keyboard(
        &mut self,
        events: &[KeyboardInput],
        intents: &mut NativePlayerUiIntentQueue,
        social: &mut crate::social::SocialModel,
    ) {
        self.consumed = true;
        for e in events {
            if !self.modal() {
                break;
            }
            if e.state == ButtonState::Pressed && !e.repeat {
                if e.key_code == KeyCode::Escape {
                    self.editor.cancel_modal();
                    continue;
                }
                if matches!(e.key_code, KeyCode::Enter | KeyCode::NumpadEnter) {
                    self.submit(intents, social);
                    continue;
                }
            }
            friend_dialog::host::edit_key(&mut self.editor, e);
        }
    }
}
#[derive(Default, Resource)]
pub struct GroupHost {
    drag: Option<Vec2>,
    selection: bool,
}
#[derive(Component)]
pub(super) struct GroupInputPanel;
pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<GroupHost>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    shell: Option<Res<NativeShellModel>>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
    social: Res<crate::social::SocialModel>,
) {
    if shell
        .as_ref()
        .is_none_or(|s| s.screen != NativeShellScreen::InGame)
    {
        state.group_dialog = Default::default();
        *host = Default::default();
        return;
    }
    if let Some(name) = &social.group.pending_invite_from {
        let identity = (name.clone(), social.group.pending_invite_epoch);
        if state.group_dialog.answered.as_ref() != Some(&identity) {
            state.group_dialog.invitation = Some(identity);
        }
    } else {
        state.group_dialog.invitation = None;
        state.group_dialog.answered = None;
    }
    state.group_dialog.consumed = state.group_dialog.modal();
    if let (Some(text), Some(chat)) = (state.group_dialog.notice.take(), chat.as_deref_mut()) {
        chat.push(crate::chat::ChatLine {
            text,
            channel: "system".into(),
        });
    }
    let (Some(mouse), Ok(window)) = (mouse, windows.single()) else {
        return;
    };
    if !window.focused || shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        *host = Default::default();
        state.group_dialog.editor.modifiers = [false; 4];
        return;
    }
    if state.leave_game.blocks() || state.social_bonds.prompt.is_some() || state.keyboard.open {
        *host = Default::default();
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    if state.group_dialog.invitation.is_some() {
        return;
    }
    if state.group_dialog.modal() {
        let local = cursor - Vec2::new(368., 306.) - Vec2::new(23., 86.);
        if mouse.just_pressed(MouseButton::Left) {
            let focused = CrystalRect::new(0., 0., 240., 19.).contains(local.x, local.y);
            state.group_dialog.editor.editor_focused = focused;
            if focused {
                friend_dialog::host::focus_editor(&mut state.group_dialog.editor, local, false);
                host.selection = true;
            }
        }
        if mouse.pressed(MouseButton::Left) && host.selection {
            friend_dialog::host::focus_editor(&mut state.group_dialog.editor, local, true);
        }
        if !mouse.pressed(MouseButton::Left) {
            host.selection = false;
        }
        return;
    }
    if state.amount_modal_open() || !state.group_open() {
        host.drag = None;
        return;
    }
    let rect = state.group_dialog.rect();
    if mouse.just_pressed(MouseButton::Left)
        && CrystalRect::new(rect.left, rect.top, 205., 30.).contains(cursor.x, cursor.y)
    {
        host.drag = Some(cursor - Vec2::new(rect.left, rect.top));
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = host.drag {
            state.group_dialog.position = Some(cursor - offset);
        }
    } else {
        host.drag = None;
    }
}
pub(super) fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<GroupInputPanel>>,
    mut state: ResMut<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
    blocks: Query<(
        &friend_dialog::view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    if state.group_dialog.editor.modal.is_none() {
        return;
    }
    for (tag, block, info) in &blocks {
        friend_dialog::host::capture_layout(&mut state.group_dialog.editor, tag, block, info);
    }
    let (Some(assets), Ok(root)) = (assets, roots.single()) else {
        return;
    };
    commands.entity(root).with_children(|root| {
        root.spawn((
            GroupInputPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(368.),
                top: Val::Px(306.),
                width: Val::Px(288.),
                height: Val::Px(156.),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(1099),
        ))
        .with_children(|p| {
            spawn_overlay_frame(p, &assets, "original-ui/Prguse/660.png", 288., 156.);
            friend_dialog::view::wrapped_text(
                p,
                if state.group_dialog.remove {
                    "Please enter the name of the person you wish to remove."
                } else {
                    "Please enter the name of the person you wish to add."
                },
                CrystalRect::new(25., 25., 235., 40.),
                Color::WHITE,
            );
            friend_dialog::view::render_editor(
                p,
                &state.group_dialog.editor,
                CrystalRect::new(23., 86., 240., 19.),
                false,
            );
            for (index, x, action) in [
                (200, 60., OverlayButton::GroupInputConfirm),
                (203, 160., OverlayButton::GroupInputCancel),
            ] {
                let spec = CrystalButtonSpec::new(
                    "Title",
                    index,
                    index + 1,
                    index + 2,
                    CrystalRect::new(x, 123., 76., 25.),
                    76.,
                    25.,
                );
                spawn_crystal_image_button(
                    p,
                    &assets,
                    spec,
                    CrystalButtonAssetSet::from_spec(spec),
                    action,
                    false,
                    true,
                );
            }
            if let Some(text) = &state.group_dialog.editor.edit_notice {
                friend_dialog::view::wrapped_text(
                    p,
                    text,
                    CrystalRect::new(15., 158., 258., 40.),
                    Color::srgb_u8(255, 90, 70),
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_invitation_accept_shows_group_and_rejects_stale_epoch() {
        let mut state = NativePlayerUiState::default();
        let mut social = crate::social::SocialModel::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        social.apply_packet("GroupInvite", &serde_json::json!({"name":"Peer"}));
        state.group_dialog.invitation = Some(("Peer".into(), social.group.pending_invite_epoch));
        assert!(!state.group_open());
        answer_invitation(&mut state, &mut social, &mut queue, true);
        assert!(state.group_open());
        assert!(!social.group.active);
        assert_eq!(
            queue.drain_intents(),
            vec![NativePlayerUiIntent::GroupInvite {
                accept_invite: true
            }]
        );
        let old = state.group_dialog.answered.clone().unwrap();
        social.group.pending_invite_from = None;
        social.apply_packet("GroupInvite", &serde_json::json!({"name":"Peer"}));
        state.group_dialog.restore_invitation(&old, &social.group);
        assert!(
            state.group_dialog.invitation.is_none(),
            "failed old reply must not revive old invitation"
        );
        state.group_dialog.invitation = Some(old);
        answer_invitation(&mut state, &mut social, &mut queue, false);
        assert!(queue.drain_intents().is_empty());
    }
    #[test]
    fn same_inviter_can_invite_again_after_decline_but_snapshot_is_idempotent() {
        let mut social = crate::social::SocialModel::default();
        let mut state = NativePlayerUiState::default();
        let mut queue = NativePlayerUiIntentQueue::default();
        let invite = serde_json::json!({"name":"Peer"});
        social.apply_network_packet("GroupInvite", &invite);
        let first = social.group.pending_invite_epoch;
        state.group_dialog.invitation = Some(("Peer".into(), first));
        answer_invitation(&mut state, &mut social, &mut queue, false);
        assert_eq!(
            queue.drain_intents(),
            vec![NativePlayerUiIntent::GroupInvite {
                accept_invite: false
            }]
        );
        social.apply_packet("GroupInvite", &invite);
        assert_eq!(social.group.pending_invite_epoch, first);
        social.apply_network_packet("GroupInvite", &invite);
        assert!(social.group.pending_invite_epoch > first);
        state.group_dialog.invitation = Some(("Peer".into(), social.group.pending_invite_epoch));
        let current = state.group_dialog.invitation.clone();
        state
            .group_dialog
            .restore_invitation(&("Peer".into(), first), &social.group);
        assert_eq!(state.group_dialog.invitation, current);
        answer_invitation(&mut state, &mut social, &mut queue, true);
        assert!(state.group_open());
        assert_eq!(
            queue.drain_intents(),
            vec![NativePlayerUiIntent::GroupInvite {
                accept_invite: true
            }]
        );
    }
    #[test]
    fn source_member_map_arrives_before_member_and_is_removed_on_leave() {
        let mut social = crate::social::SocialModel::default();
        assert!(social.apply_packet(
            "GroupMembersMap",
            &serde_json::json!({"playerName":"Peer","playerMap":"Bichon Province"})
        ));
        social.apply_packet("AddMember", &serde_json::json!({"name":"Peer"}));
        assert_eq!(social.group.member_maps["Peer"], "Bichon Province");
        social.apply_packet("DeleteMember", &serde_json::json!({"name":"Peer"}));
        assert!(social.group.member_maps.is_empty());
    }
    #[test]
    fn original_name_prompt_sends_only_add_without_permission_toggle() {
        let mut d = GroupDialogUi::default();
        let mut social = crate::social::SocialModel::default();
        let mut q = NativePlayerUiIntentQueue::default();
        d.show_input(false, &social.group, "Owner");
        d.editor.paste("Peer");
        d.submit(&mut q, &mut social);
        let wire = q.drain_intents();
        assert_eq!(
            wire,
            vec![NativePlayerUiIntent::GroupAddMember {
                name: "Peer".into()
            }]
        );
        assert!(!social.group.allow_invites);
        assert!(!d.modal());
        d.restore_unsent(false, "Peer");
        assert!(d.modal());
        assert_eq!(d.editor.editor.as_ref().unwrap().text(), "Peer");
    }
    #[test]
    fn source_membership_order_gates_leader_and_full_group() {
        let mut g = crate::social::GroupModel::default();
        g.members.push(crate::social::GroupMemberModel {
            name: "Leader".into(),
            ..Default::default()
        });
        let mut d = GroupDialogUi::default();
        d.show_input(false, &g, "Other");
        assert!(!d.modal());
        assert_eq!(
            d.notice.as_deref(),
            Some("You are not the leader of your group.")
        );
        d.show_input(true, &g, "Leader");
        assert!(d.modal());
        d.editor.cancel_modal();
        g.members.resize(15, Default::default());
        d.show_input(false, &g, "Leader");
        assert!(!d.modal());
        assert_eq!(
            d.notice.as_deref(),
            Some("Your group already has the maximum number of members.")
        );
    }
    #[test]
    fn stale_send_failure_does_not_replace_new_input() {
        let mut d = GroupDialogUi::default();
        let mut social = crate::social::SocialModel::default();
        let mut q = NativePlayerUiIntentQueue::default();
        d.show_input(false, &social.group, "Owner");
        d.editor.paste("Old");
        d.submit(&mut q, &mut social);
        d.show_input(true, &social.group, "Owner");
        d.editor.paste("New");
        d.restore_unsent(false, "Old");
        assert_eq!(d.editor.editor.as_ref().unwrap().text(), "New");
        assert!(d.remove);
    }
}

pub(super) fn answer_invitation(
    state: &mut NativePlayerUiState,
    social: &mut crate::social::SocialModel,
    intents: &mut NativePlayerUiIntentQueue,
    accept: bool,
) {
    let Some(identity) = state.group_dialog.invitation.clone() else {
        return;
    };
    if social.group.pending_invite_from.as_ref() != Some(&identity.0)
        || social.group.pending_invite_epoch != identity.1
    {
        return;
    }
    if intents.push_social_pending(
        social,
        NativePlayerUiIntent::GroupInvite {
            accept_invite: accept,
        },
    ) {
        state.group_dialog.invitation = None;
        state.group_dialog.answered = Some(identity);
        state.group_dialog.consumed = true;
        if accept && !state.group_open() {
            state.toggle_group();
        }
    }
}
#[derive(Component)]
pub(super) struct GroupInvitation;
pub(super) fn render_invitation(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<GroupInvitation>>,
    state: Res<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let (Some((name, _)), Some(assets), Ok(root)) =
        (&state.group_dialog.invitation, assets, roots.single())
    else {
        return;
    };
    commands.entity(root).with_children(|p| {
        p.spawn((
            GroupInvitation,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(284.),
                top: Val::Px(289.),
                width: Val::Px(456.),
                height: Val::Px(190.),
                ..default()
            },
            GlobalZIndex(1099),
            FocusPolicy::Block,
        ))
        .with_children(|p| {
            spawn_overlay_frame(p, &assets, "original-ui/Prguse/360.png", 456., 190.);
            friend_dialog::view::wrapped_text(
                p,
                &format!("Do you want to group with {name}?"),
                CrystalRect::new(35., 35., 390., 110.),
                Color::WHITE,
            );
            for (frame, x, action) in [
                (206, 260., OverlayButton::GroupInviteAccept),
                (210, 360., OverlayButton::GroupInviteDecline),
            ] {
                spawn_overlay_crystal_button_enabled(
                    p,
                    &assets,
                    "Title",
                    frame,
                    frame + 1,
                    frame + 2,
                    CrystalRect::new(x, 157., 76., 25.),
                    action,
                    true,
                );
            }
        });
    });
}
