use super::*;
use bevy::ecs::system::RunSystemOnce;
fn skills(count: usize) -> SkillModel {
    serde_json::from_value(serde_json::json!({"skills":(0..count).map(|id|serde_json::json!({"id":id,"name":format!("Spell{id}"),"spell":format!("Spell{id}"),"hotkey":0,"icon":0,"level":0,"experience":0,"need1":0})).collect::<Vec<_>>()})).unwrap()
}
const LONG_LOCALIZED_NAME: &str =
    "本地化技能名称必须在它自己的原始文字矩形内裁剪而不是由整个技能页裁剪";
fn fencing(level: u8) -> SkillModel {
    serde_json::from_value(serde_json::json!({"skills":[{"id":37,"name":"Fencing","spell":"Fencing","icon":0,"level":level,"castKind":"passive","canUse":false,"mpCost":999}]})).unwrap()
}
#[test]
fn skills_hover_uses_the_actual_native_description_in_all_ten_locales() {
    for locale in crate::native_i18n::Locale::ALL {
        let doc = crate::native_i18n::with_locale(locale, || {
            skill_tooltip_document(&fencing(1), 37).unwrap()
        });
        assert!(doc.source_complete, "{}", locale.code());
        assert_eq!(
            doc.source_key.as_deref(),
            Some("content.magic.Fencing.description")
        );
        assert!(
            doc.text.lines().count() >= 7,
            "a name alone is not a description"
        );
        assert!(!doc.text.contains('{'));
        assert!(!doc.text.contains("999"));
        if locale == crate::native_i18n::Locale::English {
            assert!(doc.text.contains("Passive Skill"));
            assert!(doc.text.contains("Hitting accuracy"));
            assert!(doc.text.contains("Current Skill Level 1"));
            assert!(doc.text.contains("Next Level 9"));
        }
        if locale == crate::native_i18n::Locale::SimplifiedChinese {
            assert!(doc.text.contains("被动技能"));
            assert!(doc.text.contains("命中率"));
            assert!(doc.text.contains("下一级 9"));
        }
    }
}
#[test]
fn skills_hover_matches_crystal_next_level_selection_including_max_level() {
    for (level, next) in [(0, 7), (1, 9), (2, 12), (3, 0)] {
        let doc = crate::native_i18n::with_locale(crate::native_i18n::Locale::English, || {
            skill_tooltip_document(&fencing(level), 37).unwrap()
        });
        assert!(doc.text.contains(&format!("Current Skill Level {level}")));
        assert!(doc.text.contains(&format!("Next Level {next}")));
    }
}
#[test]
fn skills_hover_missing_or_unknown_source_never_invents_description_or_attributes() {
    let mut model = fencing(2);
    model.bindings[0].spell = Some("FutureSpell".into());
    model.skills[0].name = "Future spell".into();
    let doc = skill_tooltip_document(&model, 37).unwrap();
    assert_eq!(doc.text, "Future spell (2)");
    assert!(!doc.source_complete);
    assert!(doc.source_key.is_none());
    assert!(
        skill_tooltip_document(&model, 999).is_none(),
        "unlearned is not a tooltip source"
    );
}
#[test]
fn skills_hover_corrupt_localized_key_fragment_uses_verified_source_text() {
    let model: SkillModel = serde_json::from_value(serde_json::json!({"skills":[{"id":0,"name":"FireBall","spell":"FireBall","level":0,"mpCost":999}]})).unwrap();
    let doc =
        crate::native_i18n::with_locale(crate::native_i18n::Locale::SimplifiedChinese, || {
            skill_tooltip_document(&model, 0).unwrap()
        });
    assert!(doc.source_complete);
    assert!(!doc.text.contains("SkillDescription"));
    assert!(!doc.text.contains("999"));
    assert!(doc.text.contains("Instant Casting"));
    assert!(doc.text.contains(&format!(
        "Mana Cost {}",
        mir2_game_data::crystal_magic_by_spell("FireBall")
            .unwrap()
            .base_cost
    )));
}
#[test]
fn skills_actual_hover_painter_contains_description_and_keeps_name_non_actionable() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>();
    app.world_mut()
        .run_system_once(|mut commands: Commands, assets: Res<AssetServer>| {
            let plan = SkillPagePlan::new(&fencing(0), 0, |_| 0);
            commands
                .spawn(Node::default())
                .with_children(|parent| paint_page(parent, Some(&assets), &plan, None, (), |a| a));
        })
        .unwrap();
    let world = app.world_mut();
    let targets = world
        .query::<(
            &SkillTooltipTarget,
            &super::super::widget::CrystalHint,
            Option<&SkillPageAction>,
        )>()
        .iter(world)
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2, "real icon and name hover targets");
    assert!(targets
        .iter()
        .all(|(target, hint, _)| target.0 == 37 && hint.0.contains("Hitting accuracy")));
    assert_eq!(
        targets
            .iter()
            .filter(|(_, _, action)| matches!(action, Some(SkillPageAction::Select(37))))
            .count(),
        1
    );
}
#[test]
fn skills_hover_existing_hint_renderer_paints_body_and_retires_on_leave_and_despawn() {
    use super::super::widget::{
        CrystalHintOverlayRoot, CrystalHintOverlayText, Mir2CrystalHintPlugin,
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>()
        .add_plugins(Mir2CrystalHintPlugin);
    let mut window = Window::default();
    window.set_cursor_position(Some(Vec2::new(820., 110.)));
    app.world_mut().spawn((window, bevy::window::PrimaryWindow));
    app.world_mut()
        .run_system_once(|mut commands: Commands, assets: Res<AssetServer>| {
            let plan = SkillPagePlan::new(&fencing(0), 0, |_| 0);
            commands
                .spawn(Node::default())
                .with_children(|parent| paint_page(parent, Some(&assets), &plan, None, (), |a| a));
        })
        .unwrap();
    let target = {
        let world = app.world_mut();
        world
            .query::<(Entity, &SkillTooltipTarget, Option<&SkillPageAction>)>()
            .iter(world)
            .find(|(_, target, action)| target.0 == 37 && action.is_none())
            .unwrap()
            .0
    };
    *app.world_mut().get_mut::<Interaction>(target).unwrap() = Interaction::Hovered;
    app.update();
    assert!(
        app.world_mut()
            .query_filtered::<&Text, With<CrystalHintOverlayText>>()
            .iter(app.world())
            .any(|text| text.0.contains("Passive Skill")
                && text.0.contains("Hitting accuracy")
                && text.0.contains("Next Level 7")),
        "the actual shared hint Text must paint the description, not only the skill name"
    );
    *app.world_mut().get_mut::<Interaction>(target).unwrap() = Interaction::None;
    app.update();
    assert!(app
        .world_mut()
        .query_filtered::<&Visibility, With<CrystalHintOverlayRoot>>()
        .iter(app.world())
        .all(|visibility| *visibility == Visibility::Hidden));
    *app.world_mut().get_mut::<Interaction>(target).unwrap() = Interaction::Hovered;
    app.update();
    app.world_mut().despawn(target);
    app.update();
    assert!(app
        .world_mut()
        .query_filtered::<&Visibility, With<CrystalHintOverlayRoot>>()
        .iter(app.world())
        .all(|visibility| *visibility == Visibility::Hidden));
}
#[test]
fn m10_seven_rows_all_pages_reachable_and_clamped() {
    for count in [0, 7, 8, 14, 15, 512] {
        let s = skills(count);
        let mut ids = Vec::new();
        for page in 0..page_count(count) {
            let plan = SkillPagePlan::new(&s, page, |_| 0);
            assert!(plan.rows.len() <= 7);
            ids.extend(plan.rows.iter().map(|r| r.id));
        }
        assert_eq!(ids, (0..count as u32).collect::<Vec<_>>());
        assert_eq!(
            SkillPagePlan::new(&s, usize::MAX, |_| 0).page,
            page_count(count) - 1
        );
    }
}
#[test]
fn m10_actual_painter_spawns_source_page_and_modal_controls() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>();
    let mut s = skills(8);
    s.skills[0].name = LONG_LOCALIZED_NAME.into();
    app.insert_resource(s);
    app.world_mut()
        .run_system_once(
            |mut commands: Commands, assets: Res<AssetServer>, skills: Res<SkillModel>| {
                let plan = SkillPagePlan::new(&skills, 0, |_| 0);
                let mut draft = SkillAssignUi::default();
                draft.show(0, &skills);
                commands.spawn(Node::default()).with_children(|p| {
                    paint_page(p, Some(&assets), &plan, None, (), |a| a);
                    paint_assignment(p, &assets, &skills, &draft, None, (), |a| a);
                });
            },
        )
        .unwrap();
    let w = app.world_mut();
    assert_eq!(
        w.query::<&SkillPageAction>()
            .iter(w)
            .filter(|a| matches!(a, SkillPageAction::Select(_)))
            .count(),
        7
    );
    assert_eq!(
        w.query::<&SkillPageAction>()
            .iter(w)
            .filter(|a| matches!(a, SkillPageAction::Choose(_)))
            .count(),
        16
    );
    assert!(w.query::<&Text>().iter(w).any(|t| t.0 == "0/0"));
    assert!(w.query::<&ImageNode>().iter(w).any(|n| {
        n.image
            .path()
            .is_some_and(|p| p.to_string() == "original-ui/MagIcon2/0.png")
    }));
    assert!(w.query::<&Node>().iter(w).any(|n| n.width == Val::Px(248.)
        && n.height == Val::Px(241.)
        && n.overflow == Overflow::default()));
    let mut learned_name_seen = false;
    let mut prompt_seen = false;
    for (text, layout, node) in w.query::<(&Text, &TextLayout, &Node)>().iter(w) {
        let (expected, overflow) = if text.0.starts_with("Select the Key for:") {
            prompt_seen = true;
            (
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                Overflow::default(),
            )
        } else {
            if text.0 == LONG_LOCALIZED_NAME {
                learned_name_seen = true;
                assert_eq!((node.width, node.height), (Val::Px(131.), Val::Px(14.)));
            }
            (
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                Overflow::clip(),
            )
        };
        assert_eq!(
            node.overflow, overflow,
            "own text rectangle overflow for {}",
            text.0
        );
        assert_eq!(
            (layout.justify, layout.linebreak),
            (expected.justify, expected.linebreak),
            "text policy for {}",
            text.0
        );
    }
    assert!(learned_name_seen);
    assert!(prompt_seen);
}
