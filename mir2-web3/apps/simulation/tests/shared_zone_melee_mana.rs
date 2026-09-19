use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{SessionId, ZoneCollision, ZoneCommand, ZoneJoin, ZoneKey, ZoneOutbound, ZoneRuntime};

fn sid(name: &str) -> SessionId { SessionId::new(name) }

fn fixture(hp: i32, mp: i32) -> ZoneRuntime {
    let mut zone = ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    for (name, object_id, x) in [("attacker", 101, 330), ("victim", 102, 331)] {
        let mut join = ZoneJoin {
            session_id: sid(name), account_id: name.into(), character_index: object_id as i32,
            object_id, name: name.into(), class: MirClass::Warrior, gender: MirGender::Male,
            level: 60, hp: if name == "attacker" { hp } else { 60 }, max_hp: 60, mp,
            map_file_name: "0".into(), position: Point { x, y: 270 }, direction: MirDirection::Right,
            chat_profile: Default::default(), combat_stats: Default::default(),
        };
        join.chat_profile.attack_mode = 5;
        join.chat_profile.in_safe_zone = false;
        zone.handle(ZoneCommand::Join(join));
        zone.handle(ZoneCommand::sync_player_combat_state(sid(name), MirClass::Warrior, false, false, true, false, false, false));
    }
    zone
}

fn attack(spell: Spell, now_ms: u64, target: u32) -> ZoneCommand {
    ZoneCommand::PlayerAttackObject { session_id: sid("attacker"), object_id: target,
        direction: MirDirection::Right, spell: spell as u8, level: 3, attack_type: 0, damage: 1, now_ms }
}

fn accepted(out: &[ZoneOutbound]) -> bool {
    out.iter().any(|out| match out {
        ZoneOutbound::ToSession { packets, .. } | ZoneOutbound::ToMany { packets, .. } | ZoneOutbound::ToAll { packets } =>
            packets.iter().any(|p| matches!(p, ServerPacket::ObjectAttack { info } if info.object_id == 101)),
        _ => false,
    })
}

#[test]
fn twin_drake_atomic_payment_preserves_injured_hp_and_rejects_duplicate() {
    let magic = mir2_game_data::crystal_magic_by_spell("TwinDrakeBlade").unwrap();
    let cost = i32::from(magic.base_cost) + 3 * i32::from(magic.level_cost);
    let mut zone = fixture(7, 100);
    assert!(accepted(&zone.handle(attack(Spell::TwinDrakeBlade, 100, 102))));
    assert_eq!(zone.player_vitals(&sid("attacker")), Some((7, 60, 100 - cost)));
    assert!(!accepted(&zone.handle(attack(Spell::TwinDrakeBlade, 101, 102))));
    assert_eq!(zone.player_vitals(&sid("attacker")), Some((7, 60, 100 - cost)));
    zone.tick(102);
    assert_eq!(zone.player_vitals(&sid("attacker")), Some((7, 60, 100 - cost)));
}

#[test]
fn twin_drake_insufficient_mana_dead_or_missing_target_never_hits_or_spends() {
    for (hp, mp, target) in [(7, 0, 102), (0, 100, 102), (7, 100, 999)] {
        let mut zone = fixture(hp, mp);
        let before = zone.player_vitals(&sid("attacker"));
        let victim = zone.player_vitals(&sid("victim"));
        assert!(!accepted(&zone.handle(attack(Spell::TwinDrakeBlade, 100, target))));
        assert_eq!(zone.player_vitals(&sid("attacker")), before);
        assert_eq!(zone.player_vitals(&sid("victim")), victim);
    }
}

#[test]
fn ordinary_shared_swing_has_no_mana_payment() {
    let mut zone = fixture(7, 0);
    assert!(accepted(&zone.handle(attack(Spell::None, 100, 102))));
    assert_eq!(zone.player_vitals(&sid("attacker")), Some((7, 60, 0)));
}
