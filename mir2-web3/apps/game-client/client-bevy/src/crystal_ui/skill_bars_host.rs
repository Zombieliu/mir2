use super::super::*;
use super::*;
use crate::crystal_ui::widget::CrystalHint;
#[derive(Component)]
pub struct SkillBarRoot;
#[derive(Component)]
pub struct SkillBarHit(pub usize);
#[derive(Component)]
pub struct SkillBarBackground(pub usize);

pub fn process(
    mut state: ResMut<NativePlayerUiState>,
    skills: Res<SkillModel>,
    shell: Res<NativeShellModel>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    hits: Query<(&SkillBarHit, &Interaction, Option<&SkillBarBackground>)>,
    mut effects: Option<ResMut<UiEffectQueue>>,
) {
    state.skill_bars.observe(&skills, Instant::now());
    state.skill_bars.visible = [
        SkillBarsUi::has_skill(&skills, 0),
        SkillBarsUi::has_skill(&skills, 1),
    ];
    state.skill_bars.hovered = false;
    let focused = windows.single().is_ok_and(|w| w.focused);
    if shell.screen != NativeShellScreen::InGame || !focused || state.blocks_gameplay_keys() {
        state.skill_bars.dragging = None;
        state.skill_bars.armed_slot = None;
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(help_cursor_logical) else {
        return;
    };
    state.skill_bars.cursor = Some([cursor.x, cursor.y]);
    let Some(mouse) = mouse else {
        return;
    };
    if !state.core.options.skill_bar {
        state.skill_bars.dragging = None;
        state.skill_bars.armed_slot = None;
        return;
    }
    state.skill_bars.hovered = hits.iter().any(|(_, i, _)| *i != Interaction::None);
    if mouse.just_released(MouseButton::Left) {
        if let Some(slot) = state.skill_bars.armed_slot.take() {
            let bar = usize::from((slot - 1) / 8);
            let cell = usize::from((slot - 1) % 8);
            let p = state.core.options.skill_bar_positions[bar];
            let left = p[0] as f32 + cell as f32 * 25. + 15.;
            let top = p[1] as f32 + 3.;
            if state.skill_bars.hovered
                && (left..left + 24.).contains(&cursor.x)
                && (top..top + 22.).contains(&cursor.y)
            {
                state.skill_bars.queue_cast(slot);
            }
        }
    } else if !mouse.pressed(MouseButton::Left) {
        state.skill_bars.armed_slot = None;
    }

    if mouse.just_pressed(MouseButton::Left) {
        if let Some((hit, _, _)) = hits
            .iter()
            .find(|(_, i, b)| **i == Interaction::Pressed && b.is_some())
        {
            let p = state.core.options.skill_bar_positions[hit.0];
            state.skill_bars.dragging =
                Some((hit.0, [cursor.x - p[0] as f32, cursor.y - p[1] as f32]));
        }
    }
    if let Some((bar, offset)) = state.skill_bars.dragging {
        state.skill_bars.hovered = true;
        state.core.options.skill_bar_positions[bar] = [
            ((cursor.x - offset[0]).round() as i32).clamp(0, 808),
            ((cursor.y - offset[1]).round() as i32).clamp(0, 740),
        ];
        if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
            state.skill_bars.dragging = None;
            if let Some(effects) = effects.as_deref_mut() {
                effects.push(mir2_ui_core::effect::UiEffect::PersistOptions {
                    options: state.core.options.clone(),
                });
            }
        }
    }
}

