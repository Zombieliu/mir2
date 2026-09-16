//! Crystal GameScene.QuitGame/LogOut and Globals.LogDelay.
use std::time::{Duration, Instant};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveKind {
    Exit,
    Logout,
}
impl LeaveKind {
    pub fn message(self) -> &'static str {
        match self {
            Self::Exit => "Do you want to quit Legend of Mir?",
            Self::Logout => "Do you want to log out of Legend of Mir?",
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeaveGameDialog {
    pub prompt: Option<LeaveKind>,
    pub notice: Option<String>,
    pub consumed: bool,
    log_time: Option<Instant>,
}
impl LeaveGameDialog {
    /// Called on self Struck and the ordinary attack/spell initiation paths.
    pub fn combat(&mut self, now: Instant) {
        self.log_time = Some(now + Duration::from_secs(10));
    }
    pub fn request(&mut self, kind: LeaveKind, now: Instant) {
        if let Some(until) = self.log_time.filter(|until| *until > now) {
            // C# integer division intentionally reports zero for the final partial second.
            self.notice = Some(format!(
                "Cannot leave game for {} seconds",
                until.duration_since(now).as_secs()
            ));
        } else {
            self.prompt = Some(kind);
        }
    }
    pub fn answer(&mut self, yes: bool) -> Option<LeaveKind> {
        self.consumed = self.prompt.is_some();
        let kind = self.prompt.take()?;
        yes.then_some(kind)
    }
    pub fn blocks(&self) -> bool {
        self.prompt.is_some() || self.consumed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_delay_counts_down_then_opens_confirmation() {
        let now = Instant::now();
        let mut d = LeaveGameDialog::default();
        d.combat(now);
        d.request(LeaveKind::Logout, now + Duration::from_millis(1));
        assert_eq!(d.notice.as_deref(), Some("Cannot leave game for 9 seconds"));
        assert!(d.prompt.is_none());
        d.request(LeaveKind::Exit, now + Duration::from_millis(9999));
        assert_eq!(d.notice.as_deref(), Some("Cannot leave game for 0 seconds"));
        d.request(LeaveKind::Exit, now + Duration::from_secs(10));
        assert_eq!(d.prompt, Some(LeaveKind::Exit));
    }
    #[test]
    fn decline_closes_and_consumes_frame_without_emitting_leave() {
        let mut d = LeaveGameDialog::default();
        d.request(LeaveKind::Logout, Instant::now());
        assert_eq!(d.answer(false), None);
        assert!(d.blocks());
        assert!(d.prompt.is_none());
    }
    #[test]
    fn confirmation_uses_original_single_request_gate() {
        let now = Instant::now();
        let mut d = LeaveGameDialog::default();
        d.request(LeaveKind::Logout, now);
        d.combat(now);
        // Source checks LogTime before opening, not again inside YesButton.Click.
        assert_eq!(d.answer(true), Some(LeaveKind::Logout));
    }
}

use super::*;
#[derive(Component)]
pub(super) struct LeavePanel;
pub(super) fn finish(
    state: &mut NativePlayerUiState,
    shell: &mut NativeShellModel,
    queue: &mut NativeUiIntentQueue,
    effects: Option<&mut UiEffectQueue>,
    yes: bool,
) {
    match state.leave_game.answer(yes) {
        Some(LeaveKind::Exit) => {
            if let Some(effects) = effects {
                effects.push(mir2_ui_core::effect::UiEffect::ExitApplication);
            }
        }
        Some(LeaveKind::Logout) => {
            let _ = shell.apply_ui_intent(NativeUiIntent::Logout);
            queue.push(NativeUiIntent::Logout);
            state.close_all_windows();
        }
        None => {}
    }
}
pub(super) fn process(
    mut state: ResMut<NativePlayerUiState>,
    mut chat: Option<ResMut<crate::chat::ChatModel>>,
) {
    state.leave_game.consumed = state.leave_game.prompt.is_some();
    if let (Some(text), Some(chat)) = (state.leave_game.notice.take(), chat.as_deref_mut()) {
        chat.push(crate::chat::ChatLine {
            text,
            channel: "system".into(),
        });
    }
}
pub(super) fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<LeavePanel>>,
    state: Res<NativePlayerUiState>,
    assets: Option<Res<AssetServer>>,
    shell: Option<Res<NativeShellModel>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    if shell.is_none_or(|s| s.screen != NativeShellScreen::InGame) {
        return;
    }
    let (Some(kind), Some(assets)) = (state.leave_game.prompt, assets) else {
        return;
    };
    let Ok(root) = roots.single() else {
        return;
    };
    commands.entity(root).with_children(|root| {
        root.spawn((
            LeavePanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(284.),
                top: Val::Px(289.),
                width: Val::Px(456.),
                height: Val::Px(190.),
                ..default()
            },
            FocusPolicy::Block,
            GlobalZIndex(1101),
        ))
        .with_children(|p| {
            spawn_overlay_frame(p, &assets, "original-ui/Prguse/360.png", 456., 190.);
            friend_dialog::view::wrapped_text(
                p,
                kind.message(),
                CrystalRect::new(35., 35., 390., 110.),
                Color::WHITE,
            );
            for (index, x, action) in [
                (206, 260., OverlayButton::LeaveConfirm),
                (210, 360., OverlayButton::LeaveCancel),
            ] {
                let spec = CrystalButtonSpec::new(
                    "Title",
                    index,
                    index + 1,
                    index + 2,
                    CrystalRect::new(x, 157., 76., 25.),
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
        });
    });
}
