#![cfg(any(feature = "native-ui", feature = "portable-quest-ui"))]
use mir2_client_bevy::crystal_ui::{character_stats, panel_navigation::*, shared_hud::*};
use mir2_client_bevy::read_model::*;

fn player(class: &str, level: u32) -> UiReadModel {
    UiReadModel {
        player: PlayerStats {
            hp: 0,
            max_hp: 0,
            mp: 12,
            max_mp: 18,
            gold: 4_294_967_295,
            name: Some("角色".into()),
            class_name: Some(class.into()),
            level,
            experience: 1,
            max_experience: 3,
            crystal_stats: Some(vec![CrystalPlayerStatModel {
                stat: 16,
                value: 80,
            }]),
            weights: Some(PlayerWeights {
                bag: u32::MAX,
                wear: 0,
                hand: 45,
            }),
            ..Default::default()
        },
    }
}
#[test]
fn common_hud_plan_preserves_crystal_values_rectangles_and_assets() {
    for (class, level, hp_only) in [
        ("Warrior", 25, true),
        ("Warrior", 26, false),
        ("Wizard", 25, false),
        ("Taoist", 25, false),
    ] {
        let model = player(class, level);
        let plan = main_hud_plan(&model);
        assert_eq!(plan.hp, "HP 0/0");
        assert_eq!(plan.hp_only, hp_only);
        assert_eq!(plan.mp, if hp_only { "" } else { "MP 12/18 " });
        assert_eq!(plan.gold, "4,294,967,295");
        assert_eq!(plan.experience, "33.33%");
        assert_eq!(plan.weight, "0");
        assert_eq!(plan.experience_sprite.as_ref().unwrap().source.width, 333.);
        assert_eq!(
            plan.weight_sprite.as_ref().unwrap().current,
            i64::from(u32::MAX)
        );
        assert_eq!(plan.weight_sprite.as_ref().unwrap().source.width, 74.);
        assert_eq!(
            plan.main,
            (mir2_client_bevy::crystal_ui::spec::hud::MAIN.rect).into()
        );
        assert_eq!(plan.buttons[0].normal, "original-ui/Prguse/1900.png");
        assert_eq!(plan.buttons[3].action, "quest");
        assert!(plan.buttons[3].rect.contains(974., 692.));
        assert_eq!(plan.character_hits.len(), 5);
        assert!(plan
            .character_hits
            .iter()
            .any(|h| h.action == PanelAction::CloseCharacter && h.rect.contains(1007., 12.)));
    }
}

#[test]
fn native_and_portable_share_the_vertical_centered_text_hierarchy_and_source_buttons() {
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::text::FontSource;
    for portable in [false, true] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>();
        let server = app.world().resource::<AssetServer>().clone();
        let model = player("Wizard", 26);
        let font = portable.then(|| Handle::<Font>::default());
        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                commands
                    .spawn(Node::default())
                    .with_children(|parent| spawn_main_hud(parent, &server, &model, font.as_ref()));
            })
            .unwrap();
        let mut query = app
            .world_mut()
            .query::<(Entity, &Text, &TextFont, &Node, &ChildOf)>();
        let rows = query
            .iter(app.world())
            .map(|(e, t, f, n, p)| (e, t.0.clone(), f.clone(), n.clone(), p.parent()))
            .collect::<Vec<_>>();
        for marker in ["HP 0/0", "MP 12/18 ", "角色", "4,294,967,295"] {
            let (_, _, font, node, parent) = rows.iter().find(|r| r.1 == marker).unwrap();
            assert_eq!(node.position_type, PositionType::Relative);
            assert_eq!(node.width, Val::Auto);
            let parent = app.world().get::<Node>(*parent).unwrap();
            assert_eq!(parent.position_type, PositionType::Absolute);
            assert_eq!(parent.overflow, Overflow::clip());
            assert_eq!(parent.align_items, AlignItems::Center);
            assert_eq!(
                parent.justify_content,
                if marker == "4,294,967,295" {
                    JustifyContent::FlexStart
                } else {
                    JustifyContent::Center
                }
            );
            assert_eq!(matches!(font.font, FontSource::Handle(_)), portable);
        }
        let mut buttons = app.world_mut().query::<(&CrystalHudAction, &Node)>();
        for (spec, action, _) in main_buttons() {
            let (_, node) = buttons
                .iter(app.world())
                .find(|(a, _)| **a == action)
                .unwrap();
            assert_eq!(node.left, Val::Px(spec.rect.left));
            assert_eq!(node.top, Val::Px(spec.rect.top));
        }
    }
}

