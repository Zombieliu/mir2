use super::*;
use mir2_protocol::MirGender;

fn fixture() -> (ZoneRuntime, SessionId) {
    let mut zone =
        ZoneRuntime::new_with_collision(ZoneKey::for_map("0"), ZoneCollision::unbounded());
    let owner = SessionId::new("soulfire-owner");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: owner.clone(),
        account_id: "soulfire-account".into(),
        character_index: 7,
        object_id: 101,
        name: "Taoist".into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 23,
        hp: 100,
        max_hp: 100,
        mp: 100,
        map_file_name: "0".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }));
    zone.handle(ZoneCommand::SpawnMonster {
        session_id: owner.clone(),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: 9101,
            name: "Deer".into(),
            name_colour_argb: -1,
            image: 0,
            ai: 0,
            disposition: Some(crate::WorldEntityDisposition::Neutral),
            level: 1,
            max_hp: 5,
            hp: 5,
            experience: 1,
            move_speed_ms: 600,
            attack_speed_ms: 1200,
            friendly_guild: None,
            position: Point { x: 12, y: 10 },
            direction: MirDirection::Left,
            defense: Default::default(),
            respawn: None,
            drops: Vec::new(),
        },
    });
    (zone, owner)
}

fn launch(
    zone: &mut ZoneRuntime,
    owner: &SessionId,
    spell: Spell,
    damage: i32,
) -> Vec<ZoneOutbound> {
    zone.player_cast_native_magic(
        owner,
        9101,
        spell,
        MirDirection::Right,
        Point { x: 12, y: 10 },
        true,
        0,
        damage,
        3,
        1800,
        100,
    )
}

fn practice(outbounds: &[ZoneOutbound]) -> Vec<&ZoneMagicPracticeReceipt> {
    outbounds
        .iter()
        .filter_map(|outbound| match outbound {
            ZoneOutbound::MagicPractice { receipt } => Some(receipt),
            _ => None,
        })
        .collect()
}

#[test]
fn soulfire_practice_is_resolved_positive_hit_evidence_including_lethal_remainder() {
    let (mut zone, owner) = fixture();
    assert!(practice(&launch(&mut zone, &owner, Spell::SoulFireBall, 7)).is_empty());
    assert_eq!(
        zone.native_monsters[&9101].hp, 5,
        "launch must not resolve or award practice"
    );
    let outbounds = zone.resolve_pending_native_monster_hits(100);
    let receipts = practice(&outbounds);
    assert_eq!(receipts.len(), 1);
    let receipt = receipts[0];
    assert_eq!(
        receipt.damage, 5,
        "use actual positive HP loss, including the lethal remainder"
    );
    assert_eq!(receipt.session_id, owner);
    assert_eq!(receipt.account_id, "soulfire-account");
    assert_eq!(receipt.character_index, 7);
    assert_eq!(receipt.object_id, 101);
    assert_eq!(receipt.target_object_id, 9101);
    assert!(practice(&zone.resolve_pending_native_monster_hits(200)).is_empty());
}

#[test]
fn soulfire_practice_zero_missing_dead_or_moved_target_is_not_awarded() {
    for case in 0..5 {
        let (mut zone, owner) = fixture();
        if case == 4 {
            let target = zone.native_monsters.get_mut(&9101).unwrap();
            target.defense.min_mac = 100;
            target.defense.max_mac = 100;
        }
        launch(
            &mut zone,
            &owner,
            Spell::SoulFireBall,
            if case == 0 { 0 } else { 7 },
        );
        match case {
            1 => {
                zone.native_monsters.remove(&9101);
            }
            2 => {
                let monster = zone.native_monsters.get_mut(&9101).unwrap();
                monster.dead = true;
                monster.hp = 0;
            }
            3 => {
                zone.native_monsters.get_mut(&9101).unwrap().position.x += 3;
            }
            _ => {}
        }
        let resolved = zone.resolve_pending_native_monster_hits(100);
        assert!(practice(&resolved).is_empty(), "case {case}");
        if case == 4 {
            assert_eq!(
                zone.native_monsters[&9101].hp, 5,
                "absorbed cast must not alter HP"
            );
            assert!(resolved.iter().any(|outbound| matches!(outbound,
                ZoneOutbound::ToMany { packets, .. } if packets.iter().any(|packet|
                    matches!(packet, ServerPacket::DamageIndicator { damage: 0, .. })))));
        }
    }
}

#[test]
fn soulfire_practice_pending_cast_metadata_survives_zone_checkpoint() {
    let (mut zone, owner) = fixture();
    // Strict restore reconstructs the signed map collision instead of storing
    // the test's unbounded collision in the checkpoint. Use that same source
    // here so the test verifies pending metadata rather than module reanchoring.
    zone.collision = ZoneRuntime::new(ZoneKey::for_map("0")).collision;
    launch(&mut zone, &owner, Spell::SoulFireBall, 7);
    let bytes = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&bytes).unwrap();
    let resolved = restored.resolve_pending_native_monster_hits(100);
    assert_eq!(practice(&resolved).len(), 1);
    assert_eq!(practice(&resolved)[0].damage, 5);
}

