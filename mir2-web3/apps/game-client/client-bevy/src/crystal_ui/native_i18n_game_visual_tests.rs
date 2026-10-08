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
        let bag = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<OverlayInventory>>()
            .single(world)
            .map(|(node, transform)| bounds(node, transform))
            .unwrap();
        let shop = world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<OverlayShop>>()
            .single(world)
            .map(|(node, transform)| bounds(node, transform))
            .unwrap();
        assert_eq!(bag, [445.0, 0.0, 761.0, 236.0]);
        assert_eq!(shop, [0.0, 224.0, 440.0, 558.0]);
        assert!(
            shop[2] < bag[0],
            "real service open must leave both windows usable"
        );
        assert!(
            world
                .query_filtered::<&Node, With<OriginalItemImage>>()
                .iter(world)
                .filter(|node| node.display == Display::Flex)
                .count()
                >= 2,
            "the medicine icon must render in both the shop and bag"
        );
        let rows = i18n_text_layouts(&mut app);
        assert!(rows.len() >= 5);
        failures.extend(
            rows.iter()
                .filter(|row| {
                    row["glyphs"] == 0
                        || row["missingGlyphs"] != 0
                        || row["layoutExceedsNode"] != false
                        || row["nodeOutsideViewport"] != false
                })
                .map(|row| json!({"locale":locale.code(),"row":row})),
        );
        let file = format!("{}-medicine-shop.png", locale.code());
        capture_i18n(&mut app, &target, &output.join(&file));
        evidence.push(json!({"file":file,"locale":locale.code(),"bagBounds":bag,"shopBounds":shop,"textRows":rows}));
    }
    native_i18n::activate(previous_locale);
    fs::write(
        output.join("medicine-shop-layouts.json"),
        serde_json::to_vec_pretty(&json!({
            "kind":"offline_production_npc_service_open","liveAcceptance":false,"systemFonts":false,
            "class":"Warrior","level":9,"gold":700,"hpPotions":11,
            "passed":failures.is_empty(),"layoutFailures":failures,"screenshots":evidence,
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "production shop text failures: {failures:?}"
    );
}

#[test]
#[ignore = "explicit delayed NPC snapshot GPU regression; original assets and fresh output"]
fn three_class_medicine_shop_retains_goods_after_delayed_text_hide_offscreen() {
    let asset_root = PathBuf::from(std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("MIR2_SHOP_LIFECYCLE_VISUAL_OUTPUT").unwrap());
    fs::create_dir_all(&output).unwrap();
    let report = output.join("shop-lifecycle-layouts.json");
    assert!(!report.exists(), "retain earlier regression evidence");
    let previous_locale = native_i18n::locale();
    native_i18n::activate(Locale::TraditionalChinese);
    let (mut app, target, _) = i18n_offscreen_app(&asset_root, false);
    app.init_resource::<CasterGpuPress>()
        .add_systems(
            PreUpdate,
            inject_caster_gpu_press.after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(Update, layout_original_item_images.after(render_overlays));
    npc_shop_layout_tests::install_fixture(&mut app);
    let mut captures = Vec::new();
    for class in ["Warrior", "Wizard", "Taoist"] {
        app.insert_resource(NativePlayerUiState::default())
            .insert_resource(ShopModel::default())
            .insert_resource(NpcDialogModel::default())
            .insert_resource(PendingOperations::default())
            .insert_resource(UiSurfaceSignals::default());
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .clear();
        app.world_mut()
            .resource_mut::<UiReadModel>()
            .player
            .class_name = Some(class.into());
        app.update();
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
        app.update();
        npc_shop_layout_tests::request_medicine_shop(&mut app, true);
        warm_i18n_images(&mut app);
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
        for _ in 0..35 {
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(16));
            assert!(app
                .world()
                .resource::<NativePlayerUiState>()
                .npc_shop_open());
        }
        press_caster_gpu_button(&mut app, OverlayButton::SelectShopGood(701));
        warm_i18n_images(&mut app);
        let buy_layout = caster_service_layout(&mut app);
        let rows = i18n_text_layouts(&mut app);
        assert!(rows
            .iter()
            .all(|row| row["missingGlyphs"] == 0 && row["layoutExceedsNode"] == false));
        let file = format!("{}-9-delayed-hide-buy.png", class.to_ascii_lowercase());
        capture_i18n(&mut app, &target, &output.join(&file));
        captures.push(json!({"file":file,"class":class,"phase":"buy-after-text-hide","layout":buy_layout,"textRows":rows}));
        press_caster_gpu_button(&mut app, OverlayButton::ShopBuy);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::BuyItem {
                item_index: 701,
                count: 1
            }]
        );
        press_caster_gpu_button(&mut app, OverlayButton::ShopShowSell);
        warm_i18n_images(&mut app);
        let sell_layout = caster_service_layout(&mut app);
        let file = format!("{}-9-delayed-hide-sell.png", class.to_ascii_lowercase());
        capture_i18n(&mut app, &target, &output.join(&file));
        captures.push(
            json!({"file":file,"class":class,"phase":"sell-after-text-hide","layout":sell_layout}),
        );
        press_caster_gpu_button(&mut app, OverlayButton::ShopShowBuy);
        warm_i18n_images(&mut app);
        press_caster_gpu_button(&mut app, OverlayButton::CloseShop);
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .npc_shop_open());
    }
    native_i18n::activate(previous_locale);
    fs::write(
        report,
        serde_json::to_vec_pretty(&json!({
            "kind":"production_widgets_delayed_npc_text_hide","passed":true,"liveAcceptance":false,
            "textHideAfterService":true,"waitAfterHideMs":560,"viewport":[1024,768],
            "locale":"zh-TW","captures":captures,
        }))
        .unwrap(),
    )
    .unwrap();
}

