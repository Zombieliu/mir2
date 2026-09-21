//! MailDialogs.cs Movable readers. Child controls own their mouse presses.
use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReaderWindows {
    positions: [Vec2; 2],
    dragging: Option<(MailReaderUi, Vec2)>,
}

impl Default for ReaderWindows {
    fn default() -> Self {
        Self { positions: [Vec2::splat(100.0); 2], dragging: None }
    }
}

impl ReaderWindows {
    pub(super) fn position(&self, kind: MailReaderKind) -> Vec2 {
        self.positions[usize::from(kind == MailReaderKind::Parcel)]
    }

    pub(super) fn cancel(&mut self) {
        self.dragging = None;
    }

    fn begin(&mut self, reader: MailReaderUi, cursor: Vec2) {
        self.cancel();
        let local = cursor - self.position(reader.kind);
        if drag_surface(reader.kind, local) {
            self.dragging = Some((reader, local));
        }
    }

    fn move_to(&mut self, reader: MailReaderUi, cursor: Vec2) {
        let Some((owner, offset)) = self.dragging else { return };
        if owner != reader {
            self.cancel();
            return;
        }
        let height = if reader.kind == MailReaderKind::Parcel { 384.0 } else { 300.0 };
        self.positions[usize::from(reader.kind == MailReaderKind::Parcel)] =
            (cursor - offset).clamp(Vec2::ZERO, Vec2::new(1024.0 - 236.0 - 1.0, 768.0 - height - 1.0));
    }
}

fn inside(p: Vec2, x: f32, y: f32, width: f32, height: f32) -> bool {
    p.x >= x && p.y >= y && p.x < x + width && p.y < y + height
}

fn drag_surface(kind: MailReaderKind, p: Vec2) -> bool {
    let parcel = kind == MailReaderKind::Parcel;
    if !inside(p, 0.0, 0.0, 236.0, if parcel { 384.0 } else { 300.0 })
        || inside(p, 209.0, 3.0, 24.0, 21.0)
        || inside(p, 15.0, if parcel { 98.0 } else { 92.0 }, 202.0, 165.0)
    { return false; }
    if parcel {
        !inside(p, 63.0, 290.0, 143.0, 15.0)
            && !(0..5).any(|i| inside(p, 27.0 + i as f32 * 36.0, 311.0, 35.0, 31.0))
            && !inside(p, 30.0, 350.0, 72.0, 25.0)
            && !inside(p, 135.0, 350.0, 68.0, 25.0)
    } else {
        !inside(p, 12.0, 265.0, 68.0, 25.0)
            && !inside(p, 81.0, 265.0, 72.0, 25.0)
            && !inside(p, 154.0, 265.0, 68.0, 25.0)
    }
}

pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
) {
    let Ok((entity, window)) = windows.single() else {
        state.mail_reader_windows.cancel();
        moves.clear();
        return;
    };
    let path: Vec<_> = moves.read().filter(|event| event.window == entity)
        .map(|event| cursor_logical(window, event.position)).collect();
    let ready = window.focused && !state.amount_modal_open()
        && state.mail_feedback_prompt.is_none() && !state.mail_feedback_input_consumed
        && !state.mail_recipient_prompt_active && !state.mail_recipient_input_consumed
        && state.mail_delete_prompt.is_none() && state.storage_password_prompt.is_none()
        && state.storage_rental_confirmation.is_none();
    let (Some(reader), Some(mouse), true) = (state.mail_reader, mouse, ready) else {
        state.mail_reader_windows.cancel();
        return;
    };
    let Some(cursor) = path.last().copied().or_else(|| help_cursor_logical(window)) else {
        state.mail_reader_windows.cancel();
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        state.mail_reader_windows.begin(reader, path.first().copied().unwrap_or(cursor));
    }
    // Apply the last movement even when press/move/release arrived in one frame.
    if mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left) {
        state.mail_reader_windows.move_to(reader, cursor);
    }
    if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
        state.mail_reader_windows.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_controls_never_start_a_drag_but_source_notcontrol_labels_do() {
        for kind in [MailReaderKind::Letter, MailReaderKind::Parcel] {
            assert!(drag_surface(kind, Vec2::new(100.0, 10.0)));
            assert!(drag_surface(kind, Vec2::new(100.0, 40.0)));
            assert!(!drag_surface(kind, Vec2::new(215.0, 10.0)));
            assert!(!drag_surface(kind, Vec2::new(100.0, 120.0)));
        }
        for point in [Vec2::new(40.0, 320.0), Vec2::new(80.0, 295.0), Vec2::new(55.0, 360.0), Vec2::new(150.0, 360.0)] {
            assert!(!drag_surface(MailReaderKind::Parcel, point));
        }
        assert!(drag_surface(MailReaderKind::Parcel, Vec2::new(110.0, 360.0)), "space beyond the original 72-pixel collect button remains draggable");
    }

    #[test]
    fn readers_keep_separate_positions_clamp_and_cancel_stale_identity() {
        let mut windows = ReaderWindows::default();
        let letter = MailReaderUi { mail_id: 1, kind: MailReaderKind::Letter };
        windows.begin(letter, Vec2::new(120.0, 110.0));
        windows.move_to(letter, Vec2::new(320.0, 210.0));
        assert_eq!(windows.position(letter.kind), Vec2::new(300.0, 200.0));
        assert_eq!(windows.position(MailReaderKind::Parcel), Vec2::splat(100.0));
        windows.move_to(letter, Vec2::splat(2000.0));
        assert_eq!(windows.position(letter.kind), Vec2::new(787.0, 467.0));
        windows.move_to(MailReaderUi { mail_id: 2, ..letter }, Vec2::ZERO);
        assert!(windows.dragging.is_none());
        assert_eq!(windows.position(letter.kind), Vec2::new(787.0, 467.0));
    }

    #[test]
    fn scaled_pointer_batch_moves_before_release_and_focus_loss_cancels() {
        let mut app = App::new();
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_systems(Update, process);
        let mut window = Window::default();
        window.resolution.set(2048.0, 1536.0);
        window.set_cursor_position(Some(Vec2::new(640.0, 420.0)));
        let entity = app.world_mut().spawn((window, PrimaryWindow)).id();
        let reader = MailReaderUi { mail_id: 1, kind: MailReaderKind::Letter };
        app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader = Some(reader);
        for position in [Vec2::new(240.0, 220.0), Vec2::new(640.0, 420.0)] {
            app.world_mut().write_message(CursorMoved { window: entity, position, delta: None });
        }
        {
            let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            mouse.press(MouseButton::Left);
            mouse.release(MouseButton::Left);
        }
        app.update();
        let state = app.world().resource::<NativePlayerUiState>();
        assert_eq!(state.mail_reader_windows.position(reader.kind), Vec2::new(300.0, 200.0));
        assert!(state.mail_reader_windows.dragging.is_none());
        assert!(state.blocks_world_click());
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.mail_reader_windows.begin(reader, Vec2::new(320.0, 210.0));
            state.mail_feedback_prompt = Some("Mail claim failed".into());
        }
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().mail_reader_windows.dragging.is_none());
        app.world_mut().resource_mut::<NativePlayerUiState>().mail_feedback_prompt = None;
        // Keep the mouse held so only modal input ownership can cancel the
        // drag, rather than accidentally passing because the button released.
        for closing_frame in [false, true] {
            {
                let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
                mouse.press(MouseButton::Left);
                mouse.clear();
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.mail_recipient_prompt_active = !closing_frame;
                state.mail_recipient_input_consumed = closing_frame;
                state.mail_reader_windows.begin(reader, Vec2::new(320.0, 210.0));
            }
            app.update();
            assert!(app.world().resource::<NativePlayerUiState>().mail_reader_windows.dragging.is_none());
        }
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.mail_recipient_prompt_active = false;
            state.mail_recipient_input_consumed = false;
        }
        app.world_mut().resource_mut::<NativePlayerUiState>().mail_reader_windows
            .begin(reader, Vec2::new(320.0, 210.0));
        app.world_mut().entity_mut(entity).get_mut::<Window>().unwrap().focused = false;
        app.update();
        assert!(app.world().resource::<NativePlayerUiState>().mail_reader_windows.dragging.is_none());
        app.world_mut().resource_mut::<NativePlayerUiState>().reset_session();
        assert_eq!(app.world().resource::<NativePlayerUiState>().mail_reader_windows.position(reader.kind), Vec2::splat(100.0));
    }
}
