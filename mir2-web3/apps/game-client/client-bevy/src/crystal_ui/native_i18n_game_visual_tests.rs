//! Production widgets rendered into a texture, never a live character session.
use super::*;
use crate::native_i18n::{self, Locale};
use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::native_shell_ui::i18n_visual_tests::{
    capture_i18n, i18n_offscreen_app, i18n_text_layouts, warm_i18n_images,
};
use serde_json::json;
use std::{fs, path::PathBuf};

#[test]
#[ignore = "explicit offline GPU fixture; real service request, packaged assets and fresh output"]
fn nine_locale_medicine_shop_opens_beside_inventory_offscreen() {
    use bevy::ui::UiGlobalTransform;
    let asset_root = PathBuf::from(std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("MIR2_GAME_I18N_VISUAL_OUTPUT").unwrap());
    assert!(output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    assert!(!output.join("medicine-shop-layouts.json").exists());
    let previous_locale = native_i18n::locale();
    let (mut app, target, _) = i18n_offscreen_app(&asset_root, false);
    app.add_systems(Update, layout_original_item_images.after(render_overlays));
    npc_shop_layout_tests::install_fixture(&mut app);
    let mut evidence = Vec::new();
    let mut failures = Vec::new();
    for locale in Locale::ALL {
        native_i18n::activate(locale);
        npc_shop_layout_tests::request_medicine_shop(&mut app, false);
        warm_i18n_images(&mut app);
        let bounds = |node: &ComputedNode, transform: &UiGlobalTransform| {
            let min = transform.translation - node.size / 2.0;
            [min.x, min.y, min.x + node.size.x, min.y + node.size.y]
        };
        let world = app.world_mut();
        let bag = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<OverlayInventory>>()
            .single(world).map(|(node, transform)| bounds(node, transform)).unwrap();
        let shop = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<OverlayShop>>()
            .single(world).map(|(node, transform)| bounds(node, transform)).unwrap();
        assert_eq!(bag, [445.0, 0.0, 761.0, 236.0]);
        assert_eq!(shop, [0.0, 224.0, 440.0, 558.0]);
        assert!(shop[2] < bag[0], "real service open must leave both windows usable");
        assert!(world.query_filtered::<&Node, With<OriginalItemImage>>().iter(world)
            .filter(|node| node.display == Display::Flex).count() >= 2,
            "the medicine icon must render in both the shop and bag");
        let rows = i18n_text_layouts(&mut app);
        assert!(rows.len() >= 5);
        failures.extend(rows.iter().filter(|row| row["glyphs"] == 0 || row["missingGlyphs"] != 0
            || row["layoutExceedsNode"] != false || row["nodeOutsideViewport"] != false)
            .map(|row| json!({"locale":locale.code(),"row":row})));
        let file = format!("{}-medicine-shop.png", locale.code());
        capture_i18n(&mut app, &target, &output.join(&file));
        evidence.push(json!({"file":file,"locale":locale.code(),"bagBounds":bag,"shopBounds":shop,"textRows":rows}));
    }
    native_i18n::activate(previous_locale);
    fs::write(output.join("medicine-shop-layouts.json"), serde_json::to_vec_pretty(&json!({
        "kind":"offline_production_npc_service_open","liveAcceptance":false,"systemFonts":false,
        "class":"Warrior","level":9,"gold":700,"hpPotions":11,
        "passed":failures.is_empty(),"layoutFailures":failures,"screenshots":evidence,
    })).unwrap()).unwrap();
    assert!(failures.is_empty(), "production shop text failures: {failures:?}");
}

fn fixture_node(left: f32, top: f32, width: f32, height: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(width),
        height: Val::Px(height),
        ..default()
    }
}

