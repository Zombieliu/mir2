use super::super::*;
use super::*;
use crate::crystal_ui::hud_layout::{
    HudLayoutMode, HudLayoutSpec, ReferenceHudPositions, SkillBarGeometry,
};
use crate::crystal_ui::widget::CrystalHint;
use std::collections::HashMap;
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
    layout: Option<Res<HudLayoutSpec>>,
    mut reference_positions: Option<ResMut<ReferenceHudPositions>>,
    guard: Option<Res<crate::crystal_ui::overlays::HudLayoutInputGuard>>,
) {
    let reference = layout
        .as_deref()
        .is_some_and(|s| s.mode == HudLayoutMode::ReferenceLayoutV1);
    let positions = if reference {
        reference_positions
            .as_deref()
            .cloned()
            .unwrap_or_default()
            .skill_bars
    } else {
        state.core.options.skill_bar_positions
    };
    state.skill_bars.observe(&skills, Instant::now());
    state.skill_bars.visible = [
        SkillBarsUi::has_skill(&skills, 0),
        SkillBarsUi::has_skill(&skills, 1),
    ];
    state.skill_bars.hovered = false;
    if guard.as_deref().is_some_and(|g| g.switched_this_frame) {
        state.skill_bars.dragging = None;
        state.skill_bars.armed_slot = None;
        return;
    }
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
            let rect = SkillBarGeometry.cell(positions[bar], cell);
            if state.skill_bars.hovered && rect.contains(cursor.x, cursor.y) {
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
            let p = positions[hit.0];
            state.skill_bars.dragging =
                Some((hit.0, [cursor.x - p[0] as f32, cursor.y - p[1] as f32]));
        }
    }
    if let Some((bar, offset)) = state.skill_bars.dragging {
        state.skill_bars.hovered = true;
        let next = SkillBarGeometry.clamp([
            (cursor.x - offset[0]).round() as i32,
            (cursor.y - offset[1]).round() as i32,
        ]);
        if reference {
            if let Some(reference_positions) = reference_positions.as_deref_mut() {
                if reference_positions.skill_bars[bar] != next {
                    reference_positions.skill_bars[bar] = next;
                    reference_positions.dirty = true;
                }
            }
        } else {
            state.core.options.skill_bar_positions[bar] = next;
        }
        if mouse.just_released(MouseButton::Left) || !mouse.pressed(MouseButton::Left) {
            state.skill_bars.dragging = None;
            if !reference {
                if let Some(effects) = effects.as_deref_mut() {
                    effects.push(mir2_ui_core::effect::UiEffect::PersistOptions {
                        options: state.core.options.clone(),
                    });
                }
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
    layout: Option<Res<HudLayoutSpec>>,
    reference_positions: Option<Res<ReferenceHudPositions>>,
) {
    let reference = layout
        .as_deref()
        .is_some_and(|s| s.mode == HudLayoutMode::ReferenceLayoutV1);
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
        let p = if reference {
            reference_positions
                .as_deref()
                .cloned()
                .unwrap_or_default()
                .skill_bars[bar]
        } else {
            state.core.options.skill_bar_positions[bar]
        };
        let frame = SkillBarGeometry.frame(p);
        commands.entity(root).with_children(|parent| {
            parent
                .spawn((
                    SkillBarRoot,
                    SkillBarHit(bar),
                    SkillBarBackground(bar),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(frame.left),
                        top: Val::Px(frame.top),
                        width: Val::Px(frame.width),
                        height: Val::Px(frame.height),
                        ..default()
                    },
                    if reference {
                        ImageNode {
                            color: Color::NONE,
                            ..default()
                        }
                    } else {
                        ImageNode::new(assets.load("original-ui/Prguse/2190.png"))
                    },
                    BackgroundColor(if reference {
                        Color::srgb(0.10, 0.09, 0.075)
                    } else {
                        Color::NONE
                    }),
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
                            image: if reference {
                                Handle::default()
                            } else {
                                assets.load("original-ui/Prguse/2193.png")
                            },
                            color: if reference {
                                Color::NONE
                            } else {
                                Color::srgba(1., 1., 1., 0.5)
                            },
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
                        if reference {
                            ImageNode {
                                color: Color::NONE,
                                ..default()
                            }
                        } else {
                            ImageNode::new(assets.load("original-ui/Prguse/2247.png"))
                        },
                        BackgroundColor(if reference {
                            Color::srgb(0.6, 0.4, 0.18)
                        } else {
                            Color::NONE
                        }),
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
                        let cell_rect = SkillBarGeometry.cell([0, 0], cell);
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
                                    left: Val::Px(cell_rect.left),
                                    top: Val::Px(cell_rect.top),
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
                                CrystalHint::new(crate::native_i18n::tr(&format!(
                                    "{}\nMP: {}\nCooldown: {}\nKey: {}",
                                    crate::player_text::name(&skill.name),
                                    binding.mp_cost.unwrap_or(skill.mp_cost),
                                    duration(binding.delay_ms.unwrap_or(skill.cooldown_ms)),
                                    key
                                ))),
                            ));
                            if let Some(frame) =
                                state.skill_bars.cooldown_frame(skill.id, &skills, now)
                            {
                                parent.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Px(cell_rect.left),
                                        top: Val::Px(cell_rect.top),
                                        width: Val::Px(cell_rect.width),
                                        height: Val::Px(cell_rect.height),
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

    #[test]
    fn reference_drag_keeps_original_positions_and_uses_the_same_clamped_geometry() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let mut ui = NativePlayerUiState::default();
        ui.core.options.skill_bar = true;
        ui.core.options.skill_bar_positions = [[20, 30], [40, 50]];
        ui.skill_bars.dragging = Some((0, [0., 0.]));
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        let mut window = Window {
            focused: true,
            resolution: (1024, 768).into(),
            ..default()
        };
        window.resolution.set_scale_factor_override(Some(1.));
        window.set_cursor_position(Some(Vec2::new(900., 760.)));
        app.world_mut().spawn((window, PrimaryWindow));
        app.insert_resource(ui)
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..default()
            })
            .insert_resource(SkillModel::default())
            .insert_resource(mouse)
            .insert_resource(HudLayoutSpec::reference())
            .init_resource::<ReferenceHudPositions>()
            .add_systems(Update, process);
        app.update();
        let new = app.world().resource::<ReferenceHudPositions>();
        assert_eq!(new.skill_bars[0], [808, 740]);
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .core
                .options
                .skill_bar_positions,
            [[20, 30], [40, 50]]
        );
        let cell = SkillBarGeometry.cell(new.skill_bars[0], 7);
        assert_eq!(cell, CrystalRect::new(998., 743., 24., 22.));
        assert!(cell.contains(1000., 750.));
    }

    #[test]
    fn reference_keeps_transparent_final_cooldown_frame_and_sub_100ms_gate() {
        let now = Instant::now();
        let skills: SkillModel = serde_json::from_value(serde_json::json!({"skills":[{"id":1,"name":"Healing","spell":"Healing","hotkey":1,"icon":1,"cooldown_ms":6600,"castSequence":7}]})).unwrap();
        let mut ui = SkillBarsUi::default();
        ui.observe(&skills, now);
        assert_eq!(ui.cooldown_frame(1, &skills, now), Some(1260));
        assert_eq!(
            ui.cooldown_frame(1, &skills, now + std::time::Duration::from_millis(6500)),
            Some(1282)
        );
        assert_eq!(
            ui.cooldown_frame(1, &skills, now + std::time::Duration::from_millis(6501)),
            None
        );
    }
}
