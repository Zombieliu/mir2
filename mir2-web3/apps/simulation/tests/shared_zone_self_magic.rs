use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound, ZoneRuntime,
};

fn sid() -> SessionId {
    SessionId::new("caster")
}

fn fixture(mp: i32) -> ZoneRuntime {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: sid(),
        account_id: "isolated-test".into(),
        character_index: 1,
        object_id: 101,
        name: "Caster".into(),
        class: MirClass::Wizard,
        gender: MirGender::Male,
        level: 100,
        hp: 60,
        max_hp: 60,
        mp,
        map_file_name: "0".into(),
        position: Point { x: 330, y: 270 },
        direction: MirDirection::Down,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone
}

fn cast(spell: Spell, target: Point, object_id: u32, now_ms: u64) -> ZoneCommand {
    ZoneCommand::PlayerCastMagic {
        session_id: sid(),
        object_id,
        spell,
        direction: MirDirection::Right,
        target,
        cast: true,
        level: 3,
        damage: 30,
        mp_cost: 10,
        cooldown_ms: 5000,
        now_ms,
    }
}

fn packets(out: &[ZoneOutbound]) -> Vec<&ServerPacket> {
    out.iter()
        .flat_map(|out| match out {
            ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } => {
                packets.iter().collect::<Vec<_>>()
            }
            _ => Vec::new(),
        })
        .collect()
}

fn accepted(out: &[ZoneOutbound], spell: Spell) -> bool {
    packets(out)
        .iter()
        .any(|packet| matches!(packet,ServerPacket::Magic{spell:s,cast:true,..} if *s==spell))
}

#[test]
fn original_self_magic_ignores_cursor_point_preserves_actor_direction_and_spends_once() {
    for spell in [
        Spell::Repulsion,
        Spell::EnergyRepulsor,
        Spell::MagicShield,
        Spell::MagicBooster,
        Spell::ProtectionField,
        Spell::Rage,
        Spell::Fury,
        Spell::Hiding,
    ] {
        let mut zone = fixture(100);
        let cursor = Point { x: 330, y: 276 };
        assert!(
            zone.can_player_cast_magic(
                &sid(),
                0,
                spell,
                MirDirection::Right,
                &cursor,
                true,
                30,
                10,
                5000,
                10
            ),
            "{spell:?} preflight"
        );
        let out = zone.handle(cast(spell, cursor.clone(), 0, 10));
        assert!(accepted(&out, spell), "{spell:?} real cast");
        assert!(
            packets(&out)
                .iter()
                .any(|packet| matches!(packet,ServerPacket::ObjectMagic{
            object_id:101,location,direction:MirDirection::Right,target_id:0,target,spell:s,..}
            if *s==spell && *location==(Point{x:330,y:270}) && *target==cursor)),
            "{spell:?} preserve Crystal packet semantics"
        );
        assert_eq!(zone.player_vitals(&sid()).unwrap().2, 90);
        let repeat = zone.handle(cast(spell, cursor, 0, 11));
        assert!(!accepted(&repeat, spell));
        assert_eq!(zone.player_vitals(&sid()).unwrap().2, 90);
    }
}

#[test]
fn original_caster_summons_ignore_cursor_and_spawn_near_authoritative_player() {
    for spell in [
        Spell::Mirroring,
        Spell::SummonSkeleton,
        Spell::SummonShinsu,
        Spell::SummonHolyDeva,
    ] {
        let mut zone = fixture(100);
        let cursor = Point { x: 340, y: 276 };
        assert!(zone.can_player_cast_magic(
            &sid(),
            0,
            spell,
            MirDirection::Right,
            &cursor,
            true,
            0,
            10,
            5000,
            10
        ));
        assert!(accepted(
            &zone.handle(cast(spell, cursor.clone(), 0, 10)),
            spell
        ));
        let out = zone.tick(2000);
        assert!(packets(&out).iter().any(|packet|matches!(packet,ServerPacket::ObjectMonster{info}
            if info.master_object_id==101 && (info.location.x-330).abs()<=1 && (info.location.y-270).abs()<=1
                && info.location!=cursor)),"{spell:?} summon uses owner tile");
        assert_eq!(zone.player_vitals(&sid()).unwrap().2, 90);
    }
}

#[test]
fn self_magic_cursor_fix_does_not_bypass_mp_owner_or_other_target_rules() {
    let cursor = Point { x: 330, y: 276 };
    let mut empty = fixture(0);
    assert!(!accepted(
        &empty.handle(cast(Spell::Repulsion, cursor.clone(), 0, 10)),
        Spell::Repulsion
    ));
    assert_eq!(empty.player_vitals(&sid()).unwrap().2, 0);
    let mut zone = fixture(100);
    assert!(!accepted(
        &zone.handle(cast(Spell::MagicShield, cursor.clone(), 999, 10)),
        Spell::MagicShield
    ));
    assert!(!zone.can_player_cast_magic(
        &SessionId::new("missing"),
        0,
        Spell::Repulsion,
        MirDirection::Right,
        &cursor,
        true,
        30,
        10,
        5000,
        10
    ));
    for spell in [Spell::EnergyShield, Spell::Healing] {
        assert!(
            !zone.can_player_cast_magic(
                &sid(),
                0,
                spell,
                MirDirection::Right,
                &cursor,
                true,
                30,
                10,
                5000,
                10
            ),
            "{spell:?} still validates real target"
        );
    }
    let far = Point { x: 380, y: 270 };
    for spell in [Spell::FireWall, Spell::FireBang, Spell::SummonSnakes] {
        assert!(
            !zone.can_player_cast_magic(
                &sid(),
                0,
                spell,
                MirDirection::Right,
                &far,
                true,
                30,
                10,
                5000,
                10
            ),
            "{spell:?} remains ground targeted"
        );
    }
    assert_eq!(zone.player_vitals(&sid()).unwrap().2, 100);
}

#[test]
fn directional_and_ground_spells_keep_their_distinct_target_semantics() {
    let mut zone = fixture(100);
    let cursor = Point { x: 335, y: 270 };
    let lightning = zone.handle(cast(Spell::Lightning, cursor.clone(), 0, 10));
    assert!(accepted(&lightning, Spell::Lightning));
    assert!(packets(&lightning)
        .iter()
        .any(|packet| matches!(packet,ServerPacket::ObjectMagic{
        spell:Spell::Lightning,direction:MirDirection::Right,location,target,..}
        if *location==(Point{x:330,y:270}) && *target==cursor)));
    let mut zone = fixture(100);
    assert!(accepted(
        &zone.handle(cast(Spell::FireWall, cursor.clone(), 0, 10)),
        Spell::FireWall
    ));
    let out = zone.tick(2000);
    assert!(
        packets(&out)
            .iter()
            .any(|packet| matches!(packet,ServerPacket::ObjectSpell{info}
        if info.spell==Spell::FireWall && info.location==cursor)),
        "FireWall retains cursor center"
    );
}
