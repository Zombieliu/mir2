//! Exercises real service-open input and overlay roots, not manually positioned
//! shop/bag widgets. The same fixture is available to the offline GPU harness.
use super::*;

pub(crate) fn install_fixture(app: &mut App) {
    app.init_resource::<NativePlayerUiState>()
        .init_resource::<InventoryModel>()
        .init_resource::<InventoryOperationFeedback>()
        .init_resource::<MailModel>()
        .init_resource::<MailComposeUi>()
        .init_resource::<MapModel>()
        .init_resource::<UiReadModel>()
        .init_resource::<UiSurfaceSignals>()
        .init_resource::<NpcDialogModel>()
        .init_resource::<ShopModel>()
        .init_resource::<GameShopModel>()
        .init_resource::<StorageModel>()
        .init_resource::<PendingOperations>()
        .init_resource::<NativePlayerUiIntentQueue>()
        .init_resource::<NativeUiIntentQueue>()
        .init_resource::<crate::social::SocialModel>()
        .init_resource::<ButtonInput<KeyCode>>()
        .add_message::<KeyboardInput>()
        .insert_resource(NativeShellModel {
            screen: NativeShellScreen::InGame,
            ..default()
        });
    tests::init_overlay_button_test_resources(app);
    app.add_systems(Startup, spawn_overlay_root).add_systems(
        Update,
        (
            sync_npc_dialog_inventory_location,
            process_overlay_keyboard,
            process_overlay_buttons,
            sync_npc_shop_inventory_location,
            render_overlays,
        )
            .chain(),
    );
    let mut ui = app.world_mut().resource_mut::<UiReadModel>();
    ui.player.level = 9;
    ui.player.class_name = Some("Warrior".into());
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    let mut inventory = app.world_mut().resource_mut::<InventoryModel>();
    inventory.gold = 700;
    inventory.items.push(ItemModel {
        unique_id: Some(917),
        key: format!("crystal-item-{}", template.item_index),
        name: template.name,
        quantity: 11,
        slot: 0,
        icon: template.image,
        ..default()
    });
}

pub(crate) fn request_medicine_shop(app: &mut App, sell_too: bool) {
    let template = mir2_game_data::crystal_item_by_index(658).unwrap();
    let mut shop = app.world_mut().resource_mut::<ShopModel>();
    shop.goods = vec![ShopGood {
        unique_id: 701,
        name: template.name,
        price: template.price,
        count: 1,
        stock: -1,
        icon: template.image,
        ..default()
    }];
    assert!(shop.apply_service_signal(NpcShopServiceSignal {
        mode: NpcShopServiceMode::Buy,
        repair_rate: None,
    }));
    if sell_too {
        assert!(shop.apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        }));
    }
    shop.selected_id = Some(701);
    app.world_mut()
        .resource_mut::<UiSurfaceSignals>()
        .npc_shop_open_requested = true;
}

fn fixture_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<Image>();
    install_fixture(&mut app);
    app
}

fn bag_position(app: &App) -> Vec2 {
    let bag = &app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_window;
    Vec2::new(bag.left, bag.top)
}

fn move_bag(app: &mut App, position: Vec2) {
    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.inventory_window.left = position.x;
    state.inventory_window.top = position.y;
}

pub(crate) fn press_real_button(app: &mut App, action: OverlayButton) {
    let id = app
        .world_mut()
        .query::<(Entity, &OverlayButton)>()
        .iter(app.world())
        .find_map(|(id, button)| (*button == action).then_some(id))
        .expect("production overlay button");
    app.world_mut().entity_mut(id).insert(Interaction::Pressed);
    app.update();
}

/// Synthetic read-model cases using the real imported supply templates. These
/// exercise ordinary production services/buttons, never a grant or live save.
#[derive(Clone, Copy)]
pub(crate) struct CasterShopCase {
    pub slug: &'static str,
    pub class: &'static str,
    pub level: u32,
    pub vendor: crate::quest_supplies::SupplyVendor,
    pub catalog: &'static [i32],
    pub selected: i32,
}

