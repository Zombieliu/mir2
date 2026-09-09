//! Public shared-zone TownArcher AI57 contracts, not conquest AI80.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket};
use mir2_simulation::{
    SessionId, WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneMonsterSpawn,
    ZoneOutbound, ZonePlayerCombatStats, ZoneRuntime,
};
const ID: u32 = 9157;
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn fixture(pk: i32) -> ZoneRuntime {
    let mut z = ZoneRuntime::new(ZoneKey::for_map("town-archer-fixture"));
    z.handle(ZoneCommand::Join(ZoneJoin {
        session_id: SessionId::new("player"),
        account_id: "test".into(),
        character_index: 1,
        object_id: 101,
        name: "red".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 10000,
        max_hp: 10000,
        mp: 100,
        map_file_name: "town-archer-fixture".into(),
        position: point(25, 20),
        direction: MirDirection::Left,
        chat_profile: Default::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    let mut profile = z.player_chat_profile(&SessionId::new("player")).unwrap();
    profile.pk_points = pk;
    z.handle(ZoneCommand::UpdateChatProfile {
        session_id: SessionId::new("player"),
        profile,
    });
    z.handle(ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            object_id: ID,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 139,
            ai: 57,
            disposition: Some(WorldEntityDisposition::Friendly),
            level: 0,
            max_hp: 9999,
            hp: 9999,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(20, 20),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    z
}
fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|o| match o {
            ZoneOutbound::ToSession { packets, .. }
            | ZoneOutbound::ToMany { packets, .. }
            | ZoneOutbound::ToAll { packets } => packets.as_slice(),
            _ => &[],
        })
        .collect()
}
fn shots(out: &[ZoneOutbound]) -> usize {
    packets(out)
        .iter()
        .filter(|p| matches!(p,ServerPacket::ObjectRangeAttack{info} if info.object_id==ID))
        .count()
}
fn hp(z: &ZoneRuntime) -> i32 {
    z.player_vitals(&SessionId::new("player")).unwrap().0
}

#[test]
fn town_archer_missing_disposition_cannot_acquire_or_continue_shooting() {
    let mut z = fixture(200);
    let revoke = ZoneCommand::SpawnMonster {
        session_id: SessionId::new("player"),
        now_ms: 1,
        monster: ZoneMonsterSpawn {
            object_id: ID,
            name: "ArcherGuard".into(),
            name_colour_argb: -1,
            image: 139,
            ai: 57,
            disposition: None,
            level: 0,
            max_hp: 9999,
            hp: 9999,
            experience: 0,
            move_speed_ms: 300,
            attack_speed_ms: 2000,
            friendly_guild: None,
            position: point(20, 20),
            direction: MirDirection::Down,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    };
    z.handle(revoke.clone());
    assert_eq!(shots(&z.tick(2001)), 0);
    assert_eq!(shots(&z.tick(2002)), 0);
    assert_eq!(shots(&z.tick(5000)), 0);
    assert_eq!(hp(&z), 10000);
    let mut active = fixture(200);
    active.tick(2001);
    assert_eq!(shots(&active.tick(2002)), 1);
    active.handle(revoke);
    active.tick(2752);
    assert_eq!(
        hp(&active),
        10000,
        "revoked authority also cancels queued arrows"
    );
}

#[test]
fn town_archer_pk_threshold_fear_window_projectile_damage_and_checkpoint() {
    let mut peaceful = fixture(199);
    peaceful.tick(2001);
    assert_eq!(shots(&peaceful.tick(2002)), 0);
    peaceful.tick(10000);
    assert_eq!(hp(&peaceful), 10000);
    let mut z = fixture(200);
    assert_eq!(
        shots(&z.tick(2000)),
        0,
        "spawn action lock includes exact boundary"
    );
    assert_eq!(
        shots(&z.tick(2001)),
        0,
        "first target tick opens FearTime without firing"
    );
    assert_eq!(shots(&z.tick(2002)), 1);
    let mut z = ZoneRuntime::restore_checkpoint(&z.checkpoint_bytes().unwrap()).unwrap();
    z.tick(2751);
    assert_eq!(hp(&z), 10000);
    z.tick(2752);
    assert_eq!(hp(&z), 9745, "255 DC at distance 5 arrives after 750 ms");
    assert_eq!(shots(&z.tick(4002)), 0, "strict attack deadline");
    assert_eq!(
        shots(&z.tick(4003)),
        0,
        "expired FearTime starts a fresh window"
    );
    assert_eq!(shots(&z.tick(4004)), 1);
    assert_eq!(
        z.native_monster_snapshots()
            .iter()
            .find(|m| m.object_id == ID)
            .unwrap()
            .position,
        point(20, 20)
    );
}

#[test]
fn town_archer_projectile_respects_impact_armour_and_disconnect() {
    let mut z = fixture(200);
    z.tick(2001);
    z.tick(2002);
    z.handle(ZoneCommand::UpdatePlayerCombatStats {
        session_id: SessionId::new("player"),
        stats: ZonePlayerCombatStats {
            min_ac: 300,
            max_ac: 300,
            ..Default::default()
        },
    });
    z.tick(2752);
    assert_eq!(hp(&z), 10000);
    let mut z = fixture(200);
    z.tick(2001);
    z.tick(2002);
    z.handle(ZoneCommand::Leave {
        session_id: SessionId::new("player"),
    });
    let out = z.tick(2752);
    assert!(!packets(&out)
        .iter()
        .any(|p| matches!(p, ServerPacket::DamageIndicator { object_id: 101, .. })));
}

#[test]
fn town_archer_retains_target_to_ten_tiles_then_returns_home_direction() {
    let mut z = fixture(200);
    z.tick(2001);
    z.tick(2002);
    let mut last_tick = Vec::new();
    for (seq, now) in [(1, 2300), (2, 2900), (3, 3500), (4, 4100), (5, 4700)] {
        z.handle(ZoneCommand::Walk {
            session_id: SessionId::new("player"),
            direction: MirDirection::Right,
            seq,
            now_ms: now,
        });
        last_tick = z.tick(now);
    }
    assert_eq!(
        z.player_position(&SessionId::new("player")),
        Some(point(30, 20))
    );
    // At 4100 the expired fear interval refreshes; 4700 fires at ten tiles.
    assert_eq!(
        shots(&last_tick),
        1,
        "retained targets may be beyond seven-tile FindTarget range"
    );
    z.handle(ZoneCommand::Walk {
        session_id: SessionId::new("player"),
        direction: MirDirection::Right,
        seq: 6,
        now_ms: 5300,
    });
    z.tick(5300);
    z.tick(6701);
    assert_eq!(
        z.player_position(&SessionId::new("player")),
        Some(point(31, 20))
    );
    let m = z
        .native_monster_snapshots()
        .into_iter()
        .find(|m| m.object_id == ID)
        .unwrap();
    assert_eq!(m.position, point(20, 20));
    let saved: serde_json::Value = serde_json::from_slice(&z.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(
        saved["native_monsters"][ID.to_string()]["direction"],
        serde_json::to_value(MirDirection::Down).unwrap()
    );
    assert_eq!(
        shots(&z.tick(9700)),
        0,
        "no chase or reacquire at eleven tiles"
    );
}
