//! Authoritative native damage settles one player life and keeps Death private.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZoneRuntime,
};
const MAP: &str = "shared-player-death-fixture";
const PLAYER: u32 = 101;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(session: &str, id: u32, x: i32, hp: i32, gm: bool) -> ZoneJoin {
    let mut result = ZoneJoin {
        session_id: SessionId::new(session),
        account_id: session.into(),
        character_index: id as i32,
        object_id: id,
        name: session.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp,
        max_hp: hp,
        mp: 100,
        map_file_name: MAP.into(),
        position: point(x, 20),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    };
    result.combat_stats.gm_never_die = gm;
    result
}
fn fixture(ai: u8, id: u32, hp: i32, gm: bool) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    zone.handle(ZoneCommand::Join(join("owner", PLAYER, 21, hp, gm)));
    zone.handle(ZoneCommand::Join(join("observer", 102, 25, 10000, false)));
    // Explicit nonzero stat fixture: ArcherGuard has fixed source DC/SC=255.
    // AI0 exercises ordinary native melee; AI69 exercises the real Hugger FSM.
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("owner"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: id,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 139,
            ai,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 0,
            hp: 9999,
            max_hp: 9999,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(20, 20),
            direction: MirDirection::Right,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    zone
}
fn packets<'a>(out: &'a [ZoneOutbound], recipient: &str) -> Vec<&'a ServerPacket> {
    let recipient = SessionId::new(recipient);
    out.iter()
        .flat_map(|o| match o {
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
fn assert_death_delivery(out: &[ZoneOutbound]) {
    let owner = packets(out, "owner");
    let private = owner
        .iter()
        .position(|p| matches!(p, ServerPacket::Death { .. }))
        .expect("owner Death");
    let public = owner
        .iter()
        .position(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==PLAYER))
        .expect("owner ObjectDied");
    assert!(
        private < public,
        "Death must precede ObjectDied to activate owner's death screen"
    );
    assert_eq!(
        owner
            .iter()
            .filter(|p| matches!(p, ServerPacket::Death { .. }))
            .count(),
        1
    );
    let observer = packets(out, "observer");
    assert!(
        !observer
            .iter()
            .any(|p| matches!(p, ServerPacket::Death { .. })),
        "Death is owner-only"
    );
    assert!(observer
        .iter()
        .any(|p| matches!(p,ServerPacket::ObjectDied{info} if info.object_id==PLAYER)));
}
fn owner_damaged(out: &[ZoneOutbound]) -> bool {
    out.iter().any(
        |o| matches!(o,ZoneOutbound::PlayerDamaged{session_id,..} if session_id.as_str()=="owner"),
    )
}

#[test]
fn native_melee_kills_once_and_checkpoint_preserves_exact_outbound_order() {
    let mut zone = fixture(0, 9000, 50, false);
    zone.tick(3000);
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let out = zone.tick(3600);
    let replay = restored.tick(3600);
    assert_eq!(
        out, replay,
        "legal checkpoint replay preserves every authoritative outbound"
    );
    assert_eq!(
        zone.canonical_state_root().unwrap(),
        restored.canonical_state_root().unwrap()
    );
    assert_eq!(zone.player_vitals(&SessionId::new("owner")).unwrap().0, 0);
    assert_death_delivery(&out);
    let receipt = out
        .iter()
        .find_map(|event| match event {
            ZoneOutbound::PlayerDamaged {
                session_id,
                damage,
                settlement,
            } if session_id.as_str() == "owner" => {
                assert_eq!(*damage, 50);
                Some(settlement.expect("native damage must carry its mutation-time receipt"))
            }
            _ => None,
        })
        .unwrap();
    assert_eq!((receipt.hp_before, receipt.hp_after), (50, 0));
    assert_eq!(
        (
            receipt.object_id,
            receipt.life_generation,
            receipt.receipt_sequence
        ),
        (PLAYER, 0, 1)
    );
    assert!(receipt.death_transition);
    for now in [3600, 3601, 5000, 10000] {
        let repeated = zone.tick(now);
        assert!(!owner_damaged(&repeated));
        assert!(!packets(&repeated, "owner").iter().any(|p| matches!(
            p,
            ServerPacket::Death { .. }
                | ServerPacket::ObjectDied {
                    info: mir2_protocol::ObjectDiedInfo {
                        object_id: PLAYER,
                        ..
                    }
                }
        )));
    }
}

#[test]
fn gm_never_die_leaves_authoritative_hp_unchanged() {
    let mut zone = fixture(0, 9000, 50, true);
    for now in [3000, 3600, 5001, 5601, 10000] {
        let out = zone.tick(now);
        assert_eq!(zone.player_vitals(&SessionId::new("owner")).unwrap().0, 50);
        assert!(!owner_damaged(&out));
        assert!(!packets(&out, "owner")
            .iter()
            .any(|p| matches!(p, ServerPacket::Death { .. })));
    }
}

#[test]
fn hugger_periodic_poison_kills_and_cannot_cross_player_revival() {
    let mut selected = None;
    for id in 9100..9164 {
        let mut zone = fixture(69, id, 400, false);
        zone.tick(2001);
        let blast = zone.tick(2501);
        if packets(&blast, "owner").iter().any(
            |p| matches!(p,ServerPacket::ObjectPoisoned{object_id:PLAYER,poison} if poison&1!=0),
        ) {
            assert_eq!(zone.player_vitals(&SessionId::new("owner")).unwrap().0, 145);
            selected = Some(zone);
            break;
        }
    }
    let mut zone = selected.expect("real Hugger poison branch");
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let death = zone.tick(2502);
    assert_eq!(death, restored.tick(2502));
    assert_death_delivery(&death);
    assert_eq!(zone.player_vitals(&SessionId::new("owner")).unwrap().0, 0);
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: SessionId::new("owner"),
        hp: 400,
        max_hp: 400,
        mp: 100,
    });
    let mut zone = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    for now in [2503, 4503, 6504, 8505, 10506] {
        let out = zone.tick(now);
        assert_eq!(zone.player_vitals(&SessionId::new("owner")).unwrap().0, 400);
        assert!(
            !owner_damaged(&out),
            "retired poison cannot damage the revived life"
        );
        assert!(!packets(&out, "owner")
            .iter()
            .any(|p| matches!(p, ServerPacket::Death { .. })));
    }
}

