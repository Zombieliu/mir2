//! GameScene.HeroInformation creates the real second BuffDialog at y=80.
//! PoisonBuffDialog is source-unreachable and is deliberately not synthesized.
use super::status_hud::{BuffEvent, StatusHud};
use super::*;
use std::time::Instant;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub session_epoch: u64,
    pub hero_generation: u64,
    pub object_id: u32,
}
#[derive(Debug, Clone)]
pub enum Event {
    Information(Identity),
    SpawnState(u8),
    Buff {
        identity: Identity,
        event: BuffEvent,
    },
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeroBuffHud {
    pub identity: Option<Identity>,
    pub visible: bool,
    pub rows: StatusHud,
}
impl HeroBuffHud {
    pub fn observe(&mut self, event: &Event, now: Instant) {
        match event {
            Event::Information(identity) => {
                if self.identity != Some(*identity) {
                    self.rows = Default::default();
                    self.identity = Some(*identity);
                }
                self.visible = true;
            }
            Event::SpawnState(state) if *state < 2 => {
                self.rows = Default::default();
                self.visible = false;
                self.identity = None;
            }
            Event::Buff { identity, event } if self.visible && self.identity == Some(*identity) => {
                self.rows.observe(Some(identity.object_id), event, now)
            }
            _ => {}
        }
    }
}
#[derive(Component)]
pub struct HeroBuffRoot;
#[derive(Component)]
pub struct HeroBuffHint;
fn rect(ui: &NativePlayerUiState) -> CrystalRect {
    let (_, size) = status_hud::buff_geometry(
        ui.hero_buffs.rows.buffs.len(),
        ui.core.options.expanded_hero_buff_window,
    );
    CrystalRect::new(897. - size.x, 80., size.x, size.y)
}
fn button(ui: &NativePlayerUiState, p: Vec2) -> bool {
    let r = rect(ui);
    ui.hero_buffs.visible
        && !ui.hero_buffs.rows.buffs.is_empty()
        && CrystalRect::new(r.left + r.width - 15., 80., 15., 15.).contains(p.x, p.y)
}
pub fn process(
    mut ui: ResMut<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut effects: Option<ResMut<UiEffectQueue>>,
    mut audio: Option<ResMut<crate::ui_audio::NativeUiAudioQueue>>,
) {
    ui.hero_buffs.rows.hovered = false;
    if shell.screen != NativeShellScreen::InGame
        || !ui.hero_buffs.visible
        || !windows.single().is_ok_and(|w| w.focused)
    {
        ui.hero_buffs.rows.pressed = None;
        return;
    }
    let Some(p) = windows.single().ok().and_then(help_cursor_logical) else {
        return;
    };
    ui.hero_buffs.rows.cursor = Some(p);
    let r = rect(&ui);
    let over = !ui.hero_buffs.rows.buffs.is_empty() && r.contains(p.x, p.y);
    ui.hero_buffs.rows.hovered = over;
    let now = Instant::now();
    if ui.hero_buffs.rows.fade_at.is_none_or(|t| now >= t) {
        ui.hero_buffs.rows.opacity =
            (ui.hero_buffs.rows.opacity + if over { 0.2 } else { -0.2 }).clamp(0., 1.);
        ui.hero_buffs.rows.fade_at = Some(now + std::time::Duration::from_millis(55));
    }
    if ui.blocks_gameplay_keys() {
        ui.hero_buffs.rows.pressed = None;
        return;
    }
    let Some(mouse) = mouse else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        ui.hero_buffs.rows.pressed = button(&ui, p).then_some(1);
    }
    if mouse.just_released(MouseButton::Left)
        && ui.hero_buffs.rows.pressed.take().is_some()
        && button(&ui, p)
    {
        ui.core.options.expanded_hero_buff_window =
            ui.hero_buffs.rows.buffs.len() == 1 || !ui.core.options.expanded_hero_buff_window;
        if let Some(e) = effects.as_deref_mut() {
            e.push(mir2_ui_core::effect::UiEffect::PersistOptions {
                options: ui.core.options.clone(),
            });
        }
        if let Some(a) = audio.as_deref_mut() {
            a.push(crate::ui_audio::NativeUiSound::ButtonA);
        }
    }
}
pub fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<HeroBuffRoot>>,
    ui: Res<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
    assets: Option<Res<AssetServer>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let (Ok(root), Some(assets)) = (roots.single(), assets) else {
        return;
    };
    // Crystal Camera toggles player BuffsDialog only, not HeroBuffsDialog.
    if shell.screen != NativeShellScreen::InGame
        || !ui.hero_buffs.visible
        || ui.hero_buffs.rows.buffs.is_empty()
    {
        return;
    }
    let rows = &ui.hero_buffs.rows;
    let expanded = ui.core.options.expanded_hero_buff_window;
    let (frame, size) = status_hud::buff_geometry(rows.buffs.len(), expanded);
    let r = rect(&ui);
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                HeroBuffRoot,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(1024.),
                    height: Val::Px(768.),
                    ..default()
                },
                bevy::ui::FocusPolicy::Pass,
                GlobalZIndex(20),
            ))
            .with_children(|p| {
                status_hud::image(
                    p,
                    &assets,
                    "Prguse2",
                    frame,
                    Vec2::new(r.left, 80.),
                    rows.opacity,
                );
                let index = if rows.cursor.is_some_and(|p| button(&ui, p)) {
                    if rows.pressed.is_some() {
                        9
                    } else {
                        8
                    }
                } else {
                    7
                };
                status_hud::image(
                    p,
                    &assets,
                    "Prguse2",
                    index,
                    Vec2::new(r.left + size.x - 15., 80.),
                    rows.opacity,
                );
                for (i, buff) in rows.buffs.iter().enumerate() {
                    if !expanded && i > 0 {
                        break;
                    }
                    if !buff.blink_hidden(Instant::now()) {
                        status_hud::image(
                            p,
                            &assets,
                            "BuffIcon",
                            status_hud::buff_icon(&buff.kind),
                            Vec2::new(
                                r.left + size.x - 33. - (i % 10) as f32 * 23.,
                                86. + (i / 10) as f32 * 24.,
                            ),
                            1.,
                        );
                    }
                }
                if !expanded {
                    status_hud::count_label(
                        p,
                        rows.buffs.len(),
                        CrystalRect::new(r.left, 87., 43., 20.),
                    );
                }
            });
    });
}
pub fn hint(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<(Entity, &ComputedNode), With<HeroBuffHint>>,
    mut size: Local<Vec2>,
    ui: Res<NativePlayerUiState>,
    shell: Res<NativeShellModel>,
) {
    for (e, node) in &old {
        *size = node.size() * node.inverse_scale_factor();
        commands.entity(e).despawn();
    }
    if shell.screen != NativeShellScreen::InGame
        || !ui.hero_buffs.visible
        || ui.blocks_gameplay_keys()
    {
        return;
    }
    let (Some(p), Ok(root)) = (ui.hero_buffs.rows.cursor, roots.single()) else {
        return;
    };
    let r = rect(&ui);
    let expanded = ui.core.options.expanded_hero_buff_window;
    let Some((_, buff)) = ui.hero_buffs.rows.buffs.iter().enumerate().find(|(i, b)| {
        if !expanded && *i > 0 {
            return false;
        }
        let size = status_hud::frame_size("BuffIcon", status_hud::buff_icon(&b.kind))
            .unwrap_or(Vec2::ZERO);
        CrystalRect::new(
            r.left + r.width - 33. - (*i % 10) as f32 * 23.,
            86. + (*i / 10) as f32 * 24.,
            size.x,
            size.y,
        )
        .contains(p.x, p.y)
    }) else {
        return;
    };
    let text = if expanded {
        status_hud::buff_hint(buff, Instant::now())
    } else {
        status_hud::combined_hint(&ui.hero_buffs.rows.buffs)
    };
    let p = crate::crystal_ui::widget::crystal_hint_position_for_bounds(
        crate::crystal_ui::widget::CrystalHintStyle::Control,
        p,
        *size,
        Vec2::new(1024., 768.),
    );
    commands.entity(root).with_children(|parent| {
        parent
            .spawn((
                HeroBuffHint,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(p.x),
                    top: Val::Px(p.y),
                    padding: UiRect::all(Val::Px(2.)),
                    border: UiRect::all(Val::Px(1.)),
                    ..default()
                },
                BackgroundColor(crate::crystal_ui::widget::CRYSTAL_HINT_BACKGROUND),
                BorderColor::all(crate::crystal_ui::widget::CRYSTAL_HINT_BORDER),
                GlobalZIndex(20000),
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(text),
                    crate::crystal_ui::typography::crystal_text_font(32. / 3.),
                    TextColor(crate::crystal_ui::widget::CRYSTAL_HINT_TEXT),
                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                ));
            });
    });
}
#[cfg(test)]
#[path = "hero_buff_hud_tests.rs"]
mod tests;
