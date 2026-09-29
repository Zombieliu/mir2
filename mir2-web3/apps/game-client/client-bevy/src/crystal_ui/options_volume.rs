//! Source `OptionDialog` volume controls: pointer-position bars with capture.
//!
//! `MainDialogs.cs` assigns both `SoundBar` and `MusicSoundBar` index 468 at
//! their full 76px control bounds, then calculates volume from the local x on
//! left mouse down/move. The renderer keeps that interaction separate from
//! Bevy's image-button adapter so one press cannot also trigger an old ±10
//! action or a covered window.

use super::*;

pub(super) const SOUND_BAR_RECT: CrystalRect = CrystalRect::new(159.0, 225.0, 76.0, 19.0);
pub(super) const MUSIC_BAR_RECT: CrystalRect = CrystalRect::new(159.0, 251.0, 76.0, 19.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VolumeChannel {
    Sound,
    Music,
}

pub(super) const fn rect(channel: VolumeChannel) -> CrystalRect {
    match channel {
        VolumeChannel::Sound => SOUND_BAR_RECT,
        VolumeChannel::Music => MUSIC_BAR_RECT,
    }
}

#[derive(Component)]
pub(super) struct OptionsVolumeTrack(pub(super) VolumeChannel);

#[derive(Component)]
pub(super) struct OptionsVolumeThumb(pub(super) VolumeChannel);

#[derive(Debug, Default, Resource)]
pub(super) struct OptionsVolumeDrag {
    captured: Option<VolumeChannel>,
    reset_revision: Option<u64>,
}

impl OptionsVolumeDrag {
    #[cfg(test)]
    pub(super) fn captured(&self) -> Option<VolumeChannel> {
        self.captured
    }

    fn cancel(&mut self) {
        self.captured = None;
    }
}

/// Crystal's `(byte)(localX / 76D * 100)` calculation, bounded at the two
/// native control edges for a captured drag that leaves the bar.
pub(super) fn volume_from_x(channel: VolumeChannel, local_x: f32) -> u8 {
    let bar = rect(channel);
    (((local_x - bar.left).clamp(0.0, bar.width) / bar.width) * 100.0) as u8
}

fn channel_at(local: Vec2) -> Option<VolumeChannel> {
    [VolumeChannel::Sound, VolumeChannel::Music]
        .into_iter()
        .find(|channel| rect(*channel).contains(local.x, local.y))
}

fn covered(state: &NativePlayerUiState) -> bool {
    state.amount_modal_open()
        || state.mail_feedback_prompt.is_some()
        || state.mail_feedback_input_consumed
        || state.mail_recipient_prompt_active
        || state.mail_recipient_input_consumed
        || state.mail_reader.is_some()
        || state.mail_reader_input_consumed
        || state.mail_delete_prompt.is_some()
        || state.mail_delete_input_consumed
        || state.storage_password_prompt.is_some()
        || state.storage_password_input_consumed
        || state.storage_rental_confirmation.is_some()
        || state.storage_rental_input_consumed
        || mail_compose_drag::covered(state)
}

fn apply_volume(
    state: &mut NativePlayerUiState,
    effects: &mut UiEffectQueue,
    channel: VolumeChannel,
    volume: u8,
) {
    let current = match channel {
        VolumeChannel::Sound => state.core.options.sound_volume,
        VolumeChannel::Music => state.core.options.music_volume,
    };
    if current == volume {
        return;
    }
    let action = match channel {
        VolumeChannel::Sound => mir2_ui_core::action::UiAction::SetSoundVolume { volume },
        VolumeChannel::Music => mir2_ui_core::action::UiAction::SetMusicVolume { volume },
    };
    dispatch_ui_action(&mut state.core, effects, action);
}

/// Captures a source bar on the initial left press and continues updating it
/// with logical-stage cursor movement until release. The raw cursor comes from
/// the observed primary window and goes through `cursor_logical`, preserving
/// fixed-stage scaling instead of assuming physical desktop pixels.
pub(super) fn process(
    shell: Res<NativeShellModel>,
    mut state: ResMut<NativePlayerUiState>,
    reset: Option<Res<SessionResetRevision>>,
    mut drag: ResMut<OptionsVolumeDrag>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut moves: MessageReader<CursorMoved>,
    mut effects: Option<ResMut<UiEffectQueue>>,
) {
    let revision = reset.as_deref().map_or(0, |value| value.0);
    if drag.reset_revision.is_some_and(|previous| previous != revision) {
        drag.cancel();
    }
    drag.reset_revision = Some(revision);

    if shell.screen != NativeShellScreen::InGame || !state.options_open() || covered(&state) {
        drag.cancel();
        moves.clear();
        return;
    }
    let (Some(mouse), Ok((window_entity, window))) = (mouse, windows.single()) else {
        drag.cancel();
        moves.clear();
        return;
    };
    if !window.focused {
        drag.cancel();
        moves.clear();
        return;
    }

    let path = moves
        .read()
        .filter(|event| event.window == window_entity)
        .map(|event| cursor_logical(window, event.position))
        .collect::<Vec<_>>();
    let current_cursor = path.last().copied().or_else(|| help_cursor_logical(window));
    // Windows can deliver the pointer-at-press and a held movement in one
    // frame. Like the source MouseDown/MouseMove sequence, capture belongs to
    // the point where the press started, while the later movement changes the
    // captured bar.
    let press_cursor = path.first().copied().or(current_cursor);

    if mouse.just_pressed(MouseButton::Left)
        && drag.captured.is_none()
        && !state.menu_pointer_consumed
    {
        if let Some(cursor) = press_cursor {
            let local = cursor - Vec2::new(CRYSTAL_OPTIONS_PANEL_RECT.left, CRYSTAL_OPTIONS_PANEL_RECT.top);
            drag.captured = channel_at(local);
            if let Some(channel) = drag.captured {
                let mut fallback = UiEffectQueue::default();
                let effects = effects.as_deref_mut().unwrap_or(&mut fallback);
                apply_volume(&mut state, effects, channel, volume_from_x(channel, local.x));
                state.menu_pointer_consumed = true;
            }
        }
    }

    if let Some(channel) = drag.captured {
        // A release only ends capture. Crystal's handlers update while the
        // left button is down; it does not apply a second location on MouseUp.
        if mouse.pressed(MouseButton::Left) {
            if let Some(cursor) = current_cursor {
                let local = cursor
                    - Vec2::new(CRYSTAL_OPTIONS_PANEL_RECT.left, CRYSTAL_OPTIONS_PANEL_RECT.top);
                let mut fallback = UiEffectQueue::default();
                let effects = effects.as_deref_mut().unwrap_or(&mut fallback);
                apply_volume(&mut state, effects, channel, volume_from_x(channel, local.x));
            }
            state.menu_pointer_consumed = true;
        }
        if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
            state.menu_pointer_consumed = true;
            drag.cancel();
        }
    }
}
