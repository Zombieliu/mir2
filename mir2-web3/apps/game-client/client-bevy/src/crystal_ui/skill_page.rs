//! Native wrapper around the one shared learned-page painter.
use super::*;
use crate::crystal_ui::skill_page_shared::{SkillPagePlan, experience_label, key_label};
pub(super) fn render(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&AssetServer>,
    skills: &SkillModel,
    state: &NativePlayerUiState,
) {
    let now = std::time::Instant::now();
    let plan = SkillPagePlan::new(skills, state.skill_page, |id| {
        state.skill_bars.remaining_ms(id, skills, now)
    });
    crate::crystal_ui::skill_page_shared::paint_page(
        parent,
        assets,
        &plan,
        None,
        OverlaySkillListViewport,
        skill_assign_dialog::map_action,
    );
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
            assert_eq!(
                button.assets.normal,
                format!("original-ui/Prguse/{index}.png")
            );
            assert_eq!(button.assets.hover, button.assets.normal);
            assert_eq!(
                button.assets.pressed,
                format!("original-ui/Prguse/{}.png", index + 1)
            );
            assert_eq!(children.len(), 1);
            let image = world.get::<Node>(children[0]).unwrap();
            assert_eq!((image.left, image.top), (Val::Px(0.), Val::Px(0.)));
            assert_eq!((image.width, image.height), (Val::Px(16.), Val::Px(14.)));
            assert_eq!(
                world
                    .get::<ImageNode>(children[0])
                    .unwrap()
                    .image
                    .path()
                    .unwrap()
                    .to_string(),
                button.assets.normal
            );
        }
        assert_eq!(
            count, 2,
            "both pagination arrows remain visible at page bounds"
        );
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
