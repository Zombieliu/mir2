//! Prepared consumer/fault tests. The issuing Zone's genuine births are tested
//! separately by shared_pet_pvp; these never claim transport/play acceptance.
use super::*;
use super::resources::{InventoryResource, RuntimeConfigResource};
use crate::{AccountStoreTransactionFault, EquipmentSlot, SimulationConfig, ZoneKey, ZoneOwnedPetPlayerKillReceipt};
use mir2_protocol::{ClientPacket, ServerPacket};

fn fixture() -> (SimulationSession, SimulationConfig, ZoneOwnedPetPlayerKillReceipt) {
    let mut session = SimulationSession::new(SimulationConfig::default());
    session.enable_shared_guild_authority();
    session.try_handle_packet(ClientPacket::Login { account_id: "demo".into(), password: "demo".into() }).unwrap();
    session.try_handle_packet(ClientPacket::StartGame { character_index: 0 }).unwrap();
    session.app.world_mut().resource_mut::<InventoryResource>().equipment_items =
        super::equipment::seed_equipment_items();
    session.save_active_character().unwrap();
    let config = session.app.world().resource::<RuntimeConfigResource>().config.clone();
    let active = session.active_identity().unwrap();
    let receipt = ZoneOwnedPetPlayerKillReceipt {
        direct_player: false,
        zone_key: ZoneKey::for_map("0"), sequence: 1, at_ms: 900,
        owner_session_id: SessionId::new("prepared-owner"), owner_account_id: active.account_id,
        owner_character_index: active.character_index, owner_object_id: 50000,
        owner_life_generation: 1, owner_online_identity: "prepared-born-epoch-1".into(),
        pet_object_id: 80000, pet_incarnation: 1,
        victim_session_id: SessionId::new("prepared-victim"), victim_object_id: 50001,
        victim_life_generation: 1, victim_name: "Victim".into(),
        protected_by_law: false, unlawful: true, curse_roll: 0,
    };
    (session, config, receipt)
}
fn luck(session: &SimulationSession) -> i32 {
    session.app.world().resource::<InventoryResource>().equipment_items.iter()
        .find(|item| item.slot == EquipmentSlot::Weapon).unwrap().added_luck
}

#[test]
fn pk_consumer_exact_once_checkpoint_and_relogin() {
    let (mut session, config, receipt) = fixture();
    let before = session.active_character_checkpoint().unwrap();
    let original_luck = luck(&session);
    let packets = session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.pk_points, before.pk_points + 100);
    assert_eq!(luck(&session), original_luck - 1);
    assert!(packets.iter().any(|p| matches!(p, ServerPacket::RefreshItem { .. })));
    assert_eq!(after.player_kill_receipts.len(), 1);
    assert!(session.apply_shared_owned_pet_player_kill(&receipt).unwrap().is_empty());
    assert_eq!(session.active_character_checkpoint().unwrap().revision, after.revision);
    session.handle_packet(ClientPacket::LogOut);
    let mut fresh = SimulationSession::new(config);
    fresh.enable_shared_guild_authority();
    fresh.handle_packet(ClientPacket::Login { account_id: receipt.owner_account_id.clone(), password: "demo".into() });
    fresh.handle_packet(ClientPacket::StartGame { character_index: receipt.owner_character_index });
    assert!(fresh.apply_shared_owned_pet_player_kill(&receipt).unwrap().is_empty());
    assert_eq!(fresh.active_character_checkpoint().unwrap().pk_points, after.pk_points);
    assert_eq!(luck(&fresh), original_luck - 1);
}

#[test]
fn pk_consumer_persist_failure_keeps_full_source_and_retries_once() {
    let (mut session, config, receipt) = fixture();
    session.force_authoritative_player_life(true);
    let before = session.active_character_checkpoint().unwrap();
    config.inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforePersist);
    assert!(session.apply_shared_owned_pet_player_kill(&receipt).is_err());
    let after = session.active_character_checkpoint().unwrap();
    assert_eq!(after.pk_points, before.pk_points);
    assert_eq!(after.player_kill_receipts, before.player_kill_receipts);
    assert_eq!(after.equipment_items_json, before.equipment_items_json);
    assert_eq!(after.revision, before.revision);
    assert!(super::components::current_player_is_dead(session.app.world()));
    let packets = session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
    assert_eq!(session.active_character_checkpoint().unwrap().pk_points, before.pk_points + 100);
    assert!(!packets.iter().any(|p| matches!(p, ServerPacket::ObjectRevived { .. })));
    assert!(super::components::current_player_is_dead(session.app.world()));
}

#[test]
fn pk_consumer_war_red_brown_protection_and_curse_floor() {
    for protected in [false, true] {
        let (mut session, _, mut receipt) = fixture();
        receipt.protected_by_law = protected;
        receipt.unlawful = false;
        let before = session.active_character_checkpoint().unwrap();
        let old_luck = luck(&session);
        session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
        assert_eq!(session.active_character_checkpoint().unwrap().pk_points, before.pk_points);
        assert_eq!(luck(&session), old_luck);
        assert_eq!(session.active_character_checkpoint().unwrap().player_kill_receipts.len(), 1);
    }
    let (mut session, _, receipt) = fixture();
    session.app.world_mut().resource_mut::<InventoryResource>().equipment_items.iter_mut()
        .find(|item| item.slot == EquipmentSlot::Weapon).unwrap().added_luck = -10;
    let packets = session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
    assert_eq!(luck(&session), -10);
    assert!(!packets.iter().any(|p| matches!(p, ServerPacket::RefreshItem { .. })));
}

#[test]
fn pk_consumer_no_curse_roll_no_weapon_and_owner_payload_validation() {
    let (mut session, _, mut receipt) = fixture();
    receipt.curse_roll = 3;
    let old_luck = luck(&session);
    session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
    assert_eq!(luck(&session), old_luck);
    let mut changed = receipt.clone(); changed.at_ms += 1;
    assert!(session.apply_shared_owned_pet_player_kill(&changed).is_err());
    changed = receipt.clone(); changed.sequence += 1; changed.owner_account_id = "another".into();
    assert!(session.apply_shared_owned_pet_player_kill(&changed).is_err());
    changed = receipt.clone(); changed.sequence += 1; changed.curse_roll = 4;
    assert!(session.apply_shared_owned_pet_player_kill(&changed).is_err());
    session.app.world_mut().resource_mut::<InventoryResource>().equipment_items.clear();
    receipt.sequence += 1; receipt.curse_roll = 0;
    session.apply_shared_owned_pet_player_kill(&receipt).unwrap();
    assert_eq!(session.active_character_checkpoint().unwrap().player_kill_receipts.len(), 2);
}
