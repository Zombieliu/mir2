//! Isolated shared Gateway routing evidence. Wallet balances, subscription
//! expiry and stale private checkpoints are prepared fixtures; payments here
//! enter through the trusted confirmation API, not a Stripe/webhook transport.
use mir2_gateway::{GatewayConfig, SharedInProcessZoneRuntimeFactory, ZoneId, ZoneRuntimeFactory};
use mir2_protocol::{ClientPacket, MirClass, MirGender, MirGridType, Point, ServerPacket};
use mir2_simulation::billing::{RechargeOffer, RechargeOrderState, VerifiedRechargePayment};
use mir2_simulation::monthly_card::{
    monthly_card_now_ms, MonthlyCardPolicy, MONTHLY_CARD_DURATION_MS,
};
use mir2_simulation::{
    AccountRecord, CharacterRecord, CharacterSaveRecord, WorldCommand, ZoneRuntimeHandle,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

const ACCOUNT: &str = "billing-gateway-owner";
const CHARACTER: i32 = 0;
const OTHER_CHARACTER: i32 = 1;
const PASSWORD: &str = "isolated-billing-gateway-password";
const KEY: &str = "billing-gateway-test-issuance-key-not-a-deployment-key";
const MAP: &str = "billing-gateway-fixture";

fn fixture() -> (
    GatewayConfig,
    SharedInProcessZoneRuntimeFactory,
    ZoneRuntimeHandle,
) {
    let mut config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_monthly_card_policy(MonthlyCardPolicy::new(true, Some(KEY)).unwrap());
    config.billing_monthly_card_credit_price = Some(10);
    config.monster_spawn_source = mir2_simulation::MonsterSpawnSource::StarterScenario;
    config.map.file_name = MAP.into();
    config.map.title = "Isolated billing Gateway fixture".into();
    config.spawn = Point { x: 10, y: 10 };
    config.map_collision.map_file_name = MAP.into();
    config.map_collision.map_width = 40;
    config.map_collision.map_height = 40;
    config.map_collision.play_bounds = mir2_game_data::MapBounds {
        min_x: 0,
        max_x: 39,
        min_y: 0,
        max_y: 39,
    };
    config.map_collision.region_bounds = config.map_collision.play_bounds.clone();
    config.map_collision.blocked_cells.clear();
    config.map_collision.doors.clear();
    config.visible_players.clear();
    config.visible_monsters.clear();
    config.visible_npcs.clear();
    config.safe_zones.clear();
    config.map_transfers.clear();
    {
        let mut store = config.account_store.lock().unwrap();
        store.accounts.clear();
        let mut account = AccountRecord::empty();
        account.password = PASSWORD.into();
        for (index, name) in [
            (CHARACTER, "BillingOwner"),
            (OTHER_CHARACTER, "OtherOwnedCharacter"),
        ] {
            let character = CharacterRecord {
                index,
                name: name.into(),
                level: 30,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            };
            let mut save = CharacterSaveRecord::new(character.clone());
            save.map_file_name = MAP.into();
            save.map_title = config.map.title.clone();
            save.position = config.spawn.clone();
            save.hp = 50;
            save.max_hp = 500;
            save.mp = 30;
            save.max_mp = 100;
            save.credit = 100;
            account.characters.push(character);
            account.saves.insert(index, save);
        }
        store.accounts.insert(ACCOUNT.into(), account);
    }
    // Issue in the past, initially redeem now, then prepare an expired receipt
    // after StartGame. The unit ledger remains internally valid in both states.
    let now = monthly_card_now_ms();
    let code = config
        .issue_monthly_card(
            ACCOUNT,
            "gateway-subscription-0001",
            now - MONTHLY_CARD_DURATION_MS - 86_400_000,
        )
        .unwrap();
    config
        .redeem_monthly_card(ACCOUNT, &code.code, now)
        .unwrap();
    // Keep autonomous regeneration out of the prepared stale-mirror assertions.
    let factory = SharedInProcessZoneRuntimeFactory::with_tick_cadences(
        Duration::from_secs(60),
        BTreeMap::new(),
    );
    let mut runtime = factory.create_runtime(config.clone(), &ZoneId::new("billing-gateway-zone"));
    let login = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: PASSWORD.into(),
        }))
        .unwrap();
    assert!(
        login
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
        "{login:?}"
    );
    let start = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: CHARACTER,
        }))
        .unwrap();
    assert!(
        start
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
        "{start:?}"
    );
    assert_eq!(runtime.active_identity().unwrap().account_id, ACCOUNT);
    (config, factory, runtime)
}

fn expire_access(config: &GatewayConfig) {
    let mut store = config.account_store.lock().unwrap();
    let ledger = store
        .accounts
        .get_mut(ACCOUNT)
        .unwrap()
        .monthly_card
        .as_mut()
        .unwrap();
    assert!(ledger.item_receipts.is_empty());
    let receipt = ledger.codes.values_mut().next().unwrap();
    let used_at = receipt.issued_at_ms;
    receipt.redeemed_at_ms = Some(used_at);
    receipt.credited_until_ms = Some(used_at + MONTHLY_CARD_DURATION_MS);
    ledger.expires_at_ms = used_at + MONTHLY_CARD_DURATION_MS;
    ledger.validate().unwrap();
}

