//! Native NPC surface regressions using the production renderer/button system.
//! No server purchase acknowledgement or transport permission is fabricated.
use super::*;
use crate::inventory::{CrystalItemInfoModel, CrystalItemTooltipSourceModel, CrystalUserItemModel};
use crate::npc_gold_buy_attempt::{
    npc_gold_buy_model_authority, NpcGoldBuyAttemptOutcome, NpcGoldBuyAttemptPhase,
    NpcGoldBuyConnectionEpoch, NpcGoldBuyGate, NpcGoldBuyTicket,
};

fn shop() -> ShopModel {
    ShopModel {
        service_mode: NpcShopServiceMode::Sell,
        supports_buy: true,
        supports_sell: true,
        selected_id: Some(71),
        goods: vec![ShopGood {
            unique_id: 71,
            name: "Potion".into(),
            price: 3,
            count: 1,
            stock: -1,
            purchase_rate: Some(1.0),
            requires_gold_buy_plan: true,
            tooltip_source: Some(CrystalItemTooltipSourceModel {
                info: CrystalItemInfoModel {
                    item_index: 658,
                    name: "Potion".into(),
                    price: 3,
                    stack_size: 99,
                    item_type: 13,
                    ..Default::default()
                },
                user_item: Some(CrystalUserItemModel {
                    unique_id: 71,
                    item_index: 658,
                    count: 1,
                    is_shop_item: true,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn open(app: &mut App) -> NpcGoldBuyGate {
    *app.world_mut().resource_mut::<ShopModel>() = shop();
    app.world_mut().resource_mut::<InventoryModel>().gold = 100;
    let gate = app
        .world()
        .resource::<NativePlayerUiIntentQueue>()
        .npc_gold_buy_gate();
    let mut state = app.world_mut().resource_mut::<NativePlayerUiState>();
    state.bind_npc_gold_buy_gate(gate.clone());
    state.core.panel = mir2_ui_core::state::UiPanel::NpcShop;
    state.npc_shop_buy_tab = true;
    drop(state);
    app.world_mut().resource_mut::<ShopUiState>().common_gate = Some(gate.clone());
    gate
}

fn press_waiting_close(app: &mut App, gate: NpcGoldBuyGate, action: OverlayButton) {
    let entity = app
        .world_mut()
        .spawn((
            Button,
            Interaction::Pressed,
            action,
            NativeNpcShopWaitingClose,
            NativeNpcShopSource { gate },
        ))
        .id();
    app.update();
    app.world_mut().despawn(entity);
}

#[test]
fn npc_shop_missing_stamp_keeps_original_frame_received_goods_and_local_exit_visible() {
    let mut app = tests::overlay_render_test_app();
    let gate = open(&mut app);
    assert!(gate.ui_authority_revision().is_none());
    app.update();
    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&NativeNpcShopWaitingPanel>()
            .iter(world)
            .count(),
        1
    );
    let panel = world
        .query_filtered::<&Node, With<NativeNpcShopWaitingPanel>>()
        .single(world)
        .unwrap();
    assert_eq!(
        (panel.width, panel.height),
        (Val::Px(244.0), Val::Px(334.0))
    );
    assert!(world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0 == "Potion"));
    assert_eq!(
        world
            .query_filtered::<&OverlayButton, (With<Button>, With<NativeNpcShopWaitingClose>)>()
            .iter(world)
            .copied()
            .collect::<Vec<_>>(),
        vec![OverlayButton::CloseShop]
    );
    assert!(world
        .query::<&OverlayButton>()
        .iter(world)
        .all(|action| !matches!(
            action,
            OverlayButton::ShopBuy | OverlayButton::ShopConfirm | OverlayButton::SelectShopGood(_)
        )));
    assert!(world
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn npc_shop_ready_buysell_source_paints_usable_original_goods_instead_of_waiting_shell() {
    let mut app = tests::overlay_render_test_app();
    let gate = open(&mut app);
    let mut source = app.world().resource::<ShopModel>().clone();
    source.service_mode = NpcShopServiceMode::Buy;
    let inventory = app.world().resource::<InventoryModel>();
    let model = npc_gold_buy_model_authority(&source, inventory).unwrap();
    assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch {
        run: 1,
        connection: 1
    }));
    assert!(gate.observe("same-complete-owner", 1, &model, true));
    assert!(prepare_npc_shop_button_fixture(&mut app).is_some());
    app.update();
    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&NativeNpcShopWaitingPanel>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(
        world
            .query::<&super::super::shop_paint::ShopPaintPanel>()
            .iter(world)
            .count(),
        1
    );
    assert!(world
        .query_filtered::<&OverlayButton, With<Button>>()
        .iter(world)
        .any(|action| *action == OverlayButton::ShopBuy));
}

#[test]
fn npc_shop_waiting_exit_closes_without_purchase_and_cannot_be_reused_as_buy() {
    let mut app = tests::help_button_test_app();
    let gate = open(&mut app);
    press_waiting_close(&mut app, gate.clone(), OverlayButton::ShopBuy);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    press_waiting_close(&mut app, gate, OverlayButton::CloseShop);
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn npc_shop_old_waiting_exit_cannot_cancel_a_new_ready_source_or_another_gate() {
    let mut app = tests::help_button_test_app();
    let gate = open(&mut app);
    press_waiting_close(
        &mut app,
        NpcGoldBuyGate::default(),
        OverlayButton::CloseShop,
    );
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch {
        run: 1,
        connection: 1
    }));
    let model = npc_gold_buy_model_authority(
        app.world().resource::<ShopModel>(),
        app.world().resource::<InventoryModel>(),
    )
    .unwrap();
    assert!(gate.observe("real-current-owner", 1, &model, true));
    press_waiting_close(&mut app, gate, OverlayButton::CloseShop);
    assert!(app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
}

#[test]
fn npc_shop_waiting_close_and_reopen_preserve_the_real_unknown_purchase_barrier() {
    let mut app = tests::help_button_test_app();
    let gate = open(&mut app);
    assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch {
        run: 1,
        connection: 1
    }));
    let original = app.world().resource::<ShopModel>().clone();
    let inventory = app.world().resource::<InventoryModel>().clone();
    let model = npc_gold_buy_model_authority(&original, &inventory).unwrap();
    assert!(gate.observe("same-owner", 1, &model, true));
    let (token, _) = gate.reserve(&original, &inventory, 1).unwrap();
    let ticket = NpcGoldBuyTicket {
        run: 1,
        connection: 1,
        procedure: 1,
        owner_epoch: 1,
        scene_epoch: 1,
        cancellation: 1,
        actor: Some(10),
        map: Some(0),
        sequence: token.value(),
        local_attempt_token: token.value(),
        source_revision: 1,
    };
    assert!(gate.bind(token, ticket));
    assert_eq!(gate.commit(ticket, || ()), Some(()));
    assert!(gate.receipt(ticket, NpcGoldBuyAttemptOutcome::Unknown));
    // An oversized replacement has no valid model key/stamp. This reproduces
    // the unavailable shell while retaining a genuinely entered old attempt.
    app.world_mut().resource_mut::<ShopModel>().goods[0].description = "x".repeat(1024 * 1024 + 1);
    press_waiting_close(&mut app, gate.clone(), OverlayButton::CloseShop);
    assert!(!app
        .world()
        .resource::<NativePlayerUiState>()
        .npc_shop_open());
    assert_eq!(gate.feedback().phase, Some(NpcGoldBuyAttemptPhase::Unknown));
    assert!(gate.feedback().pending);
    *app.world_mut().resource_mut::<ShopModel>() = original.clone();
    app.world_mut()
        .resource_mut::<NativePlayerUiState>()
        .core
        .panel = mir2_ui_core::state::UiPanel::NpcShop;
    assert!(gate.observe("same-owner", 1, &model, true));
    assert!(gate.reserve(&original, &inventory, 1).is_none());
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
}

#[test]
fn npc_shop_synchronizing_message_uses_keyed_simplified_chinese_overlay() {
    assert_eq!(
        crate::native_i18n::for_locale(
            crate::native_i18n::Locale::SimplifiedChinese,
            "npc.shop.synchronizing",
            "Synchronizing shop data..."
        ),
        "正在同步商店数据……"
    );
}

#[test]
#[ignore = "explicit offline GPU fixture with packaged assets and a fresh output directory"]
fn npc_shop_buysell_ready_and_waiting_windows_offscreen() {
    use crate::native_shell_ui::i18n_visual_tests::{
        capture_i18n, i18n_offscreen_app_configured, warm_i18n_images,
    };
    use std::{fs, path::PathBuf};
    let assets = PathBuf::from(std::env::var_os("MIR2_I18N_VISUAL_ASSET_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("MIR2_GAME_I18N_VISUAL_OUTPUT").unwrap());
    assert!(output.is_absolute() && !output.exists());
    fs::create_dir_all(&output).unwrap();
    crate::native_i18n::activate(crate::native_i18n::Locale::SimplifiedChinese);
    let (mut app, target, _) = i18n_offscreen_app_configured(&assets, false, |app| {
        app.init_resource::<NativePlayerUiState>()
            .init_resource::<NativePlayerUiIntentQueue>()
            .init_resource::<InventoryModel>()
            .init_resource::<InventoryOperationFeedback>()
            .init_resource::<MailModel>()
            .init_resource::<MailUiState>()
            .init_resource::<MailComposeUi>()
            .init_resource::<mail_compose_drag::MailLetterWindow>()
            .init_resource::<mail_editor::MailLetterEditor>()
            .init_resource::<mail_parcel::MailParcelUi>()
            .init_resource::<BigMapModel>()
            .init_resource::<BigMapUiState>()
            .init_resource::<UiReadModel>()
            .init_resource::<ShopModel>()
            .init_resource::<ShopUiState>()
            .init_resource::<GameShopModel>()
            .init_resource::<StorageModel>()
            .init_resource::<StorageUiState>()
            .init_resource::<SkillModel>()
            .init_resource::<SkillBindingUi>()
            .insert_resource(SkillBindingPersistenceRuntime::with_config_path(
                output.join("unused-fixture-bindings.json"),
            ))
            .init_resource::<crate::social::SocialModel>()
            .insert_resource(NativeShellModel {
                screen: NativeShellScreen::InGame,
                ..Default::default()
            })
            .add_systems(Startup, spawn_overlay_root)
            .add_systems(Update, (sync_mail_letter_editor, render_overlays).chain());
    });
    let gate = open(&mut app);
    warm_i18n_images(&mut app);
    assert_eq!(
        app.world_mut()
            .query::<&NativeNpcShopWaitingPanel>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(gate.ui_authority_revision().is_none());
    capture_i18n(
        &mut app,
        &target,
        &output.join("npc-shop-waiting-zh-CN.png"),
    );

    let mut source = app.world().resource::<ShopModel>().clone();
    source.service_mode = NpcShopServiceMode::Buy;
    let key =
        npc_gold_buy_model_authority(&source, app.world().resource::<InventoryModel>()).unwrap();
    assert!(gate.observe_connection(NpcGoldBuyConnectionEpoch {
        run: 1,
        connection: 1
    }));
    assert!(gate.observe("offline-gpu-fixture", 1, &key, true));
    assert!(prepare_npc_shop_button_fixture(&mut app).is_some());
    warm_i18n_images(&mut app);
    assert_eq!(
        app.world_mut()
            .query::<&NativeNpcShopWaitingPanel>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&super::super::shop_paint::ShopPaintPanel>()
            .iter(app.world())
            .count(),
        1
    );
    capture_i18n(&mut app, &target, &output.join("npc-shop-ready-zh-CN.png"));
    assert!(app
        .world_mut()
        .resource_mut::<NativePlayerUiIntentQueue>()
        .drain_intents()
        .is_empty());
    fs::write(output.join("fixture.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "scope":"offline GPU rendering of production widgets with explicit local source fixtures",
        "locale":"zh-CN", "states":["no-source-visible-and-disabled","canonical-BUYSELL-source-ready"],
        "realSocket":false, "liveLogin":false, "purchaseSent":false,
        "files":["npc-shop-waiting-zh-CN.png","npc-shop-ready-zh-CN.png"]
    })).unwrap()).unwrap();
}