fn icon_size(index: u16) -> Option<Vec2> {
    static SIZES: std::sync::OnceLock<HashMap<u16, Vec2>> = std::sync::OnceLock::new();
    SIZES
        .get_or_init(|| {
            let meta: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../web/public/original-ui/MagIcon/meta.json"
            ))
            .expect("source MagIcon metadata");
            meta["frames"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|f| {
                    Some((
                        u16::try_from(f["index"].as_u64()?).ok()?,
                        Vec2::new(f["width"].as_u64()? as f32, f["height"].as_u64()? as f32),
                    ))
                })
                .collect()
        })
        .get(&index)
        .copied()
}
fn duration(ms: u32) -> String {
    let sec = ms / 1000;
    if ms < 60000 {
        format!("{}.{:01}s", sec, (ms % 1000) / 100)
    } else if ms < 3600000 {
        format!("{}m {:02}s", ms as f64 / 60000., sec % 60)
    } else if ms < 86400000 {
        format!("{}h {:02}m {:02}s", sec / 3600, (sec / 60) % 60, sec % 60)
    } else {
        format!(
            "{}d {}h {:02}m {:02}s",
            sec / 86400,
            (sec / 3600) % 24,
            (sec / 60) % 60,
            sec % 60
        )
    }
}
pub fn render(
    mut commands: Commands,
    roots: Query<Entity, With<OverlayRoot>>,
    old: Query<Entity, With<SkillBarRoot>>,
    state: Res<NativePlayerUiState>,
    skills: Res<SkillModel>,
    shell: Res<NativeShellModel>,
    assets: Option<Res<AssetServer>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let (Ok(root), Some(assets)) = (roots.single(), assets) else {
        return;
    };
    if shell.screen != NativeShellScreen::InGame || !state.core.options.skill_bar {
        return;
    }
    let now = Instant::now();
    for bar in 0..2 {
        if !SkillBarsUi::has_skill(&skills, bar) {
            continue;
        }
        let p = state.core.options.skill_bar_positions[bar];
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    SkillBarRoot,
                    SkillBarHit(bar),
                    SkillBarBackground(bar),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(p[0] as f32),
                        top: Val::Px(p[1] as f32),
                        width: Val::Px(216.),
                        height: Val::Px(28.),
                        ..default()
                    },
                    ImageNode::new(assets.load("original-ui/Prguse/2190.png")),
                    Interaction::None,
                    bevy::ui::FocusPolicy::Block,
                    GlobalZIndex(40),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(12.),
                            top: Val::Px(0.),
                            width: Val::Px(204.),
                            height: Val::Px(28.),
                            ..default()
                        },
                        ImageNode {
                            image: assets.load("original-ui/Prguse/2193.png"),
                            color: Color::srgba(1., 1., 1., 0.5),
                            ..default()
                        },
                        bevy::ui::FocusPolicy::Pass,
                    ));
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(16.),
                            height: Val::Px(28.),
                            ..default()
                        },
                        ImageNode::new(assets.load("original-ui/Prguse/2247.png")),
                        Button,
                        OverlayButton::RefreshSkillBar,
                        SkillBarHit(bar),
                    ));
                    overlay_text_at(
                        parent,
                        &(bar + 1).to_string(),
                        CrystalRect::new(0., 1., 10., 25.),
                        32. / 3.,
                        Color::WHITE,
                    );
                    for cell in 0..8 {
                        let slot = (bar * 8 + cell + 1) as u8;
                        let key = state
                            .keyboard
                            .bindings
                            .iter()
                            .find(|b| b.function == format!("Bar{}Skill{}", bar + 1, cell + 1))
                            .map(|b| b.key.as_str())
                            .unwrap_or("");
                        let skill = skills.skill_for_shortcut(slot);
                        if let Some((skill, binding, index, size)) = skill.and_then(|skill| {
                            let binding = skills.binding_for(skill.id);
                            let index = u16::from(binding.icon?) * 2;
                            Some((skill, binding, index, icon_size(index)?))
                        }) {
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(cell as f32 * 25. + 15.),
                                    top: Val::Px(3.),
                                    width: Val::Px(size.x),
                                    height: Val::Px(size.y),
                                    ..default()
                                },
                                ImageNode::new(
                                    assets.load(format!("original-ui/MagIcon/{index}.png")),
                                ),
                                Button,
                                OverlayButton::CastSkillBar(slot),
                                SkillBarHit(bar),
                                CrystalHint::new(format!(
                                    "{}\nMP: {}\nCooldown: {}\nKey: {}",
                                    skill.name,
                                    binding.mp_cost.unwrap_or(skill.mp_cost),
                                    duration(binding.delay_ms.unwrap_or(skill.cooldown_ms)),
                                    key
                                )),
                            ));
                            if let Some(frame) =
                                state.skill_bars.cooldown_frame(skill.id, &skills, now)
                            {
                                parent.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(cell as f32 * 25. + 15.),
                                        top: Val::Px(3.),
                                        width: Val::Px(24.),
                                        height: Val::Px(22.),
                                        ..default()
                                    },
                                    ImageNode {
                                        image: assets
                                            .load(format!("original-ui/Prguse2/{frame}.png")),
                                        color: Color::srgba(1., 1., 1., 0.6),
                                        ..default()
                                    },
                                    bevy::ui::FocusPolicy::Pass,
                                ));
                            }
                        } else {
                            overlay_text_at(
                                parent,
                                key,
                                CrystalRect::new(cell as f32 * 25. + 13., 0., 25., 25.),
                                32. / 3.,
                                Color::WHITE,
                            );
                        }
                    }
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_source_resources_have_measured_sizes_and_cooldown_duration() {
        assert_eq!(icon_size(0), Some(Vec2::new(24., 22.)));
        assert!(icon_size(224).is_none());
        assert_eq!(duration(2250), "2.2s");
    }
}
