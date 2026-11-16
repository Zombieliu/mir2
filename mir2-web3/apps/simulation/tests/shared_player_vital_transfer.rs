//! Shared player receipt clocks survive map/channel movement, never identity changes.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneOutbound, ZoneVitalSettlement,
};

const OLD: &str = "vital-transfer-old";
const NEW: &str = "vital-transfer-new";
const OWNER: u32 = 101;

fn session() -> SessionId {
    SessionId::new("owner")
}
fn join(map: &str, id: &str, object: u32, hp: i32, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(id),
        account_id: format!("{id}-account"),
        character_index: object as i32,
        object_id: object,
        name: id.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp,
        max_hp: 100,
        mp: 100,
        map_file_name: map.into(),
        position: Point { x, y: 20 },
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}
fn owner_receipts(out: &[ZoneOutbound]) -> Vec<ZoneVitalSettlement> {
    out.iter()
        .filter_map(|event| match event {
            ZoneOutbound::PlayerDamaged {
                session_id,
                settlement,
                ..
            }
            | ZoneOutbound::PlayerHealed {
                session_id,
                settlement,
                ..
            } if *session_id == session() => Some(settlement.expect("real Zone producer receipt")),
            _ => None,
        })
        .collect()
}
fn recipient_packets<'a>(out: &'a [ZoneOutbound], name: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(name);
    out.iter()
        .flat_map(|event| match event {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if *session_id == recipient => packets.as_slice(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&recipient) => packets.as_slice(),
            ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn fixture() -> ZoneManager {
    let mut manager = ZoneManager::new();
    manager.join(join(OLD, "owner", OWNER, 40, 10));
    manager.join(join(OLD, "old-observer", 102, 100, 14));
    manager.join(join(NEW, "new-observer", 103, 100, 14));
    manager.tick_all(1);
    let receipts = owner_receipts(&manager.tick_all(4001));
    assert_eq!(receipts.len(), 1);
    assert_eq!((receipts[0].hp_before, receipts[0].hp_after), (40, 44));
    assert_eq!(
        (receipts[0].life_generation, receipts[0].receipt_sequence),
        (0, 1)
    );
    manager
}

#[test]
fn every_manager_join_entry_preserves_clock_and_emits_one_aoi_leave() {
    for entry in 0..3 {
        let mut manager = fixture();
        let next = join(NEW, "owner", OWNER, 44, 10);
        let out = match entry {
            0 => manager.join(next),
            1 => manager.handle(ZoneCommand::Join(next)),
            _ => manager.handle_for_key(ZoneKey::for_map(NEW), ZoneCommand::Join(next)),
        };
        assert_eq!(recipient_packets(&out, "old-observer").iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectRemove { object_id } if *object_id == OWNER)).count(), 1);
        assert_eq!(recipient_packets(&out, "new-observer").iter()
            .filter(|packet| matches!(packet, ServerPacket::ObjectPlayer { info } if info.object_id == OWNER)).count(), 1);
        assert_eq!(
            manager.zone(&ZoneKey::for_map(OLD)).unwrap().player_count(),
            1
        );
        assert_eq!(manager.player_life_generation(&session()), Some(0));
        manager.tick_all(4002);
        let bytes = manager.checkpoint_bytes().unwrap();
        let mut restored = ZoneManager::restore_checkpoint(&bytes).unwrap();
        let pulse = manager.tick_all(8002);
        assert_eq!(pulse, restored.tick_all(8002));
        let receipts = owner_receipts(&pulse);
        assert_eq!(receipts.len(), 1);
        assert_eq!((receipts[0].hp_before, receipts[0].hp_after), (44, 48));
        assert_eq!(
            (receipts[0].life_generation, receipts[0].receipt_sequence),
            (0, 2)
        );
        assert_eq!(
            manager.checkpoint_bytes().unwrap(),
            restored.checkpoint_bytes().unwrap()
        );
    }
}

#[test]
fn dead_to_alive_map_join_advances_life_without_resetting_receipts() {
    let mut manager = fixture();
    let mut attacker = join(OLD, "attacker", 104, 100, 11);
    attacker.chat_profile.attack_mode = 5;
    attacker.combat_stats.min_dc = 500;
    attacker.combat_stats.max_dc = 500;
    attacker.combat_stats.accuracy = 100;
    manager.join(attacker);
    manager.handle(ZoneCommand::sync_player_combat_state(
        SessionId::new("attacker"),
        MirClass::Warrior,
        true,
        false,
        true,
        false,
        false,
        false,
    ));
    let death = manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: SessionId::new("attacker"),
        object_id: OWNER,
        direction: MirDirection::Left,
        spell: 0,
        level: 0,
        attack_type: 0,
        damage: 1,
        now_ms: 4002,
    });
    let receipts = owner_receipts(&death);
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].death_transition);
    assert_eq!(
        (receipts[0].life_generation, receipts[0].receipt_sequence),
        (0, 2)
    );
    manager.join(join(NEW, "owner", OWNER, 50, 10));
    assert_eq!(manager.player_life_generation(&session()), Some(1));
    manager.tick_all(4003);
    let mut restored =
        ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
    let pulse = manager.tick_all(8003);
    assert_eq!(pulse, restored.tick_all(8003));
    let receipts = owner_receipts(&pulse);
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        (receipts[0].life_generation, receipts[0].receipt_sequence),
        (1, 3)
    );
    assert!(!receipts[0].death_transition);
}

#[test]
fn same_zone_rejoin_preserves_clock_and_only_revives_once() {
    let mut manager = fixture();
    manager.handle(ZoneCommand::SyncPlayerVitals {
        session_id: session(),
        hp: 0,
        max_hp: 100,
        mp: 100,
    });
    manager.handle_for_key(
        ZoneKey::for_map(OLD),
        ZoneCommand::Join(join(OLD, "owner", OWNER, 50, 10)),
    );
    assert_eq!(manager.player_life_generation(&session()), Some(1));
    manager.join(join(OLD, "owner", OWNER, 50, 10));
    assert_eq!(manager.player_life_generation(&session()), Some(1));
    manager.tick_all(4002);
    let receipts = owner_receipts(&manager.tick_all(8002));
    assert_eq!(receipts.len(), 1);
    assert_eq!(
        (receipts[0].life_generation, receipts[0].receipt_sequence),
        (1, 2)
    );
}

#[test]
fn transfer_does_not_copy_another_characters_or_incarnations_clock() {
    for changed in 0..3 {
        let mut manager = fixture();
        manager.handle(ZoneCommand::SyncPlayerVitals {
            session_id: session(),
            hp: 0,
            max_hp: 100,
            mp: 100,
        });
        let mut next = join(NEW, "owner", OWNER, 40, 10);
        match changed {
            0 => next.account_id = "different-account".into(),
            1 => next.character_index += 1,
            _ => next.object_id = 105,
        }
        manager.join(next);
        assert_eq!(manager.player_life_generation(&session()), Some(0));
        manager.tick_all(4002);
        let receipts = owner_receipts(&manager.tick_all(8002));
        assert_eq!(receipts.len(), 1);
        assert_eq!(
            (receipts[0].life_generation, receipts[0].receipt_sequence),
            (0, 1)
        );
    }
}
