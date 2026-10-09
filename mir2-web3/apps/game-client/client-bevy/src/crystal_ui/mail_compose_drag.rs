//! MailComposeLetterDialog.Movable: use the source frame and child hit bounds.
use super::*;

#[derive(Resource, Debug)]
pub(super) struct MailLetterWindow {
    pub position: Vec2,
    drag: Option<(String, Vec2)>,
    reset_revision: Option<u64>,
}

impl Default for MailLetterWindow {
    fn default() -> Self {
        Self { position: Vec2::splat(100.0), drag: None, reset_revision: None }
    }
}

impl MailLetterWindow {
    pub(super) fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

pub(super) fn covered(state: &NativePlayerUiState) -> bool {
    state.leave_game.blocks() || state.group_dialog.modal() || state.group_dialog.consumed
        || state.social_bonds.prompt.is_some()
        || state.keyboard.open || state.keyboard.input_consumed
        || state.friends.modal.is_some() || state.friends.input_consumed
        || state.trade_dialog.open || state.trade_dialog.message.is_some() || state.trade_dialog.input_consumed
        || state.game_shop_dialog.confirmation.is_some()
        || state.inventory_delete_prompt.is_some() || state.guild_gold_prompt.is_some()
        || state.guild_panel.blocks() || state.guild_panel.consumed
        || state.skill_assign.open || state.hero.modal() || state.hero.input_consumed
        || equipment_creature_host::modal(state) || state.help.open
}

fn drag_surface(point: Vec2) -> bool {
    let contains = |rect: CrystalRect| rect.contains(point.x, point.y);
    contains(CrystalRect::new(0.0, 0.0, 236.0, 300.0))
        && !contains(CrystalRect::new(209.0, 3.0, 24.0, 21.0))
        && !contains(CrystalRect::new(15.0, 92.0, 202.0, 165.0))
        && !contains(CrystalRect::new(30.0, 265.0, 76.0, 25.0))
        && !contains(CrystalRect::new(135.0, 265.0, 68.0, 25.0))
}

pub(super) fn process(
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    compose: Res<MailComposeUi>,
    reset: Option<Res<crate::pending_operations::SessionResetRevision>>,
    mut frame: ResMut<MailLetterWindow>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
) {
    let revision = reset.as_deref().map_or(0, |value| value.0);
    if frame.reset_revision.is_some_and(|previous| previous != revision) {
        *frame = default();
    }
    frame.reset_revision = Some(revision);
    if shell.screen != NativeShellScreen::InGame {
        *frame = default();
        moves.clear();
        return;
    }
    let Ok((entity, window)) = windows.single() else {
        frame.drag = None;
        moves.clear();
        return;
    };
    let path: Vec<_> = moves.read().filter(|event| event.window == entity)
        .map(|event| cursor_logical(window, event.position)).collect();
    let ready = window.focused && state.mail_open()
        && !covered(&state)
        && compose.kind == MailComposeKind::Letter
        && !state.amount_modal_open()
        && state.mail_feedback_prompt.is_none() && !state.mail_feedback_input_consumed
        && !state.mail_recipient_prompt_active && !state.mail_recipient_input_consumed
        && state.mail_reader.is_none() && !state.mail_reader_input_consumed
        && state.mail_delete_prompt.is_none() && !state.mail_delete_input_consumed
        && state.storage_password_prompt.is_none() && state.storage_rental_confirmation.is_none();
    let (Some(draft), Some(mouse), true) = (state.core.mail_compose.as_ref(), mouse, ready) else {
        frame.drag = None;
        return;
    };
    let Some(cursor) = path.last().copied().or_else(|| help_cursor_logical(window)) else {
        frame.drag = None;
        return;
    };
    let recipient = draft.recipient.clone();
    if mouse.just_pressed(MouseButton::Left) {
        frame.drag = None;
        let offset = path.first().copied().unwrap_or(cursor) - frame.position;
        if drag_surface(offset) {
            frame.drag = Some((recipient.clone(), offset));
        }
    }
    if let Some((owner, offset)) = frame.drag.clone() {
        if owner != recipient {
            frame.drag = None;
            return;
        }
        if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
            frame.position = (cursor - offset).clamp(Vec2::ZERO, Vec2::new(787.0, 467.0));
            state.menu_pointer_consumed = true;
        }
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        frame.drag = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_children_exclude_drag_and_recipient_label_is_not_control() {
        assert!(drag_surface(Vec2::new(100.0, 10.0)));
        assert!(drag_surface(Vec2::new(100.0, 40.0)));
        assert!(!drag_surface(Vec2::new(220.0, 10.0)));
        assert!(!drag_surface(Vec2::new(50.0, 100.0)));
        assert!(!drag_surface(Vec2::new(60.0, 275.0)));
        assert!(!drag_surface(Vec2::new(150.0, 275.0)));
        assert!(drag_surface(Vec2::new(210.0, 275.0)), "actual Cancel is68px wide");
    }

    #[test]
    fn scaled_batch_release_clamping_modality_and_session_reset() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<MailLetterWindow>()
            .init_resource::<crate::pending_operations::SessionResetRevision>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(NativeShellModel { screen: NativeShellScreen::InGame, ..default() })
            .add_message::<CursorMoved>()
            .add_systems(Update, process);
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.core.panel = mir2_ui_core::state::UiPanel::Mail;
            state.core.mail_compose = Some(mir2_ui_core::state::MailComposeDraft {
                recipient: "Receiver".into(), ..default()
            });
        }
        let mut window = Window::default();
        window.resolution.set(2048.0, 1536.0);
        window.set_cursor_position(Some(Vec2::new(640.0, 420.0)));
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        for position in [Vec2::new(240.0, 220.0), Vec2::new(640.0, 420.0)] {
            app.world_mut().write_message(CursorMoved { window: entity, position, delta: None });
        }
        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.press(MouseButton::Left);
            mouse.release(MouseButton::Left);
        }
        app.update();
        assert_eq!(app.world().resource::<MailLetterWindow>().position, Vec2::new(300.0, 200.0));
        assert!(app.world().resource::<MailLetterWindow>().drag.is_none());
        assert!(app.world().resource::<NativePlayerUiState>().menu_pointer_consumed);
        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.press(MouseButton::Left);
            mouse.clear();
        }
        app.world_mut().resource_mut::<MailLetterWindow>().drag = Some(("Receiver".into(), Vec2::ZERO));
        app.world_mut().write_message(CursorMoved { window: entity, position: Vec2::splat(4000.0), delta: None });
        app.update();
        assert_eq!(app.world().resource::<MailLetterWindow>().position, Vec2::new(787.0, 467.0));
        for closing_frame in [false, true] {
            app.world_mut().resource_mut::<MailLetterWindow>().drag = Some(("Receiver".into(), Vec2::ZERO));
            {
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.mail_recipient_prompt_active = !closing_frame;
                state.mail_recipient_input_consumed = closing_frame;
            }
            app.update();
            assert!(app.world().resource::<MailLetterWindow>().drag.is_none());
            assert_eq!(app.world().resource::<MailLetterWindow>().position, Vec2::new(787.0, 467.0));
        }
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.mail_recipient_prompt_active = false;
            state.mail_recipient_input_consumed = false;
            state.keyboard.open = true;
        }
        app.world_mut().resource_mut::<MailLetterWindow>().drag = Some(("Receiver".into(), Vec2::ZERO));
        app.update();
        assert!(app.world().resource::<MailLetterWindow>().drag.is_none());
        app.world_mut().resource_mut::<NativePlayerUiState>().keyboard.open = false;
        app.world_mut().resource_mut::<MailLetterWindow>().drag = Some(("Receiver".into(), Vec2::ZERO));
        app.world_mut().resource_mut::<crate::pending_operations::SessionResetRevision>().0 += 1;
        app.update();
        assert_eq!(app.world().resource::<MailLetterWindow>().position, Vec2::splat(100.0));
        assert!(app.world().resource::<MailLetterWindow>().drag.is_none(), "same-recipient ownership must not survive an in-game session reset");
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::Login;
        app.update();
        assert_eq!(app.world().resource::<MailLetterWindow>().position, Vec2::splat(100.0));
    }
}