pub(crate) const CASTER_SHOP_CASES: [CasterShopCase; 7] = [
    CasterShopCase {
        slug: "wizard-9-medicine",
        class: "Wizard",
        level: 9,
        vendor: crate::quest_supplies::SupplyVendor::Potions,
        catalog: &[658, 659],
        selected: 659,
    },
    CasterShopCase {
        slug: "wizard-28-medicine",
        class: "Wizard",
        level: 28,
        vendor: crate::quest_supplies::SupplyVendor::Potions,
        catalog: &[658, 659, 660, 661, 662, 663],
        selected: 663,
    },
    CasterShopCase {
        slug: "wizard-28-scrolls",
        class: "Wizard",
        level: 28,
        vendor: crate::quest_supplies::SupplyVendor::General,
        catalog: &[717, 719],
        selected: 719,
    },
    CasterShopCase {
        slug: "taoist-9-medicine",
        class: "Taoist",
        level: 9,
        vendor: crate::quest_supplies::SupplyVendor::Potions,
        catalog: &[658, 659],
        selected: 659,
    },
    CasterShopCase {
        slug: "taoist-28-medicine",
        class: "Taoist",
        level: 28,
        vendor: crate::quest_supplies::SupplyVendor::Potions,
        catalog: &[658, 659, 660, 661, 662, 663],
        selected: 663,
    },
    CasterShopCase {
        slug: "taoist-28-amulets",
        class: "Taoist",
        level: 28,
        vendor: crate::quest_supplies::SupplyVendor::General,
        catalog: &[712, 717, 719],
        selected: 712,
    },
    CasterShopCase {
        slug: "taoist-28-poison",
        class: "Taoist",
        level: 28,
        vendor: crate::quest_supplies::SupplyVendor::Poison,
        catalog: &[710, 711],
        selected: 710,
    },
];

fn source_item(index: i32, count: u16, container: u8, slot: u32) -> ItemModel {
    let template = mir2_game_data::crystal_item_by_index(index).unwrap();
    let unique_id = 10_000 + index as u64;
    ItemModel {
        unique_id: Some(unique_id),
        key: format!("crystal-item-{index}"),
        name: template.name.clone(),
        quantity: u32::from(count),
        container,
        slot,
        icon: template.image,
        shape: u16::try_from(template.shape).ok(),
        durability_current: Some(template.durability),
        durability_max: Some(template.durability),
        equip_slot: (template.item_type == 8).then(|| "Amulet".into()),
        tooltip_source: Some(crate::inventory::CrystalItemTooltipSourceModel {
            info: serde_json::from_value(serde_json::to_value(&template).unwrap()).unwrap(),
            user_item: Some(crate::inventory::CrystalUserItemModel {
                unique_id,
                item_index: index,
                count,
                current_dura: template.durability,
                max_dura: template.durability,
                ..default()
            }),
            ..default()
        }),
        ..default()
    }
}

pub(crate) fn prepare_caster_case(app: &mut App, case: CasterShopCase) {
    let mut state = NativePlayerUiState::default();
    state.core.screen = mir2_ui_core::state::UiScreen::InGame;
    app.insert_resource(state)
        .insert_resource(ShopModel::default())
        .insert_resource(NpcDialogModel::default())
        .insert_resource(PendingOperations::default())
        .insert_resource(UiSurfaceSignals::default());
    app.world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .clear();
    // Observe the closed surface before the next service signal, including
    // when one offscreen application is reused for multiple independent cases.
    app.update();
    let gold = if case.level == 9 { 700 } else { 25_000 };
    {
        let mut ui = app.world_mut().resource_mut::<UiReadModel>();
        ui.player.level = case.level;
        ui.player.class_name = Some(case.class.into());
        ui.player.gender = Some("Male".into());
        ui.player.gold = gold;
    }
    let late = case.level >= 25;
    let mut inventory = InventoryModel { gold, ..default() };
    inventory.items = vec![
        source_item(if late { 662 } else { 658 }, 11, 0, 0),
        source_item(if late { 663 } else { 659 }, 8, 1, 0),
        source_item(719, 2, 0, 1),
        source_item(717, 2, 0, 2),
    ];
    if case.class == "Taoist" && case.level >= 18 {
        inventory.items.push(source_item(712, 20, 2, 9));
        inventory.items.push(source_item(710, 5, 0, 3));
        inventory.items.push(source_item(711, 4, 0, 4));
    }
    app.insert_resource(inventory);
}