fn buy(runtime: &mut ZoneRuntimeHandle, request: &str) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::BillingBuyMonthlyCard {
            character_index: CHARACTER,
            request_id: request.into(),
        })
        .unwrap()
}

fn activate(runtime: &mut ZoneRuntimeHandle, uid: u64) -> Vec<ServerPacket> {
    runtime
        .execute(WorldCommand::BillingActivateMonthlyCard {
            character_index: CHARACTER,
            unique_id: uid,
        })
        .unwrap()
}

fn financial_state(config: &GatewayConfig) -> Value {
    let store = config.account_store.lock().unwrap();
    let account = &store.accounts[ACCOUNT];
    let save = &account.saves[&CHARACTER];
    json!({"card": account.monthly_card, "billing": account.billing,
        "credit": save.credit, "inventory": save.inventory_items_json, "belt": save.belt_items_json,
        "revision": save.revision})
}

fn card_uid(config: &GatewayConfig, request: &str) -> u64 {
    let store = config.account_store.lock().unwrap();
    store.accounts[ACCOUNT]
        .monthly_card
        .as_ref()
        .unwrap()
        .item_receipts
        .values()
        .find(|row| row.request_id == request)
        .unwrap()
        .unique_id
}

fn assert_use_ack(packets: &[ServerPacket], uid: u64) {
    assert!(
        packets.iter().any(|packet| matches!(packet,
        ServerPacket::UseItem { unique_id, grid: MirGridType::Inventory, success: true }
            if *unique_id == uid)),
        "{packets:?}"
    );
}

#[test]
fn shared_gateway_expired_owner_can_refresh_buy_activate_and_replay() {
    let (config, _factory, mut runtime) = fixture();
    expire_access(&config);
    assert!(
        !config
            .refresh_monthly_card_status(ACCOUNT, monthly_card_now_ms())
            .unwrap()
            .can_enter_game
    );
    let identity = runtime.active_identity().unwrap();
    let refreshed = runtime
        .execute(WorldCommand::BillingRefresh {
            character_index: CHARACTER,
        })
        .unwrap();
    assert_eq!(
        runtime.active_identity().unwrap(),
        identity,
        "{refreshed:?}"
    );
    assert!(!refreshed
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LogOutSuccess { .. })));

    let purchased = buy(&mut runtime, "gateway-expired-buy-0001");
    let uid = card_uid(&config, "gateway-expired-buy-0001");
    assert!(purchased
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseCredit { credit: 10 })));
    assert!(purchased.iter().any(
        |packet| matches!(packet, ServerPacket::GainedItem { item } if item.unique_id == uid)
    ));
    assert_eq!(runtime.active_identity().unwrap(), identity);
    assert!(
        !config
            .refresh_monthly_card_status(ACCOUNT, monthly_card_now_ms())
            .unwrap()
            .can_enter_game
    );
    let before = financial_state(&config);
    let replay = buy(&mut runtime, "gateway-expired-buy-0001");
    assert!(!replay.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseCredit { .. } | ServerPacket::GainedItem { .. }
    )));
    assert_eq!(financial_state(&config), before);

    let activation_before_ms = monthly_card_now_ms();
    assert_use_ack(&activate(&mut runtime, uid), uid);
    let activation_after_ms = monthly_card_now_ms();
    let status = config
        .refresh_monthly_card_status(ACCOUNT, monthly_card_now_ms())
        .unwrap();
    assert!(status.can_enter_game);
    assert!((activation_before_ms + MONTHLY_CARD_DURATION_MS
        ..=activation_after_ms + MONTHLY_CARD_DURATION_MS)
        .contains(&status.expires_at_ms.unwrap()));
    let before = financial_state(&config);
    assert_use_ack(&activate(&mut runtime, uid), uid);
    assert_eq!(financial_state(&config), before);
    assert_eq!(runtime.active_identity().unwrap(), identity);
    assert!(config
        .list_billing_monthly_cards(ACCOUNT, CHARACTER)
        .unwrap()
        .is_empty());
}

#[test]
fn shared_gateway_old_consumed_uid_cannot_consume_the_next_card() {
    let (config, _factory, mut runtime) = fixture();
    buy(&mut runtime, "gateway-card-first-0001");
    let first = card_uid(&config, "gateway-card-first-0001");
    assert_use_ack(&activate(&mut runtime, first), first);
    let first_expiry = config
        .refresh_monthly_card_status(ACCOUNT, monthly_card_now_ms())
        .unwrap()
        .expires_at_ms
        .unwrap();
    buy(&mut runtime, "gateway-card-second-0002");
    let second = card_uid(&config, "gateway-card-second-0002");
    assert_ne!(first, second);
    let before = financial_state(&config);
    let replay = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: first,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    assert_use_ack(&replay, first);
    assert_eq!(financial_state(&config), before);
    assert_eq!(
        config
            .list_billing_monthly_cards(ACCOUNT, CHARACTER)
            .unwrap()[0]
            .unique_id,
        second
    );
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::UseItem {
            unique_id: second,
            grid: MirGridType::Inventory,
        }))
        .unwrap();
    assert_use_ack(&packets, second);
    assert_eq!(
        config
            .refresh_monthly_card_status(ACCOUNT, monthly_card_now_ms())
            .unwrap()
            .expires_at_ms,
        Some(first_expiry + MONTHLY_CARD_DURATION_MS)
    );
}