fn spawn_case(app: &mut App, case: &str) -> Entity {
    let assets = app.world().resource::<AssetServer>().clone();
    let inventory = InventoryModel {
        gold: 123456,
        items: vec![crate::inventory::ItemModel {
            unique_id: Some(917),
            key: "(HP)PotionSmall".into(),
            name: "(HP)PotionSmall".into(),
            quantity: 10,
            slot: 6,
            icon: 315,
            ..default()
        }],
        ..default()
    };
    let mut ui = UiReadModel::default();
    ui.player.name = Some("Gold".into());
    ui.player.class_name = Some("Warrior".into());
    ui.player.level = 30;
    let mut state = NativePlayerUiState::default();
    state.npc_shop_buy_tab = true;
    let mut commands = app.world_mut().commands();
    let mut root = commands.spawn(fixture_node(0.0, 0.0, 1024.0, 768.0));
    let id = root.id();
    root.with_children(|parent| match case {
        "options-shop-bag" => {
            parent
                .spawn(fixture_node(20.0, 30.0, 259.0, 540.0))
                .with_children(|parent| render_options(parent, Some(&assets), &state.core));
            let shop = ShopModel {
                supports_buy: true,
                selected_id: Some(701),
                goods: vec![
                    crate::shop::ShopGood {
                        unique_id: 701,
                        name: "(HP)PotionSmall".into(),
                        price: 350,
                        count: 1,
                        stock: -1,
                        icon: 315,
                        ..default()
                    },
                    crate::shop::ShopGood {
                        unique_id: 702,
                        name: "TownTeleport".into(),
                        price: 500,
                        count: 1,
                        stock: -1,
                        icon: 402,
                        ..default()
                    },
                ],
                ..default()
            };
            parent
                .spawn(fixture_node(310.0, 30.0, 244.0, 334.0))
                .with_children(|parent| {
                    render_shop(
                        parent,
                        Some(&assets),
                        &shop,
                        &ShopUiState::default(),
                        &inventory,
                        &state,
                        &ui.player,
                    )
                });
            parent
                .spawn(fixture_node(590.0, 30.0, 310.0, 360.0))
                .with_children(|parent| {
                    render_inventory(
                        parent,
                        Some(&assets),
                        &inventory,
                        &ui,
                        &state,
                        &InventoryOperationFeedback::default(),
                        &crate::social::SocialModel::default(),
                        None,
                    )
                });
        }
        "character-stats" => {
            for (page, left) in [
                (CharacterPage::Stats1, 30.0),
                (CharacterPage::Stats2, 330.0),
            ] {
                state.character_page = page;
                parent
                    .spawn(fixture_node(left, 30.0, 264.0, 380.0))
                    .with_children(|parent| {
                        render_equipment(
                            parent,
                            Some(&assets),
                            None,
                            &inventory,
                            &ui,
                            &state,
                            &SkillModel::default(),
                        )
                    });
            }
        }
        "help" | "help-detail" => {
            parent
                .spawn(fixture_node(20.0, 30.0, 540.0, 510.0))
                .with_children(|parent| {
                    render_help(
                        parent,
                        Some(&assets),
                        if case == "help" { 0 } else { 44 },
                        &state.keyboard,
                    )
                });
        }
        _ => panic!("unknown fixture case"),
    });
    id
}

#[test]
#[ignore = "explicit native real-asset multilingual GPU layout fixture; run alone"]
fn multilingual_game_widgets_render_offscreen() {
    let asset_root =
        PathBuf::from(std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("asset root"));
    let output = PathBuf::from(
        std::env::var_os("MIR2_I18N_VISUAL_OUTPUT").expect("fresh evidence directory"),
    );
    assert!(asset_root.is_absolute() && output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    let report_path = output.join("game-i18n-report.json");
    assert!(!report_path.exists(), "do not overwrite previous evidence");
    fs::write(&report_path, b"{\"passed\":false,\"complete\":false}").unwrap();
    let (mut app, target, _) = i18n_offscreen_app(&asset_root, false);
    app.add_systems(Update, layout_original_item_images);
    app.insert_resource(NativeShellModel {
        screen: NativeShellScreen::InGame,
        ..default()
    });
    let mut reports = Vec::new();
    for language in Locale::ALL {
        native_i18n::activate(language);
        for case in ["options-shop-bag", "character-stats", "help", "help-detail"] {
            let root = spawn_case(&mut app, case);
            warm_i18n_images(&mut app);
            let rows = i18n_text_layouts(&mut app);
            assert!(!rows.is_empty());
            capture_i18n(
                &mut app,
                &target,
                &output.join(format!("game-{}-{case}.png", language.code())),
            );
            reports.push(json!({"locale": language.code(), "case": case, "text": rows}));
            app.world_mut().entity_mut(root).despawn();
        }
    }
    // Save measurements before asserting so a failure stays independently inspectable.
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&json!({
            "passed": false, "complete": true, "render": "production widgets, offline GPU",
            "locales":Locale::ALL.len(), "systemFonts":false, "screens": reports,
        }))
        .unwrap(),
    )
    .unwrap();
    for report in &reports {
        for row in report["text"].as_array().unwrap() {
            assert!(
                !row["nodeOutsideViewport"].as_bool().unwrap(),
                "outside viewport: {report}"
            );
            assert!(
                !row["layoutExceedsNode"].as_bool().unwrap(),
                "text overflow: {row}"
            );
            assert!(
                row["glyphs"].as_u64().unwrap() > 0,
                "missing glyph layout: {row}"
            );
            assert_eq!(row["missingGlyphs"], 0, "missing bundled glyph: {row}");
        }
    }
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&json!({
            "passed": true, "complete": true, "render": "production widgets, offline GPU",
            "locales":Locale::ALL.len(), "systemFonts":false, "screens": reports,
        }))
        .unwrap(),
    )
    .unwrap();
}