pub(crate) fn request_caster_shop(app: &mut App, case: CasterShopCase) {
    let goods = case
        .catalog
        .iter()
        .map(|&index| {
            let item = source_item(index, 1, 0, 0);
            let mut source = item.tooltip_source.unwrap();
            let user_item = source.user_item.as_mut().unwrap();
            user_item.unique_id = index as u64;
            user_item.is_shop_item = true;
            ShopGood {
                unique_id: index as u64,
                name: item.name,
                price: source.info.price,
                count: 1,
                stock: -1,
                icon: item.icon,
                tooltip_source: Some(source),
                ..default()
            }
        })
        .collect();
    let mut shop = app.world_mut().resource_mut::<ShopModel>();
    shop.goods = goods;
    assert!(shop.apply_service_signal(NpcShopServiceSignal {
        mode: NpcShopServiceMode::Buy,
        repair_rate: None,
    }));
    assert!(shop.apply_service_signal(NpcShopServiceSignal {
        mode: NpcShopServiceMode::Sell,
        repair_rate: None,
    }));
    app.world_mut()
        .resource_mut::<UiSurfaceSignals>()
        .npc_shop_open_requested = true;
}

fn assert_case_stock(app: &App, case: CasterShopCase) {
    let inventory = app.world().resource::<InventoryModel>();
    assert_eq!(
        app.world()
            .resource::<UiReadModel>()
            .player
            .class_name
            .as_deref(),
        Some(case.class)
    );
    assert!(inventory
        .items
        .iter()
        .any(|item| item.container == 1 && item.slot == 0));
    assert_eq!(
        inventory
            .items
            .iter()
            .any(|item| item.container == 2 && item.slot == 9),
        case.class == "Taoist" && case.level >= 18,
        "only an eligible late Taoist fixture may have equipped materials"
    );
    let supplies = crate::quest_supplies::plan(
        &app.world().resource::<UiReadModel>().player,
        inventory,
        None,
        (case.level >= 25).then_some(2_110_021),
    );
    assert_eq!(
        supplies
            .rows
            .iter()
            .find(|row| row.label == "蓝药")
            .unwrap()
            .held,
        8
    );
    if case.class == "Taoist" && case.level >= 18 {
        assert_eq!(
            supplies
                .rows
                .iter()
                .find(|row| row.label == "护身符")
                .unwrap()
                .held,
            20
        );
        assert_eq!(
            supplies
                .rows
                .iter()
                .find(|row| row.label == "毒粉")
                .unwrap()
                .held,
            9
        );
    } else {
        assert!(!supplies
            .rows
            .iter()
            .any(|row| matches!(row.label, "护身符" | "毒粉")));
    }
}

#[test]
fn caster_medicine_and_material_services_keep_buy_sell_close_reopen_usable() {
    for case in CASTER_SHOP_CASES {
        let mut app = fixture_app();
        prepare_caster_case(&mut app, case);
        assert_case_stock(&app, case);
        let stock_before = serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap();
        request_caster_shop(&mut app, case);
        app.update();
        assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0), "{}", case.slug);
        assert!(app.world().resource::<ShopModel>().selected_id.is_none());
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![]
        );
        press_real_button(
            &mut app,
            OverlayButton::SelectShopGood(case.selected as u64),
        );
        press_real_button(&mut app, OverlayButton::ShopQuantityInc);
        press_real_button(&mut app, OverlayButton::ShopBuy);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::BuyItem {
                item_index: case.selected as u64,
                count: 2
            }],
            "{} must emit the selected canonical goods request",
            case.slug
        );
        // Sending a request must not optimistically grant stock or change gold.
        assert_eq!(
            serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap(),
            stock_before
        );
        move_bag(&mut app, Vec2::new(300.0, 250.0));
        press_real_button(&mut app, OverlayButton::ShopShowSell);
        assert!(
            !app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab
        );
        assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
        assert!(
            app.world_mut()
                .query::<&OverlayButton>()
                .iter(app.world())
                .any(|action| *action == OverlayButton::InspectBag(0)),
            "sell must retain bag item input"
        );
        press_real_button(&mut app, OverlayButton::ShopShowBuy);
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab
        );
        press_real_button(&mut app, OverlayButton::CloseShop);
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .npc_shop_open());
        assert!(!app.world().resource::<ShopModel>().allows_buy());
        move_bag(&mut app, Vec2::ZERO);
        // The native sender accepts a new service request before its reply.
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .begin_npc_service_request();
        request_caster_shop(&mut app, case);
        app.update();
        assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
        assert!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_buy_tab
        );
        assert!(app.world().resource::<ShopModel>().selected_id.is_none());
        press_real_button(
            &mut app,
            OverlayButton::SelectShopGood(case.selected as u64),
        );
        assert_eq!(
            app.world().resource::<ShopModel>().selected_id,
            Some(case.selected as u64)
        );
        assert_eq!(
            serde_json::to_value(app.world().resource::<InventoryModel>()).unwrap(),
            stock_before
        );
        assert!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents()
                .is_empty(),
            "tab switches/close/reopen/select must not replay an old purchase"
        );
    }
}