#[test]
fn soulfire_practice_rejects_changed_presence_and_revived_life() {
    for changed_object in [false, true] {
        let (mut zone, owner) = fixture();
        launch(&mut zone, &owner, Spell::SoulFireBall, 7);
        let player = zone.players.get_mut(&owner).unwrap();
        if changed_object {
            player.object_id += 1;
        } else {
            player.life_generation += 1;
        }
        assert!(practice(&zone.resolve_pending_native_monster_hits(100)).is_empty());
    }
}

#[test]
fn soulfire_practice_lawful_projectile_survives_caster_death_until_presence_changes() {
    let (mut zone, owner) = fixture();
    launch(&mut zone, &owner, Spell::SoulFireBall, 7);
    let player = zone.players.get_mut(&owner).unwrap();
    player.dead = true;
    player.hp = 0;
    assert_eq!(
        practice(&zone.resolve_pending_native_monster_hits(100)).len(),
        1
    );
}

#[test]
fn soulfire_practice_does_not_extend_other_magic_or_reward_departed_owner() {
    let (mut zone, owner) = fixture();
    launch(&mut zone, &owner, Spell::ThunderBolt, 7);
    assert!(practice(&zone.resolve_pending_native_monster_hits(100)).is_empty());
    let (mut zone, owner) = fixture();
    launch(&mut zone, &owner, Spell::SoulFireBall, 7);
    zone.leave(&owner);
    assert!(practice(&zone.resolve_pending_native_monster_hits(100)).is_empty());
}

#[test]
fn fireball_practice_positive_primary_hit_is_spell_typed_and_not_launch_reward() {
    for spell in [Spell::FireBall, Spell::GreatFireBall] {
        let (mut zone, owner) = fixture();
        zone.players.get_mut(&owner).unwrap().class = MirClass::Wizard;
        assert!(practice(&launch(&mut zone, &owner, spell, 7)).is_empty());
        assert_eq!(zone.native_monsters[&9101].hp, 5);
        let resolved = zone.resolve_pending_native_monster_hits(100);
        let receipts = practice(&resolved);
        assert_eq!(receipts.len(), 1, "{spell:?}");
        assert_eq!(receipts[0].spell.spell(), spell);
        assert_eq!(receipts[0].damage, 5);
        assert_eq!(receipts[0].target_object_id, 9101);
        assert!(practice(&zone.resolve_pending_native_monster_hits(200)).is_empty());
    }
    for spell in [Spell::FireBounce, Spell::FrostCrunch, Spell::ThunderBolt] {
        assert_eq!(ZoneMagicPracticeSpell::from_spell(spell), None);
    }
}

#[test]
fn fireball_practice_rejects_zero_invalid_target_and_changed_owner_but_keeps_lawful_caster_death() {
    for spell in [Spell::FireBall, Spell::GreatFireBall] {
        for case in 0..9 {
            let (mut zone, owner) = fixture();
            zone.players.get_mut(&owner).unwrap().class = MirClass::Wizard;
            if case == 4 {
                let target = zone.native_monsters.get_mut(&9101).unwrap();
                target.defense.min_mac = 100;
                target.defense.max_mac = 100;
            }
            launch(&mut zone, &owner, spell, if case == 0 { 0 } else { 7 });
            match case {
                1 => {
                    zone.native_monsters.remove(&9101);
                }
                2 => {
                    let target = zone.native_monsters.get_mut(&9101).unwrap();
                    target.dead = true;
                    target.hp = 0;
                }
                3 => zone.native_monsters.get_mut(&9101).unwrap().position.x += 3,
                5 => zone.players.get_mut(&owner).unwrap().object_id += 1,
                6 => zone.players.get_mut(&owner).unwrap().life_generation += 1,
                7 => {
                    zone.leave(&owner);
                }
                8 => {
                    let caster = zone.players.get_mut(&owner).unwrap();
                    caster.dead = true;
                    caster.hp = 0;
                }
                _ => {}
            }
            let resolved = zone.resolve_pending_native_monster_hits(100);
            assert_eq!(
                practice(&resolved).len(),
                usize::from(case == 8),
                "{spell:?} case {case}"
            );
        }
    }
}

#[test]
fn fireball_practice_pending_metadata_survives_checkpoint_and_legacy_defaults_only_soulfire() {
    for spell in [Spell::FireBall, Spell::GreatFireBall] {
        let (mut zone, owner) = fixture();
        zone.collision = ZoneRuntime::new(ZoneKey::for_map("0")).collision;
        launch(&mut zone, &owner, spell, 7);
        let mut restored =
            ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
        let resolved = restored.resolve_pending_native_monster_hits(100);
        assert_eq!(practice(&resolved).len(), 1);
        assert_eq!(practice(&resolved)[0].spell.spell(), spell);
    }
    let (mut zone, owner) = fixture();
    launch(&mut zone, &owner, Spell::SoulFireBall, 7);
    let resolved = zone.resolve_pending_native_monster_hits(100);
    let mut legacy = serde_json::to_value(practice(&resolved)[0]).unwrap();
    legacy.as_object_mut().unwrap().remove("spell");
    let legacy: ZoneMagicPracticeReceipt = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.spell, ZoneMagicPracticeSpell::SoulFireBall);
    let mut invalid = serde_json::to_value(legacy).unwrap();
    invalid["spell"] = serde_json::json!("thunderBolt");
    assert!(serde_json::from_value::<ZoneMagicPracticeReceipt>(invalid).is_err());
}
