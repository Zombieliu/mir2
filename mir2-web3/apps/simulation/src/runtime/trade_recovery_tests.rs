use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};

use crate::{AccountStoreTransactionFault, SimulationSession};

use super::*;

fn file_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mir2-file-trade-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    dir.join("accounts.json")
}

fn login(session: &mut SimulationSession, account_id: &str, password: &str) {
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: account_id.into(),
        password: password.into(),
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
}

fn enter(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
    index: i32,
) -> SimulationSession {
    let mut session = SimulationSession::new(config.clone());
    login(&mut session, account_id, password);
    session.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    assert_eq!(session.active_identity().unwrap().character_index, index);
    session
}

fn new_peer(config: &SimulationConfig) -> (SimulationSession, i32) {
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::NewAccount {
        account_id: "filetradeqa".into(),
        password: "filetradeqa".into(),
        birth_date_binary: 0,
        user_name: String::new(),
        secret_question: String::new(),
        secret_answer: String::new(),
        email_address: String::new(),
    });
    login(&mut session, "filetradeqa", "filetradeqa");
    let index = session
        .handle_packet(ClientPacket::NewCharacter {
            name: "FileTradePeer".into(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        })
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .unwrap();
    session.handle_packet(ClientPacket::StartGame {
        character_index: index,
    });
    assert_eq!(session.active_identity().unwrap().character_index, index);
    (session, index)
}

fn open_offer(session: &mut SimulationSession, partner: &str) {
    session.trade_request(partner);
    let packets = session.handle_packet(ClientPacket::TradeReply {
        accept_invite: true,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::TradeAccept { .. })));
}

fn offer_potions_and_gold(session: &mut SimulationSession) -> u64 {
    let before = session.world_snapshot();
    let item = before
        .inventory_items
        .iter()
        .find(|item| item.key == "red-potion")
        .unwrap();
    let packets = session.handle_packet(ClientPacket::DepositTradeItem {
        from: i32::from(item.slot),
        to: 3,
    });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::DepositTradeItem { success: true, .. })));
    let packets = session.handle_packet(ClientPacket::TradeGold { amount: 17 });
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoseGold { gold: 17 })));
    item.unique_id
}

fn recorded_match_count(config: &SimulationConfig) -> usize {
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .values()
        .map(|account| account.local_trade_matches.len())
        .sum()
}