#[test]
fn direct_medicine_service_opens_separate_bag_and_usable_buy_controls_in_one_frame() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, false);
    app.update();
    assert!(!app.world().resource::<NpcDialogModel>().is_open);
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
    let inventory = app
        .world_mut()
        .query_filtered::<&Node, With<OverlayInventory>>()
        .single(app.world())
        .unwrap();
    assert_eq!(inventory.display, Display::Flex);
    assert_eq!(
        (inventory.left, inventory.top),
        (Val::Px(445.0), Val::Px(0.0))
    );
    assert_eq!(
        (inventory.width, inventory.height),
        (Val::Px(316.0), Val::Px(236.0))
    );
    let shop = app
        .world_mut()
        .query_filtered::<&Node, With<OverlayShop>>()
        .single(app.world())
        .unwrap();
    assert_eq!(shop.display, Display::Flex);
    assert_eq!((shop.left, shop.top), (Val::Px(0.0), Val::Px(224.0)));
    assert_eq!((shop.width, shop.height), (Val::Px(440.0), Val::Px(334.0)));
    assert!(app
        .world_mut()
        .query::<&OverlayButton>()
        .iter(app.world())
        .any(|button| *button == OverlayButton::InspectBag(0)));
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    press_real_button(&mut app, OverlayButton::ShopBuy);
    assert_eq!(
        app.world_mut()
            .resource_mut::<NativePlayerUiIntentQueue>()
            .drain_intents(),
        vec![NativePlayerUiIntent::BuyItem {
            item_index: 701,
            count: 1
        }]
    );
}

#[test]
fn medicine_service_survives_delayed_parent_text_hide_for_all_three_classes() {
    for class in ["Warrior", "Wizard", "Taoist"] {
        let mut app = fixture_app();
        app.world_mut()
            .resource_mut::<UiReadModel>()
            .player
            .class_name = Some(class.into());
        // Actual ordinary level-nine receipts: an open Samuel page is followed
        // by NPCGoods/NPCSell and then activeNpcDialog:null in the next snapshot.
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
        app.update();
        request_medicine_shop(&mut app, true);
        app.update();
        assert!(app
            .world()
            .resource::<NativePlayerUiState>()
            .npc_shop_open());
        app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
        for _ in 0..30 {
            app.update();
            assert!(
                app.world()
                    .resource::<NativePlayerUiState>()
                    .npc_shop_open(),
                "{class}: hiding the text page must retain the receipted service"
            );
            assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0), "{class}");
            assert_eq!(app.world().resource::<ShopModel>().goods.len(), 1);
            let node = app
                .world_mut()
                .query_filtered::<&Node, With<OverlayShop>>()
                .single(app.world())
                .unwrap();
            assert_eq!(node.display, Display::Flex);
        }
        press_real_button(&mut app, OverlayButton::ShopBuy);
        assert_eq!(
            app.world_mut()
                .resource_mut::<NativePlayerUiIntentQueue>()
                .drain_intents(),
            vec![NativePlayerUiIntent::BuyItem {
                item_index: 701,
                count: 1
            }]
        );
        press_real_button(&mut app, OverlayButton::CloseShop);
        assert!(!app
            .world()
            .resource::<NativePlayerUiState>()
            .npc_shop_open());
        assert_eq!(
            app.world().resource::<ShopModel>().service_mode,
            NpcShopServiceMode::Closed
        );
    }
}

