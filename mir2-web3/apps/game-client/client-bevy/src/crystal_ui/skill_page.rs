//! Crystal CharacterDialog.SkillPage and MainDialogs.MagicButton.
use super::*;

fn key_label(key: Option<i32>) -> String {
    match key {
        Some(key @ 1..=8) => format!("F{key}"),
        Some(key @ 9..=16) => format!("CTRL\nF{}", key - 8),
        _ => String::new(),
    }
}

fn experience_label(level: u8, binding: &crate::skill_model::SkillBinding) -> String {
    let needed = match level {
        0 => binding.need1,
        1 => binding.need2,
        2 => binding.need3,
        3 => return "-".into(),
        _ => return String::new(),
    };
    match (binding.experience, needed) {
        (Some(exp), Some(needed)) => format!("{exp}/{needed}"),
        _ => String::new(),
    }
}

pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    skills: &SkillModel,
    state: &NativePlayerUiState,
) {
    parent
        .spawn((
            OverlaySkillListViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.),
                top: Val::Px(90.),
                width: Val::Px(248.),
                height: Val::Px(241.),
                ..default()
            },
        ))
        .with_children(|rows| render_rows(rows, assets, skills, state));
    // Coordinates include SkillPage's (8,90) offset. Original buttons remain
    // visible at both ends; the click handler guards the page bounds.
    if let Some(assets) = assets {
        for (x, index, action) in [
            (98., 398, OverlayButton::SkillPagePrev),
            (148., 396, OverlayButton::SkillPageNext),
        ] {
            // Prguse 396..399 stores 16x14 art with 13x14 nontransparent
            // bounds. Crystal AutoSize uses those bounds for pointer hits,
            // while DrawControl draws the original image at the same origin.
            let spec = CrystalButtonSpec::new(
                "Prguse",
                index,
                index,
                index + 1,
                CrystalRect::new(x, 340., 13., 14.),
                16.,
                14.,
            );
            spawn_crystal_image_button(
                parent,
                assets,
                spec,
                CrystalButtonAssetSet::from_spec(spec),
                action,
                false,
                true,
            );
        }
    }
}