fn caster_service_layout(app: &mut App) -> serde_json::Value {
    use bevy::ui::UiGlobalTransform;
    let show_buy = app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_buy_tab;
    let bounds = |node: &ComputedNode, transform: &UiGlobalTransform| {
        let min = transform.translation - node.size / 2.0;
        [min.x, min.y, min.x + node.size.x, min.y + node.size.y]
    };
    let world = app.world_mut();
    let (bag_entity, bag) = world
        .query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<OverlayInventory>>()
        .single(world)
        .map(|(entity, node, transform)| (entity, bounds(node, transform)))
        .unwrap();
    let (shop_entity, shop) = world
        .query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<OverlayShop>>()
        .single(world)
        .map(|(entity, node, transform)| (entity, bounds(node, transform)))
        .unwrap();
    assert_eq!(bag, [445.0, 0.0, 761.0, 236.0]);
    assert_eq!(shop, [0.0, 224.0, 440.0, 558.0]);
    assert!(
        shop[2] < bag[0],
        "ordinary service must keep shop and carried bag separate"
    );
    let controls = world
        .query::<(Entity, &OverlayButton, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .filter(|(_, _, node, _)| node.size != Vec2::ZERO)
        .map(|(entity, button, node, transform)| {
            (entity, format!("{button:?}"), bounds(node, transform))
        })
        .collect::<Vec<_>>();
    let mut measured_controls = Vec::new();
    for (entity, action, rect) in controls {
        let mut ancestor = Some(entity);
        let mut enclosing = None;
        while let Some(current) = ancestor {
            if current == bag_entity {
                enclosing = Some(("bag", bag));
                break;
            }
            if current == shop_entity {
                enclosing = Some(("shop", shop));
                break;
            }
            ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
        }
        let Some((surface, outer)) = enclosing else {
            continue;
        };
        assert!(
            rect[0] >= outer[0] - 1.0
                && rect[1] >= outer[1] - 1.0
                && rect[2] <= outer[2] + 1.0
                && rect[3] <= outer[3] + 1.0,
            "production {surface} control outside its usable window: {action} {rect:?}"
        );
        assert!(rect[0] >= 0.0 && rect[1] >= 0.0 && rect[2] <= 1024.0 && rect[3] <= 768.0);
        measured_controls.push(json!({"surface":surface,"action":action,"bounds":rect}));
    }
    // Crystal NPCGoodsDialog owns Close; NPCDropDialog only owns Confirm
    // and Hold. The combined native session adds a Buy navigation control
    // to the Sell frame, so return to the measured Buy list before closing.
    assert_eq!(
        measured_controls
            .iter()
            .any(|control| control["action"] == "CloseShop"),
        show_buy,
        "only the Crystal NPCGoods list has its own Close control"
    );
    if !show_buy {
        assert!(
            measured_controls
                .iter()
                .any(|control| control["action"] == "ShopShowBuy"),
            "the Sell frame must retain a measured return to the closable Buy list"
        );
    }
    assert!(measured_controls
        .iter()
        .any(|control| control["action"] == "InspectBag(0)"));
    json!({"bagBounds":bag,"shopBounds":shop,"controls":measured_controls})
}

#[derive(Resource, Default)]
struct CasterGpuPress(Option<OverlayButton>);

fn inject_caster_gpu_press(
    mut pending: ResMut<CasterGpuPress>,
    mut buttons: Query<(&OverlayButton, &mut Interaction), With<Button>>,
) {
    let Some(action) = pending.0.take() else {
        return;
    };
    let mut matching = buttons.iter_mut().filter(|(button, _)| **button == action);
    let (_, mut interaction) = matching
        .next()
        .expect("rendered enabled production control");
    *interaction = Interaction::Pressed;
    assert!(
        matching.next().is_none(),
        "the fixture must address exactly one production control"
    );
}

fn press_caster_gpu_button(app: &mut App, action: OverlayButton) {
    assert!(app.world().resource::<CasterGpuPress>().0.is_none());
    app.world_mut().resource_mut::<CasterGpuPress>().0 = Some(action);
    app.update();
    assert!(
        app.world().resource::<CasterGpuPress>().0.is_none(),
        "one-shot ECS press must be consumed"
    );
}

#[test]
#[ignore = "explicit offline caster shop GPU fixture; original assets, bundled fonts, fresh output"]
fn caster_medicine_amulet_poison_service_cycles_offscreen() {
    use npc_shop_layout_tests::{prepare_caster_case, request_caster_shop, CASTER_SHOP_CASES};
    let asset_root = PathBuf::from(
        std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").expect("original asset root"),
    );
    let output = PathBuf::from(
        std::env::var_os("MIR2_CASTER_SHOP_VISUAL_OUTPUT").expect("fresh caster output"),
    );
    assert!(output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    let report = output.join("caster-shop-layouts.json");
    assert!(!report.exists(), "retain earlier fixture results");
    fs::write(
        &report,
        b"{\"passed\":false,\"complete\":false,\"liveAcceptance\":false}",
    )
    .unwrap();
    let previous_locale = native_i18n::locale();
    let (mut app, target, _) = i18n_offscreen_app(&asset_root, false);
    app.init_resource::<CasterGpuPress>()
        // DefaultPlugins' UI Focus clears interactions in an application with
        // no window. Deliver this fixture's one-shot ECS press after Focus;
        // the ordinary production Update handler still selects/buys/closes.
        .add_systems(
            PreUpdate,
            inject_caster_gpu_press.after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(Update, layout_original_item_images.after(render_overlays));
    npc_shop_layout_tests::install_fixture(&mut app);
    let mut captures = Vec::new();
    let mut failures = Vec::new();
    for locale in [
        Locale::TraditionalChinese,
        Locale::English,
        Locale::BrazilianPortuguese,
    ] {
        native_i18n::activate(locale);
        for case in CASTER_SHOP_CASES {
            prepare_caster_case(&mut app, case);
            let inventory_before =
                serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap();
            request_caster_shop(&mut app, case);
            warm_i18n_images(&mut app);
            assert!(app.world().resource::<ShopModel>().selected_id.is_none());
            press_caster_gpu_button(
                &mut app,
                OverlayButton::SelectShopGood(case.selected as u64),
            );
            press_caster_gpu_button(&mut app, OverlayButton::ShopQuantityInc);
            warm_i18n_images(&mut app);
            let buy_layout = caster_service_layout(&mut app);
            assert!(
                buy_layout["controls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|control| control["action"] == "ShopBuy"),
                "selected caster stock must be purchasable"
            );
            let rows = i18n_text_layouts(&mut app);
            let template = mir2_game_data::crystal_item_by_index(case.selected).unwrap();
            assert!(
                rows.iter()
                    .any(|row| row["text"] == crate::player_text::name(&template.name)),
                "selected imported supply name must render"
            );
            for row in &rows {
                if row["glyphs"] == 0
                    || row["missingGlyphs"] != 0
                    || row["layoutExceedsNode"] != false
                    || row["nodeOutsideViewport"] != false
                {
                    failures.push(
                        json!({"locale":locale.code(),"case":case.slug,"phase":"buy","row":row}),
                    );
                }
            }
            let buy_file = format!("{}-{}-buy.png", locale.code(), case.slug);
            capture_i18n(&mut app, &target, &output.join(&buy_file));
            captures.push(json!({"file":buy_file,"locale":locale.code(),"class":case.class,
                "level":case.level,"vendor":case.vendor.npc_name(),"phase":"buy","layout":buy_layout,"textRows":rows}));
            press_caster_gpu_button(&mut app, OverlayButton::ShopBuy);
            let purchase = app
                .world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents();
            assert_eq!(
                purchase,
                vec![NativePlayerUiIntent::BuyItem {
                    item_index: case.selected as u64,
                    count: 2
                }]
            );
            assert_eq!(
                serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap(),
                inventory_before,
                "offline UI requests never grant stock or debit gold"
            );
            // Deliberately collide with the Sell frame, then use its real tab
            // input so the production service-transition placement repairs it.
            {
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.inventory_window.left = 300.0;
                state.inventory_window.top = 250.0;
            }
            press_caster_gpu_button(&mut app, OverlayButton::ShopShowSell);
            warm_i18n_images(&mut app);
            let sell_layout = caster_service_layout(&mut app);
            assert!(sell_layout["controls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|control| control["action"] == "ShopShowBuy"));
            let rows = i18n_text_layouts(&mut app);
            for row in &rows {
                if row["glyphs"] == 0
                    || row["missingGlyphs"] != 0
                    || row["layoutExceedsNode"] != false
                    || row["nodeOutsideViewport"] != false
                {
                    failures.push(
                        json!({"locale":locale.code(),"case":case.slug,"phase":"sell","row":row}),
                    );
                }
            }
            let sell_file = format!("{}-{}-sell.png", locale.code(), case.slug);
            capture_i18n(&mut app, &target, &output.join(&sell_file));
            captures.push(json!({"file":sell_file,"locale":locale.code(),"class":case.class,
                "level":case.level,"vendor":case.vendor.npc_name(),"phase":"sell","layout":sell_layout,"textRows":rows}));
            press_caster_gpu_button(&mut app, OverlayButton::ShopShowBuy);
            warm_i18n_images(&mut app);
            assert!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .npc_shop_buy_tab
            );
            caster_service_layout(&mut app);
            press_caster_gpu_button(&mut app, OverlayButton::CloseShop);
            assert!(!app
                .world()
                .resource::<NativePlayerUiState>()
                .npc_shop_open());
            {
                let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
                state.inventory_window.left = 0.0;
                state.inventory_window.top = 0.0;
                state.begin_npc_service_request();
            }
            request_caster_shop(&mut app, case);
            warm_i18n_images(&mut app);
            caster_service_layout(&mut app);
            assert!(app.world().resource::<ShopModel>().selected_id.is_none());
            assert_eq!(
                serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap(),
                inventory_before
            );
            assert!(
                app.world_mut()
                    .resource_mut::<NativePlayerUiIntentQueue>()
                    .drain_intents()
                    .is_empty(),
                "service cycles must not replay the old Buy request"
            );
        }
    }
    native_i18n::activate(previous_locale);
    fs::write(&report, serde_json::to_vec_pretty(&json!({
        "kind":"synthetic_production_caster_service_ui", "liveAcceptance":false,"systemFonts":false,
        "complete":true,"passed":failures.is_empty(),"viewport":[1024,768],
        "locales":["zh-TW","en","pt-BR"],"caseCount":CASTER_SHOP_CASES.len(),
        "flows":["NpcGoods/NpcSell","select imported supply","Buy intent only","Sell tab",
            "Buy tab","Close","service reopen without purchase replay"],
        "layoutFailures":failures,"screenshots":captures,
    })).unwrap()).unwrap();
    assert!(
        failures.is_empty(),
        "caster service text/layout failures: {failures:?}"
    );
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