#[test]
fn medicine_service_opens_when_text_hide_and_goods_arrive_in_the_same_frame() {
    let mut app = fixture_app();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
    app.update();
    request_medicine_shop(&mut app, true);
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
    app.update();
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
    assert!(app.world().resource::<ShopModel>().allows_buy());
}

#[test]
fn explicit_npc_exit_closes_service_with_an_already_hidden_text_page() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, true);
    app.update();
    assert!(!app.world().resource::<NpcDialogModel>().is_open);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .npc_service_exit_requested = true;
    // A queued service-open signal must not reopen the service after exit.
    app.world_mut()
        .resource_mut::<UiSurfaceSignals>()
        .npc_shop_open_requested = true;
    app.update();
    let state = app.world().resource::<NativePlayerUiState>();
    assert!(!state.npc_shop_open());
    assert_eq!(state.core.panel, mir2_ui_core::state::UiPanel::Inventory);
    assert_eq!(bag_position(&app), Vec2::ZERO);
    assert!(!state.npc_service_exit_requested);
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
}

#[test]
fn delayed_text_hide_does_not_leave_a_closed_service_modal_after_scene_reset() {
    let mut app = fixture_app();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = true;
    app.update();
    request_medicine_shop(&mut app, true);
    app.update();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
    app.update();
    app.world_mut()
        .resource_mut::<ShopModel>()
        .apply_service_signal(NpcShopServiceSignal::default());
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
}

#[test]
fn escape_after_text_hide_drops_old_buy_capability_before_sell_only_service() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, true);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
    // The next merchant opens a new text page before advertising Sell only.
    let mut dialog = app.world_mut().resource_mut::<NpcDialogModel>();
    dialog.is_open = true;
    dialog.npc_object_id = Some(21);
    drop(dialog);
    app.update();
    app.world_mut()
        .resource_mut::<ShopModel>()
        .apply_service_signal(NpcShopServiceSignal {
            mode: NpcShopServiceMode::Sell,
            repair_rate: None,
        });
    app.world_mut()
        .resource_mut::<UiSurfaceSignals>()
        .npc_shop_open_requested = true;
    app.update();
    assert!(app.world().resource::<ShopModel>().allows_sell());
    assert!(!app.world().resource::<ShopModel>().allows_buy());
}

#[test]
fn new_npc_text_page_cannot_keep_the_previous_merchants_buy_service() {
    let mut app = fixture_app();
    {
        let mut dialog = app.world_mut().resource_mut::<NpcDialogModel>();
        dialog.is_open = true;
        dialog.npc_object_id = Some(20);
    }
    app.update();
    request_medicine_shop(&mut app, true);
    app.update();
    app.world_mut().resource_mut::<NpcDialogModel>().is_open = false;
    app.update();
    {
        let mut dialog = app.world_mut().resource_mut::<NpcDialogModel>();
        dialog.is_open = true;
        dialog.npc_object_id = Some(21);
    }
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert!(!app.world().resource::<ShopModel>().allows_buy());
}

#[test]
fn ordered_exit_closes_real_service_and_rejects_late_goods_but_a_new_request_can_reopen() {
    for exit_last in [true, false] {
        let mut app = fixture_app();
        request_medicine_shop(&mut app, true);
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
            state.request_npc_service_exit();
            if exit_last {
                state.begin_npc_service_request();
                state.request_npc_service_exit();
            } else {
                state.begin_npc_service_request();
            }
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativePlayerUiState>()
                .npc_shop_open(),
            !exit_last
        );
        if exit_last {
            assert_eq!(
                app.world().resource::<ShopModel>().service_mode,
                NpcShopServiceMode::Closed
            );
            assert_eq!(bag_position(&app), Vec2::ZERO);
            request_medicine_shop(&mut app, true);
            app.update();
            assert!(!app
                .world()
                .resource::<NativePlayerUiState>()
                .npc_shop_open());
            app.world_mut()
                .resource_mut::<NativePlayerUiState>()
                .begin_npc_service_request();
            request_medicine_shop(&mut app, true);
            app.update();
            assert!(app
                .world()
                .resource::<NativePlayerUiState>()
                .npc_shop_open());
            assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
        }
    }
}

