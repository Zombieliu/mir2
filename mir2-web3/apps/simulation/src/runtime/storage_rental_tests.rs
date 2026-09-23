use crate::config::AccountStoreTransactionFault;
use crate::runtime::crystal_compat::EXPANDED_STORAGE_SLOTS;
use crate::runtime::resources::PlayerRuntimeResource;
use crate::runtime::session::SimulationSession;
use crate::SimulationConfig;
use mir2_game_data::{localized_text_or_fallback, LanguageCode};
use mir2_protocol::{ChatType, ClientPacket, ServerPacket};

const STORAGE_RENTAL_PRICE_GOLD: u32 = 1_000_000;
const TICKS_PER_DAY: i64 = 24 * 60 * 60 * 10_000_000;

fn authenticated_rental_session(config: SimulationConfig, gold: u32) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    assert!(session
        .handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(session
        .handle_packet(ClientPacket::StartGame { character_index: 0 })
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    session
        .app
        .world_mut()
        .resource_mut::<PlayerRuntimeResource>()
        .gold = gold;
    session
}

fn add_storage(session: &mut SimulationSession) -> Vec<ServerPacket> {
    session.handle_packet(ClientPacket::Chat {
        message: "@ADDSTORAGE".to_string(),
        linked_items: Vec::new(),
    })
}

fn resize_expiry(packets: &[ServerPacket]) -> i64 {
    packets
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::ResizeStorage {
                size,
                has_expanded_storage,
                expiry_time_binary_datetime,
            } if *size == i32::from(EXPANDED_STORAGE_SLOTS) && *has_expanded_storage => {
                Some(*expiry_time_binary_datetime)
            }
            _ => None,
        })
        .expect("successful storage rental should resize storage")
}

fn serialized_store(config: &SimulationConfig) -> Vec<u8> {
    serde_json::to_vec(
        &*config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned"),
    )
    .expect("account store should serialize")
}

#[test]
fn addstorage_fresh_rental_debits_gold_and_persists_ten_day_capacity() {
    let config = SimulationConfig::default();
    let mut session = authenticated_rental_session(config.clone(), STORAGE_RENTAL_PRICE_GOLD);

    let before = super::inventory::current_binary_datetime();
    let packets = add_storage(&mut session);
    let after = super::inventory::current_binary_datetime();
    let expiry = resize_expiry(&packets);
    assert!(matches!(
        packets.as_slice(),
        [
            ServerPacket::LoseGold {
                gold: STORAGE_RENTAL_PRICE_GOLD
            },
            ServerPacket::ResizeStorage { .. },
        ]
    ));
    assert!(
        super::inventory::binary_datetime_ticks(expiry)
            >= super::inventory::binary_datetime_ticks(before) + 10 * TICKS_PER_DAY
            && super::inventory::binary_datetime_ticks(expiry)
                <= super::inventory::binary_datetime_ticks(after) + 10 * TICKS_PER_DAY,
        "fresh rental expiry should be exactly ten days from its commit time"
    );
    let snapshot = session.world_snapshot();
    assert_eq!(snapshot.gold, 0);
    assert_eq!(snapshot.storage_size, EXPANDED_STORAGE_SLOTS);
    assert!(snapshot.has_expanded_storage);
    assert_eq!(
        snapshot.expanded_storage_expiry_time_binary_datetime,
        expiry
    );

    let store = config
        .account_store
        .lock()
        .expect("account store mutex should not be poisoned");
    let account = store
        .accounts
        .get("demo")
        .expect("authenticated account should persist");
    assert_eq!(account.storage_size, EXPANDED_STORAGE_SLOTS);
    assert!(account.has_expanded_storage);
    assert_eq!(account.expanded_storage_expiry_time_binary_datetime, expiry);
    assert_eq!(account.saves.get(&0).expect("active save").gold, 0);
}

#[test]
fn addstorage_renewal_extends_future_expiry_by_ten_days_and_debits_again() {
    let config = SimulationConfig::default();
    let mut session =
        authenticated_rental_session(config.clone(), STORAGE_RENTAL_PRICE_GOLD.saturating_mul(2));

    let first_expiry = resize_expiry(&add_storage(&mut session));
    let second_expiry = resize_expiry(&add_storage(&mut session));
    assert_eq!(
        super::inventory::binary_datetime_ticks(second_expiry)
            - super::inventory::binary_datetime_ticks(first_expiry),
        10 * TICKS_PER_DAY,
        "an active rental extends from its existing expiry, not from now"
    );
    assert_eq!(session.world_snapshot().gold, 0);
    assert_eq!(
        config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned")
            .accounts
            .get("demo")
            .expect("account should persist")
            .saves
            .get(&0)
            .expect("active save")
            .gold,
        0
    );
}

#[test]
fn addstorage_low_gold_returns_low_gold_without_world_or_store_mutation() {
    let config = SimulationConfig::default();
    let mut session = authenticated_rental_session(config.clone(), STORAGE_RENTAL_PRICE_GOLD - 1);
    let before_world = session.world_snapshot();
    let before_store = serialized_store(&config);

    let packets = add_storage(&mut session);

    let low_gold =
        localized_text_or_fallback(LanguageCode::English, "server.LowGold", "server.LowGold");
    assert!(packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::Chat { message, chat_type: ChatType::System }
            if message == &low_gold
    )));
    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ResizeStorage { .. })));
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(serialized_store(&config), before_store);
}

#[test]
fn addstorage_requires_authenticated_active_identity_without_demo_fallback() {
    let config = SimulationConfig::default();
    let mut session = SimulationSession::new(config.clone());
    let before_world = session.world_snapshot();
    let before_store = serialized_store(&config);

    assert!(super::inventory::expand_storage_rental_impl(session.app.world_mut()).is_empty());
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(serialized_store(&config), before_store);
}

#[test]
fn addstorage_save_failure_rolls_back_gold_and_capacity_before_world_update() {
    let config = SimulationConfig::default();
    let mut session = authenticated_rental_session(config.clone(), STORAGE_RENTAL_PRICE_GOLD);
    let before_world = session.world_snapshot();
    let before_store = serialized_store(&config);

    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    let packets = add_storage(&mut session);

    assert!(!packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::ResizeStorage { .. })));
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(serialized_store(&config), before_store);
}

#[test]
fn addstorage_stale_active_revision_rejects_without_gold_or_capacity_mutation() {
    let config = SimulationConfig::default();
    let mut session = authenticated_rental_session(config.clone(), STORAGE_RENTAL_PRICE_GOLD);
    {
        let mut store = config
            .account_store
            .lock()
            .expect("account store mutex should not be poisoned");
        let save = store
            .accounts
            .get_mut("demo")
            .expect("fixture account")
            .saves
            .get_mut(&0)
            .expect("fixture active save");
        save.revision += 1;
    }
    let before_world = session.world_snapshot();
    let before_store = serialized_store(&config);

    let packets = add_storage(&mut session);

    assert!(!packets.iter().any(|packet| matches!(
        packet,
        ServerPacket::LoseGold { .. } | ServerPacket::ResizeStorage { .. }
    )));
    assert_eq!(session.world_snapshot(), before_world);
    assert_eq!(serialized_store(&config), before_store);
}
