use super::*;
use bevy::ecs::system::RunSystemOnce;
fn skills(count: usize) -> SkillModel {
    serde_json::from_value(serde_json::json!({"skills":(0..count).map(|id|serde_json::json!({"id":id,"name":format!("Spell{id}"),"spell":format!("Spell{id}"),"hotkey":0,"icon":0,"level":0,"experience":0,"need1":0})).collect::<Vec<_>>()})).unwrap()
}
const LONG_LOCALIZED_NAME: &str =
    "本地化技能名称必须在它自己的原始文字矩形内裁剪而不是由整个技能页裁剪";
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