#[test]
fn regeneration_receipts_survive_checkpoint_and_stamp_the_current_life() {
    let mut zone = ZoneRuntime::new(ZoneKey::for_map(MAP));
    let mut player = join("owner", PLAYER, 21, 40, false);
    player.max_hp = 100;
    zone.handle(ZoneCommand::Join(player));
    let pristine = zone.checkpoint_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&pristine).unwrap();
    // New default counters are omitted, preserving legacy canonical bytes.
    assert!(value["players"]["owner"]
        .get("vital_receipt_sequence")
        .is_none());
    let restored = ZoneRuntime::restore_checkpoint(&pristine).unwrap();
    assert_eq!(
        zone.canonical_state_root().unwrap(),
        restored.canonical_state_root().unwrap()
    );
    zone.tick(1);
    let pulse = zone.tick(4001);
    let first = pulse
        .iter()
        .find_map(|event| match event {
            ZoneOutbound::PlayerHealed {
                settlement,
                amount: 4,
                ..
            } => *settlement,
            _ => None,
        })
        .expect("normal HP regeneration is also a native settlement");
    assert_eq!(
        (
            first.hp_before,
            first.hp_after,
            first.life_generation,
            first.receipt_sequence
        ),
        (40, 44, 0, 1)
    );
    assert!(!first.death_transition);
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: SessionId::new("owner"),
        hp: 0,
        max_hp: 100,
        mp: 100,
    });
    zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: SessionId::new("owner"),
        hp: 50,
        max_hp: 100,
        mp: 100,
    });
    let mut restored = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    let pulse = zone.tick(8001);
    assert_eq!(pulse, restored.tick(8001));
    let next = pulse
        .iter()
        .find_map(|event| match event {
            ZoneOutbound::PlayerHealed { settlement, .. } => *settlement,
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (
            next.hp_before,
            next.hp_after,
            next.life_generation,
            next.receipt_sequence
        ),
        (50, 54, 1, 2)
    );
}