#[test]
fn late_goods_after_explicit_exit_cannot_reopen_the_previous_service() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, true);
    app.update();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .npc_service_exit_requested = true;
    app.update();
    app.update();
    app.update();
    request_medicine_shop(&mut app, true);
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
}

#[test]
fn map_information_epoch_closes_an_old_service_without_a_scene_reset_packet() {
    let mut app = fixture_app();
    app.insert_resource(BigMapModel::default());
    request_medicine_shop(&mut app, true);
    app.update();
    app.world_mut().resource_mut::<BigMapModel>().reset_epoch += 1;
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert_eq!(
        app.world().resource::<ShopModel>().service_mode,
        NpcShopServiceMode::Closed
    );
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .begin_npc_service_request();
    request_medicine_shop(&mut app, true);
    app.update();
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
}

#[test]
fn service_open_preserves_visible_bag_positions_and_repairs_collisions_or_offstage_state() {
    for (old, expected) in [
        (Vec2::new(520.0, 200.0), Vec2::new(520.0, 200.0)),
        (Vec2::new(300.0, 400.0), Vec2::new(300.0, 400.0)),
        (Vec2::new(0.0, 0.0), Vec2::new(445.0, 0.0)),
        (Vec2::new(120.0, 300.0), Vec2::new(445.0, 0.0)),
        (Vec2::new(900.0, 600.0), Vec2::new(445.0, 0.0)),
        (Vec2::new(f32::NAN, 0.0), Vec2::new(445.0, 0.0)),
    ] {
        let mut app = fixture_app();
        app.world_mut()
            .resource_mut::<NativePlayerUiState>()
            .toggle_inventory();
        move_bag(&mut app, old);
        request_medicine_shop(&mut app, false);
        app.update();
        assert_eq!(bag_position(&app), expected, "previous bag at {old:?}");
    }
}

#[test]
fn shop_tab_switch_and_bag_reopen_repair_overlap_but_idle_updates_preserve_manual_drag() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, true);
    app.update();
    // This position clears Buy but intersects the Drop/Sell frame.
    move_bag(&mut app, Vec2::new(300.0, 250.0));
    app.update();
    assert_eq!(bag_position(&app), Vec2::new(300.0, 250.0));
    press_real_button(&mut app, OverlayButton::ShopShowSell);
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
    move_bag(&mut app, Vec2::new(100.0, 300.0));
    app.update();
    assert_eq!(bag_position(&app), Vec2::new(100.0, 300.0));
    press_real_button(&mut app, OverlayButton::ShopShowBuy);
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
    move_bag(&mut app, Vec2::new(50.0, 250.0));
    app.update();
    assert_eq!(bag_position(&app), Vec2::new(50.0, 250.0));
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .toggle_inventory();
    app.update();
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .inventory_open());
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .toggle_inventory();
    app.update();
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
    press_real_button(&mut app, OverlayButton::CloseShop);
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    move_bag(&mut app, Vec2::ZERO);
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .begin_npc_service_request();
    request_medicine_shop(&mut app, false);
    app.update();
    assert_eq!(bag_position(&app), Vec2::new(445.0, 0.0));
}

#[test]
fn paired_service_and_bag_remain_visible_across_supported_viewport_transforms() {
    let mut app = fixture_app();
    request_medicine_shop(&mut app, true);
    app.update();
    for (width, height) in [
        (640.0, 480.0),
        (1024.0, 768.0),
        (1280.0, 720.0),
        (1536.0, 1152.0),
    ] {
        let transform = super::super::metrics::CrystalStageTransform::fit(width, height);
        for (left, top, right, bottom) in [(0.0, 224.0, 440.0, 558.0), (445.0, 0.0, 761.0, 236.0)] {
            let min = transform.logical_to_physical(left, top);
            let max = transform.logical_to_physical(right, bottom);
            assert!(min.0 >= 0.0 && min.1 >= 0.0);
            assert!(max.0 <= width && max.1 <= height);
        }
    }
}