#[test]
fn file_trade_match_survives_one_projection_and_the_other_restarts_exactly_once() {
    let file = file_path("matched");
    let config = SimulationConfig::default().with_account_store_path(&file);
    let mut sender = enter(&config, "demo", "demo", 0);
    let (mut receiver, receiver_index) = new_peer(&config);
    let sender_gold = sender.world_snapshot().gold;
    let receiver_gold = receiver.world_snapshot().gold;
    let sender_name = sender.active_identity().unwrap().character_name;
    open_offer(&mut sender, "FileTradePeer");
    open_offer(&mut receiver, &sender_name);
    offer_potions_and_gold(&mut sender);
    let sender_offer = sender.shared_trade_confirm().1.unwrap();
    let receiver_offer = receiver.shared_trade_confirm().1.unwrap();
    sender.save_local_trade_preparation().unwrap();

    assert!(sender
        .commit_local_trade_match(&sender_offer, &receiver_offer)
        .is_err());
    assert_eq!(recorded_match_count(&config), 0);
    receiver.save_local_trade_preparation().unwrap();
    sender
        .commit_local_trade_match(&sender_offer, &receiver_offer)
        .unwrap();
    assert_eq!(recorded_match_count(&config), 2);
    assert!(sender.recover_orphaned_shared_trade().is_empty());
    assert!(receiver.recover_orphaned_shared_trade().is_empty());

    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::Persist);
    assert!(sender
        .reconcile_local_trade_match()
        .iter()
        .all(|packet| !matches!(packet, ServerPacket::TradeConfirm)));
    assert_eq!(recorded_match_count(&config), 2);
    assert_eq!(sender.world_snapshot().gold, sender_gold - 17);
    assert!(
        sender
            .world_snapshot()
            .stage5_systems
            .trade
            .unwrap()
            .escrow_prepared
    );
    let packets = sender.reconcile_local_trade_match();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::TradeConfirm))
            .count(),
        1
    );
    assert!(sender.world_snapshot().stage5_systems.trade.is_none());
    assert_eq!(recorded_match_count(&config), 1);
    assert!(sender.reconcile_local_trade_match().is_empty());
    drop((sender, receiver, config));

    let restarted = SimulationConfig::default().with_account_store_path(&file);
    let mut receiver = enter(&restarted, "filetradeqa", "filetradeqa", receiver_index);
    assert_eq!(recorded_match_count(&restarted), 1);
    assert!(
        receiver
            .world_snapshot()
            .stage5_systems
            .trade
            .unwrap()
            .escrow_prepared
    );
    assert!(receiver.recover_orphaned_shared_trade().is_empty());
    let packets = receiver.reconcile_local_trade_match();
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::TradeConfirm))
            .count(),
        1
    );
    assert_eq!(
        packets
            .iter()
            .filter(|packet| matches!(packet, ServerPacket::GainedGold { gold: 17 }))
            .count(),
        1
    );
    let after = receiver.world_snapshot();
    assert_eq!(after.gold, receiver_gold + 17);
    assert_eq!(
        after
            .inventory_items
            .iter()
            .filter(|item| item.key == "red-potion" && item.quantity == 5)
            .count(),
        1
    );
    assert!(after.stage5_systems.trade.is_none());
    assert_eq!(recorded_match_count(&restarted), 0);
    assert!(receiver.reconcile_local_trade_match().is_empty());
    drop((receiver, restarted));

    let restarted_again = SimulationConfig::default().with_account_store_path(&file);
    let sender = enter(&restarted_again, "demo", "demo", 0);
    let receiver = enter(
        &restarted_again,
        "filetradeqa",
        "filetradeqa",
        receiver_index,
    );
    assert_eq!(sender.world_snapshot().gold, sender_gold - 17);
    assert_eq!(receiver.world_snapshot().gold, receiver_gold + 17);
    assert_eq!(
        receiver.world_snapshot().inventory_items,
        after.inventory_items
    );
    assert!(sender.world_snapshot().stage5_systems.trade.is_none());
    assert!(receiver.world_snapshot().stage5_systems.trade.is_none());
}

#[test]
fn file_trade_restart_returns_only_new_unmatched_preparations() {
    for marked in [true, false] {
        let file = file_path(if marked { "orphan" } else { "legacy" });
        let config = SimulationConfig::default().with_account_store_path(&file);
        let mut sender = enter(&config, "demo", "demo", 0);
        let wallet = sender.world_snapshot().gold;
        open_offer(&mut sender, "FileTradePeer");
        let uid = offer_potions_and_gold(&mut sender);
        assert!(sender.shared_trade_confirm().1.is_some());
        if marked {
            sender.save_local_trade_preparation().unwrap();
        } else {
            sender.save_active_character().unwrap();
        }
        drop((sender, config));

        let restarted = SimulationConfig::default().with_account_store_path(&file);
        let mut sender = enter(&restarted, "demo", "demo", 0);
        let packets = sender.recover_orphaned_shared_trade();
        assert_eq!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::TradeCancel { unlock: false })),
            marked
        );
        assert_eq!(
            sender.world_snapshot().gold,
            if marked { wallet } else { wallet - 17 }
        );
        assert_eq!(
            sender
                .world_snapshot()
                .inventory_items
                .iter()
                .any(|item| item.key == "red-potion" && item.unique_id == uid),
            marked
        );
        assert!(sender.recover_orphaned_shared_trade().is_empty());
    }
}
