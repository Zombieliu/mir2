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

fn press_real_button(app: &mut App, action: OverlayButton) {
    let id = app
        .world_mut()
        .query::<(Entity, &OverlayButton)>()
        .iter(app.world())
        .find_map(|(id, button)| (*button == action).then_some(id))
        .expect("production overlay button");
    app.world_mut().entity_mut(id).insert(Interaction::Pressed);
    app.update();
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