fn make_private_pools_stale(runtime: &mut ZoneRuntimeHandle) -> (i32, i32) {
    let current = runtime.world_snapshot();
    let expected = (current.player_hp.unwrap(), current.player_mp.unwrap());
    assert!(
        expected.0 > 1 && expected.1 > 1,
        "fixture needs positive distinct current pools"
    );
    // This public trusted checkpoint API affects the personal Source mirror.
    // The existing shared admission and pools remain authoritative.
    let mut stale = runtime.active_character_checkpoint().unwrap();
    stale.hp = 1;
    stale.mp = 1;
    runtime.restore_active_character_checkpoint(&stale).unwrap();
    let private = runtime.active_character_checkpoint().unwrap();
    assert_eq!((private.hp, private.mp), (1, 1));
    let shared = runtime.world_snapshot();
    assert_eq!(
        (shared.player_hp.unwrap(), shared.player_mp.unwrap()),
        expected
    );
    expected
}

fn assert_saved_and_current_pools(
    config: &GatewayConfig,
    runtime: &ZoneRuntimeHandle,
    expected: (i32, i32),
) {
    let store = config.account_store.lock().unwrap();
    let save = &store.accounts[ACCOUNT].saves[&CHARACTER];
    assert_eq!(
        (save.hp, save.mp),
        expected,
        "billing must not save stale personal pools"
    );
    drop(store);
    let shared = runtime.world_snapshot();
    assert_eq!(
        (shared.player_hp.unwrap(), shared.player_mp.unwrap()),
        expected
    );
}

#[test]
fn shared_gateway_refresh_purchase_and_activation_save_current_zone_pools() {
    let (config, _factory, mut runtime) = fixture();
    let now = monthly_card_now_ms();
    let offer = RechargeOffer {
        id: "billing-fixture-20".into(),
        label: "Prepared Credits".into(),
        currency: "usd".into(),
        amount_minor: 100,
        credits: 20,
    };
    let order = config
        .prepare_recharge_order(
            ACCOUNT,
            CHARACTER,
            "gateway-refresh-pools-0001",
            &offer,
            now,
            false,
        )
        .unwrap();
    let paid = config
        .confirm_recharge_payment(
            ACCOUNT,
            &order.id,
            &VerifiedRechargePayment {
                session_id: "cs_gateway_pool_fixture".into(),
                payment_intent_id: "pi_gateway_pool_fixture".into(),
                amount_minor: 100,
                currency: "usd".into(),
                livemode: false,
            },
            now,
        )
        .unwrap();
    assert_eq!(paid.state, RechargeOrderState::PaidPending);
    let expected = make_private_pools_stale(&mut runtime);
    let refreshed = runtime
        .execute(WorldCommand::BillingRefresh {
            character_index: CHARACTER,
        })
        .unwrap();
    assert!(
        refreshed
            .iter()
            .any(|packet| matches!(packet, ServerPacket::GainedCredit { credit: 20 })),
        "{refreshed:?}"
    );
    assert_saved_and_current_pools(&config, &runtime, expected);

    let expected = make_private_pools_stale(&mut runtime);
    buy(&mut runtime, "gateway-pool-buy-0001");
    assert_saved_and_current_pools(&config, &runtime, expected);
    let uid = card_uid(&config, "gateway-pool-buy-0001");
    let expected = make_private_pools_stale(&mut runtime);
    assert_use_ack(&activate(&mut runtime, uid), uid);
    assert_saved_and_current_pools(&config, &runtime, expected);
    assert_eq!(runtime.world_snapshot().credit, 110);
}

#[test]
fn shared_gateway_billing_rejects_another_owned_character_while_active() {
    let (config, _factory, mut runtime) = fixture();
    let before = financial_state(&config);
    for command in [
        WorldCommand::BillingRefresh {
            character_index: OTHER_CHARACTER,
        },
        WorldCommand::BillingBuyMonthlyCard {
            character_index: OTHER_CHARACTER,
            request_id: "gateway-wrong-actor-0001".into(),
        },
        WorldCommand::BillingActivateMonthlyCard {
            character_index: OTHER_CHARACTER,
            unique_id: 1,
        },
    ] {
        assert!(runtime.execute(command).is_err());
        assert_eq!(financial_state(&config), before);
        assert_eq!(
            runtime.active_identity().unwrap().character_index,
            CHARACTER
        );
    }
}
