use super::super::*;
use super::*;
#[derive(Default, Resource)]
pub struct SocialHost {
    dimensions: std::collections::HashMap<(String, u16), Vec2>,
    dragging: Option<BondPage>,
    selecting: bool,
}
impl SocialHost {
    fn size(&self, l: &str, i: u16) -> Option<Vec2> {
        self.dimensions.get(&(l.into(), i)).copied()
    }
}
pub fn toggle(state: &mut NativePlayerUiState, page: BondPage) {
    let open = match page {
        BondPage::Mentor => state.social_bonds.mentor.open,
        BondPage::Relationship => state.social_bonds.relationship.open,
    };
    if open {
        state.social_bonds.hide(page);
    } else {
        state.social_bonds.show(page);
    }
}
pub fn covers(state: &NativePlayerUiState, p: Vec2) -> bool {
    let b = &state.social_bonds;
    [
        (b.mentor.open, b.mentor.position, 244., 207.),
        (b.relationship.open, b.relationship.position, 284., 194.),
    ]
    .iter()
    .any(|(open, pos, w, h)| {
        *open && pos.is_some_and(|q| CrystalRect::new(q[0], q[1], *w, *h).contains(p.x, p.y))
    })
}
fn enqueue(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    packet: ClientPacket,
) {
    if !intents.push_intent(NativePlayerUiIntent::SocialBondPacket(packet.clone())) {
        state.social_bonds.release_unsent(&packet);
    }
}
fn answer(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    revision: u64,
    accept: bool,
) {
    state.social_bonds.sync_draft();
    if let Some(packet) = state.social_bonds.answer(revision, accept) {
        enqueue(state, intents, packet);
    }
}
pub fn keyboard(
    state: &mut NativePlayerUiState,
    intents: &mut NativePlayerUiIntentQueue,
    events: &[KeyboardInput],
) {
    state.social_bonds.input_consumed = true;
    state.social_bonds.sync_editor();
    for event in events {
        let Some(revision) = state.social_bonds.prompt.as_ref().map(|p| p.revision) else {
            break;
        };
        if event.state == ButtonState::Pressed {
            if event.key_code == KeyCode::Escape {
                answer(state, intents, revision, false);
                break;
            }
            if matches!(event.key_code, KeyCode::Enter | KeyCode::NumpadEnter) {
                if !event.repeat {
                    answer(state, intents, revision, true);
                }
                break;
            }
        }
        friend_dialog::host::edit_key(&mut state.social_bonds.input, event);
        state.social_bonds.sync_draft();
    }
}
pub(in super::super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<SocialHost>,
    shell: Option<Res<NativeShellModel>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut intents: ResMut<NativePlayerUiIntentQueue>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    state.social_bonds.input_consumed = state.social_bonds.prompt.is_some();
    state.social_bonds.sync_editor();
    if let Some(text) = state.social_bonds.notice.take() {
        if let Some(chat) = chat.as_deref_mut() {
            chat.push(crate::chat::ChatLine {
                text,
                channel: "system".into(),
            });
        }
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        return;
    }
    let (Ok(window), Some(mouse)) = (windows.single(), mouse) else {
        return;
    };
    if !window.focused {
        host.dragging = None;
        host.selecting = false;
        state.social_bonds.end_drag();
        state.social_bonds.input.modifiers = [false; 4];
        return;
    }
    if state.leave_game.blocks() {
        return;
    }
    let Some(cursor) = help_cursor_logical(window) else {
        return;
    };
    if let Some(prompt) = state.social_bonds.prompt.clone() {
        let frame = if matches!(prompt.kind, BondPromptKind::MentorName { .. }) {
            660
        } else {
            360
        };
        if let Some(size) = host.size("Prguse", frame) {
            let local = cursor - (Vec2::new(1024., 768.) - size) / 2.;
            if mouse.just_pressed(MouseButton::Left) {
                state.menu_pointer_consumed = true;
                state.social_bonds.input_consumed = true;
                if let Some(hit) = view::prompt_hit(&prompt, local, |l, i| host.size(l, i)) {
                    answer(&mut state, &mut intents, hit.revision, hit.accept);
                } else if frame == 660 {
                    state.social_bonds.input.editor_focused =
                        CrystalRect::new(23., 86., 240., 19.).contains(local.x, local.y);
                    if state.social_bonds.input.editor_focused {
                        friend_dialog::host::focus_editor(
                            &mut state.social_bonds.input,
                            local - Vec2::new(23., 86.),
                            false,
                        );
                        host.selecting = true;
                    }
                }
            }
            if mouse.pressed(MouseButton::Left) && host.selecting {
                friend_dialog::host::focus_editor(
                    &mut state.social_bonds.input,
                    local - Vec2::new(23., 86.),
                    true,
                );
            }
        }
    } else if !state.amount_modal_open() && !state.keyboard.open {
        for page in [BondPage::Relationship, BondPage::Mentor] {
            let (open, pos, frame) = match page {
                BondPage::Mentor => (
                    state.social_bonds.mentor.open,
                    state.social_bonds.mentor.position,
                    170,
                ),
                BondPage::Relationship => (
                    state.social_bonds.relationship.open,
                    state.social_bonds.relationship.position,
                    583,
                ),
            };
            let (true, Some(pos), Some(size)) = (open, pos, host.size("Prguse", frame)) else {
                continue;
            };
            let local = cursor - Vec2::from(pos);
            if !CrystalRect::new(0., 0., size.x, size.y).contains(local.x, local.y) {
                continue;
            }
            if mouse.just_pressed(MouseButton::Left) {
                state.menu_pointer_consumed = true;
                if let Some(action) =
                    view::hit_action(&state.social_bonds, page, local, |l, i| host.size(l, i))
                {
                    if let Some(effect) = state.social_bonds.action(page, action) {
                        match effect {
                            BondEffect::Packet(packet) => enqueue(&mut state, &mut intents, packet),
                            BondEffect::SystemChat(text) => state.social_bonds.notice = Some(text),
                            BondEffect::ComposeMail(recipient) => {
                                state.apply(mir2_ui_core::action::UiAction::OpenMailCompose);
                                state.apply(mir2_ui_core::action::UiAction::SetMailRecipient {
                                    recipient,
                                });
                            }
                            BondEffect::Whisper(text) => {
                                state.chat_draft = text;
                                state.set_chat_focused(true);
                            }
                        }
                    }
                } else if local.y < 30. {
                    state.social_bonds.begin_drag(page, cursor.to_array());
                    host.dragging = Some(page);
                }
            }
            break;
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(page) = host.dragging {
            state.social_bonds.drag_to(page, cursor.to_array());
        }
    } else {
        host.dragging = None;
        host.selecting = false;
        state.social_bonds.end_drag();
    }
}
fn short_date(binary: i64) -> String {
    let ticks = (binary as u64 & 0x3fff_ffff_ffff_ffff) as i128;
    let seconds = (ticks - 621355968000000000i128).div_euclid(10000000);
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds as i64, 0)
        .map(|d| {
            if binary as u64 & 0x8000_0000_0000_0000 != 0 {
                d.with_timezone(&chrono::Local)
                    .format("%Y/%m/%d")
                    .to_string()
            } else {
                d.format("%Y/%m/%d").to_string()
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: bevy::input::keyboard::Key::Unidentified(
                bevy::input::keyboard::NativeKey::Unidentified,
            ),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }
    #[test]
    fn invitation_blocks_gameplay_and_decline_uses_authoritative_reply() {
        let mut s = NativePlayerUiState::default();
        let mut q = NativePlayerUiIntentQueue::default();
        s.social_bonds.observe(&ServerPacket::MentorRequest {
            name: "Student".into(),
            level: 10,
        });
        assert!(s.blocks_gameplay_keys());
        assert!(s.blocks_world_click());
        keyboard(&mut s, &mut q, &[key(KeyCode::Escape)]);
        assert!(matches!(
            q.drain_intents().into_iter().next(),
            Some(NativePlayerUiIntent::SocialBondPacket(
                ClientPacket::MentorReply {
                    accept_invite: false
                }
            ))
        ));
        assert!(s.social_bonds.prompt.is_none());
        assert!(s.amount_modal_open());
    }
    #[test]
    fn full_queue_restores_name_modal_and_session_reset_clears_relationship_owner() {
        let mut s = NativePlayerUiState::default();
        let mut q = NativePlayerUiIntentQueue::default();
        toggle(&mut s, BondPage::Mentor);
        s.social_bonds
            .action(BondPage::Mentor, BondAction::AddMentor);
        let rev = s.social_bonds.prompt.as_ref().unwrap().revision;
        s.social_bonds.input_name(rev, "Mentor");
        while q.push_intent(NativePlayerUiIntent::RefreshFriends) {}
        answer(&mut s, &mut q, rev, true);
        assert!(
            matches!(&s.social_bonds.prompt.as_ref().unwrap().kind,BondPromptKind::MentorName{text} if text=="Mentor")
        );
        s.social_bonds.relationship.name = "Peer".into();
        s.reset_session();
        assert!(s.social_bonds.prompt.is_none());
        assert!(s.social_bonds.relationship.name.is_empty());
    }
}
pub(in super::super) fn render_system(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    panels: Query<Entity, Or<(With<view::BondPanel>, With<view::BondPromptPanel>)>>,
    text_blocks: Query<(
        &friend_dialog::view::FriendEditText,
        &bevy::text::ComputedTextBlock,
        &bevy::text::TextLayoutInfo,
    )>,
    mut state: ResMut<NativePlayerUiState>,
    mut host: ResMut<SocialHost>,
    ui: Option<Res<UiReadModel>>,
    shell: Option<Res<NativeShellModel>>,
    assets: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    mut handles: Local<std::collections::HashMap<(String, u16), Handle<Image>>>,
) {
    for p in &panels {
        commands.entity(p).despawn();
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        return;
    }
    let (Some(assets), Some(images), Some(ui)) = (assets, images, ui) else {
        return;
    };
    state.social_bonds.sync_editor();
    for (tag, block, info) in &text_blocks {
        friend_dialog::host::capture_layout(&mut state.social_bonds.input, tag, block, info);
    }
    for (lib, ids) in view::REQUIRED_ASSETS {
        for i in *ids {
            let handle = handles
                .entry(((*lib).into(), *i))
                .or_insert_with(|| assets.load(format!("original-ui/{lib}/{i}.png")));
            if let Some(image) = images.get(handle) {
                host.dimensions.insert(
                    ((*lib).into(), *i),
                    Vec2::new(image.width() as f32, image.height() as f32),
                );
            }
        }
    }
    let Ok(root) = roots.single() else {
        return;
    };
    commands.entity(root).with_children(|p| {
        for page in [BondPage::Mentor, BondPage::Relationship] {
            view::render(
                p,
                &assets,
                &mut state.social_bonds,
                page,
                Vec2::new(1024., 768.),
                ui.player.name.as_deref().unwrap_or(""),
                ui.player.level.min(u16::MAX as u32) as u16,
                short_date,
                |l, i| host.size(l, i),
            );
        }
        if let Some(prompt) = &state.social_bonds.prompt {
            view::render_prompt(
                p,
                &assets,
                prompt,
                Vec2::new(1024., 768.),
                ui.player.class_name.as_deref().unwrap_or(""),
                &state.social_bonds.input,
                |l, i| host.size(l, i),
            );
        }
    });
}