#[test]
fn character_header_and_stats_rows_use_one_native_portable_painter() {
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    let mut player = player("Wizard", 26).player;
    player.guild_name = Some(" Guild ".into());
    player.guild_rank_name = Some(" Leader ".into());
    assert_eq!(character_stats::guild_label(&player), "Guild Leader");
    assert_eq!(
        character_stats::class_image_index(player.class_name.as_deref()),
        Some(101)
    );
    let mut snapshots = vec![];
    for portable in [false, true] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>();
        let server = app.world().resource::<AssetServer>().clone();
        let p = player.clone();
        let font = portable.then(|| Handle::<Font>::default());
        app.world_mut()
            .run_system_once(move |mut commands: Commands| {
                commands.spawn(Node::default()).with_children(|parent| {
                    character_stats::spawn_character_header(
                        parent,
                        Some(&server),
                        &p,
                        font.as_ref(),
                    );
                    character_stats::spawn_authoritative_rows(parent, &p, true, font.as_ref());
                });
            })
            .unwrap();
        let mut texts = app.world_mut().query::<(&Text, &Node, Option<&ChildOf>)>();
        let rows = texts
            .iter(app.world())
            .map(|(t, n, parent)| {
                (
                    t.0.clone(),
                    n.left,
                    n.top,
                    parent
                        .and_then(|p| app.world().get::<Node>(p.parent()))
                        .map(|n| (n.top, n.align_items)),
                )
            })
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|(s, _, _, layout)| s == "Guild Leader"
            && *layout == Some((Val::Px(33.), AlignItems::Center))));
        snapshots.push(rows);
    }
    assert_eq!(snapshots[0], snapshots[1]);
}
#[test]
fn authoritative_stats_keep_missing_block_distinct_from_missing_id_and_u32_weights() {
    let mut model = player("Wizard", 10);
    let rows = character_stats::authoritative_lines(&model.player, true);
    assert_eq!(rows[1], ("4294967295/80".into(), 128.));
    assert_eq!(rows[2].0, "0/0");
    assert_eq!(rows[3].0, "45/0");
    assert_eq!(rows[11].1, 308.);
    model.player.crystal_stats = Some(vec![]);
    assert_eq!(
        character_stats::authoritative_lines(&model.player, false)[2].0,
        "0-0"
    );
    model.player.weights = None;
    assert_eq!(main_hud_plan(&model).weight, "");
    assert!(
        character_stats::authoritative_lines(&model.player, true)[1..4]
            .iter()
            .all(|r| r.0.is_empty())
    );
    model.player.crystal_stats = None;
    assert!(character_stats::authoritative_lines(&model.player, false).is_empty());
}
#[test]
fn character_action_restores_stats_pages_then_closes_and_other_entries_are_local() {
    for page in [
        CharacterPage::Stats1,
        CharacterPage::Stats2,
        CharacterPage::Spells,
    ] {
        let state = reduce(
            PanelNavigation::default(),
            PanelAction::SelectCharacterPage(page),
        );
        let restored = reduce(state, PanelAction::Character);
        assert!(restored.character_open);
        assert_eq!(restored.character_page, CharacterPage::Character);
        assert!(!reduce(restored, PanelAction::Character).character_open);
    }
    let open = reduce(
        reduce(PanelNavigation::default(), PanelAction::Bag),
        PanelAction::Quest,
    );
    assert!(open.bag_open && open.quest_open);
    assert!(!reduce(open, PanelAction::CloseBag).bag_open);
}
#[test]
fn complete_player_ingress_survives_quest_cleanup_and_rejects_old_lifetimes() {
    let mut ingress = UiReadModelIngress::default();
    let mut model = UiReadModel::default();
    assert!(ingress.apply_full(&mut model, 7, 1, Some(player("Wizard", 10).player)));
    let before = serde_json::to_string(&model).unwrap();
    assert!(!ingress.apply_legacy_quest(&mut model, 7, 999, PlayerStats::default()));
    assert_eq!(serde_json::to_string(&model).unwrap(), before);
    assert!(!ingress.apply_full(&mut model, 7, 1, None));
    assert!(ingress.apply_legacy_quest(&mut model, 8, 999, PlayerStats::default()));
    assert!(!ingress.has_complete_generation(7));
    assert!(!ingress.has_complete_generation(8));
    assert!(ingress.apply_full(&mut model, 8, 1, Some(player("Taoist", 11).player)));
    assert!(!ingress.apply_full(&mut model, 7, 5000, None));
    assert_eq!(model.player.class_name.as_deref(), Some("Taoist"));
    assert!(ingress.apply_full(&mut model, 8, 2, None));
    assert_eq!(model.player, PlayerStats::default());
}