fn render_rows(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    skills: &SkillModel,
    state: &NativePlayerUiState,
) {
    let page = state
        .skill_page
        .min(native_skill_page_count(skills.skills.len()).saturating_sub(1));
    for (row, skill) in skills
        .skills
        .iter()
        .skip(page * SKILL_PAGE_SIZE)
        .take(SKILL_PAGE_SIZE)
        .enumerate()
    {
        let binding = skills.binding_for(skill.id);
        let x = (SKILL_ROW_ORIGIN.x - 8) as f32;
        let y = (SKILL_ROW_ORIGIN.y - 90 + row as i32 * SKILL_ROW_STEP_Y) as f32;
        // Only the icon is the assign-key button. Passive skills and insufficient
        // MP do not turn this configuration control into a cast/disabled button.
        if let (Some(assets), Some(icon)) = (assets, binding.icon) {
            let index = u16::from(icon) * 2;
            spawn_overlay_crystal_button(
                parent,
                assets,
                "MagIcon2",
                index,
                index,
                index + 1,
                CrystalRect::new(x + 36., y, 36., 34.),
                OverlayButton::SelectSkill(skill.id),
            );
            let remaining =
                state
                    .skill_bars
                    .remaining_ms(skill.id, skills, std::time::Instant::now());
            let delay = binding.delay_ms.unwrap_or(0);
            if remaining >= 100 && delay >= 34 {
                let frame = 1290 + 34 - (remaining / (delay / 34)).min(34) as u16;
                parent.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x + 36.),
                        top: Val::Px(y),
                        width: Val::Px(36.),
                        height: Val::Px(34.),
                        ..default()
                    },
                    ImageNode {
                        image: assets.load(format!("original-ui/Prguse2/{frame}.png")),
                        color: Color::srgba(1., 1., 1., 0.6),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ));
            }
        }
        if let Some(assets) = assets {
            for (index, dy, height) in [(516, 7., 9.), (517, 19., 11.)] {
                spawn_static_overlay_sprite(
                    parent,
                    assets,
                    format!("original-ui/Title/{index}.png"),
                    CrystalRect::new(x + 73., y + dy, 24., height),
                );
            }
        }
        overlay_text_at(
            parent,
            &skill.level.to_string(),
            CrystalRect::new(x + 88., y + 2., 21., 14.),
            32. / 3.,
            Color::WHITE,
        );
        overlay_text_at(
            parent,
            &skill.name,
            CrystalRect::new(x + 109., y + 2., 131., 14.),
            32. / 3.,
            Color::WHITE,
        );
        overlay_text_at(
            parent,
            &experience_label(skill.level, &binding),
            CrystalRect::new(x + 109., y + 15., 131., 14.),
            32. / 3.,
            Color::WHITE,
        );
        overlay_text_at(
            parent,
            &key_label(binding.hotkey),
            CrystalRect::new(x + 2., y + 2., 34., 31.),
            32. / 3.,
            Color::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn learned_skill_renders_real_icon_full_name_and_experience_even_when_passive() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        app.world_mut()
            .run_system_once(|mut commands: Commands, assets: Res<AssetServer>| {
                let skills: SkillModel = serde_json::from_value(serde_json::json!({"skills": [{
                    "id": 17, "name": "Summon Skeleton", "level": 1, "icon": 7,
                    "hotkey": 9, "experience": 27, "need2": 100,
                    "castKind": "passive", "canUse": false
                }]}))
                .unwrap();
                commands
                    .spawn(Node::default())
                    .with_children(|p| render(p, Some(&assets), &skills, &Default::default()));
            })
            .unwrap();
        let world = app.world_mut();
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains(&"Summon Skeleton".into()));
        assert!(texts.contains(&"27/100".into()));
        assert!(texts.contains(&"CTRL\nF1".into()));
        assert!(!texts.iter().any(|t| t.contains("ready")
            || t.contains("locked")
            || t.contains("low MP")
            || t.contains("Lv")));
        let images: Vec<_> = world
            .query::<&ImageNode>()
            .iter(world)
            .filter_map(|image| image.image.path().map(|p| p.to_string()))
            .collect();
        assert!(images.contains(&"original-ui/MagIcon2/14.png".into()));
        assert!(images.contains(&"original-ui/Title/516.png".into()));
        assert!(images.contains(&"original-ui/Title/517.png".into()));
        let nodes: Vec<_> = world
            .query::<(&OverlayButton, &Node)>()
            .iter(world)
            .filter(|(action, _)| matches!(action, OverlayButton::SelectSkill(17)))
            .collect();
        assert_eq!(nodes.len(), 1, "only the source icon is clickable");
        assert_eq!(nodes[0].1.left, Val::Px(44.));
        assert_eq!(nodes[0].1.top, Val::Px(8.));
        assert_eq!(nodes[0].1.width, Val::Px(36.));
        // Verify actual spawned hit nodes and art nodes separately: the
        // generic overlay helper used to stretch both arrows to 32x24.
        let mut arrows = world.query::<(&OverlayButton, &Node, &Children, &CrystalImageButton)>();
        let mut count = 0;
        for (action, hit, children, button) in arrows.iter(world) {
            let (x, index) = match action {
                OverlayButton::SkillPagePrev => (98., 398),
                OverlayButton::SkillPageNext => (148., 396),
                _ => continue,
            };
            count += 1;
            assert_eq!((hit.left, hit.top), (Val::Px(x), Val::Px(340.)));
            assert_eq!((hit.width, hit.height), (Val::Px(13.), Val::Px(14.)));
            assert_eq!(button.assets.normal, format!("original-ui/Prguse/{index}.png"));
            assert_eq!(button.assets.hover, button.assets.normal);
            assert_eq!(button.assets.pressed, format!("original-ui/Prguse/{}.png", index + 1));
            assert_eq!(children.len(), 1);
            let image = world.get::<Node>(children[0]).unwrap();
            assert_eq!((image.left, image.top), (Val::Px(0.), Val::Px(0.)));
            assert_eq!((image.width, image.height), (Val::Px(16.), Val::Px(14.)));
            assert_eq!(world.get::<ImageNode>(children[0]).unwrap().image.path().unwrap().to_string(), button.assets.normal);
        }
        assert_eq!(count, 2, "both pagination arrows remain visible at page bounds");
    }
    #[test]
    fn source_keys_use_two_lines_and_no_unbound_placeholder() {
        assert_eq!(key_label(Some(9)), "CTRL\nF1");
        assert_eq!(key_label(Some(16)), "CTRL\nF8");
        assert_eq!(key_label(Some(8)), "F8");
        for key in [None, Some(0), Some(17)] {
            assert!(key_label(key).is_empty());
        }
    }
    #[test]
    fn source_experience_uses_level_specific_threshold_without_fabricating_data() {
        let binding = crate::skill_model::SkillBinding {
            experience: Some(27),
            need1: Some(50),
            need2: Some(100),
            need3: Some(200),
            ..default()
        };
        assert_eq!(experience_label(0, &binding), "27/50");
        assert_eq!(experience_label(1, &binding), "27/100");
        assert_eq!(experience_label(2, &binding), "27/200");
        assert_eq!(experience_label(3, &binding), "-");
        assert!(experience_label(0, &Default::default()).is_empty());
    }
}
