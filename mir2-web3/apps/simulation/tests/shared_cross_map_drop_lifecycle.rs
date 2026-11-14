//! Prepared shared-manager fixtures, not network/player-journey acceptance.
//! Positive damage, ordinary Join/Leave and custody use public server commands.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, Spell};
use mir2_simulation::{
    GroundDropClaimTicket, GroundDropLootSnapshot, GroundDropSnapshot, SessionId,
    WorldEntityDisposition, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneMonsterKillAward,
    ZoneMonsterSpawn, ZoneOutbound,
};

const SOURCE: &str = "cross-map-owner-source";
const DEST: &str = "cross-map-owner-destination";
const MONSTER: u32 = 90_101;
fn sid(name: &str) -> SessionId {
    SessionId::new(name)
}
fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}
fn join(name: &str, map: &str) -> ZoneJoin {
    ZoneJoin {
        session_id: sid(name),
        account_id: format!("{name}-account"),
        character_index: 0,
        object_id: if name == "a" { 101 } else { 102 },
        name: name.into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 40,
        hp: 100_000,
        max_hp: 100_000,
        mp: 100,
        map_file_name: map.into(),
        position: if name == "a" {
            point(9, 10)
        } else {
            point(10, 11)
        },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}
fn admit(manager: &mut ZoneManager, name: &str, map: &str) {
    manager.join(join(name, map));
    manager.handle(ZoneCommand::sync_player_combat_state(
        sid(name),
        MirClass::Warrior,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
}
fn fixture() -> ZoneManager {
    fixture_hp(100)
}
fn fixture_hp(hp: i32) -> ZoneManager {
    let mut manager = ZoneManager::new();
    admit(&mut manager, "a", SOURCE);
    admit(&mut manager, "b", SOURCE);
    manager.handle(ZoneCommand::SpawnMonster {
        session_id: sid("a"),
        now_ms: 0,
        monster: ZoneMonsterSpawn {
            crystal_drop_seed: None,
            object_id: MONSTER,
            name: "Skeleton".into(),
            name_colour_argb: -1,
            image: 3,
            ai: 3,
            disposition: Some(WorldEntityDisposition::Hostile),
            level: 20,
            hp,
            max_hp: hp,
            experience: 123,
            move_speed_ms: 100_000,
            attack_speed_ms: 100_000,
            friendly_guild: None,
            defense: Default::default(),
            respawn: None,
            position: point(10, 10),
            direction: MirDirection::Down,
            drops: vec![GroundDropSnapshot {
                object_id: 0,
                name: "Gold".into(),
                name_colour_argb: -1,
                icon: 0,
                x: 0,
                y: 0,
                quantity: 7,
                source_monster: "Skeleton".into(),
                owner_object_id: None,
                ownership_remaining_ticks: Some(60),
                loot: GroundDropLootSnapshot::Gold { amount: 7 },
            }],
        },
    });
    manager
}
fn hit(manager: &mut ZoneManager, name: &str, damage: i32, now_ms: u64) -> Vec<ZoneOutbound> {
    let mut out = manager.handle(ZoneCommand::PlayerAttackObject {
        session_id: sid(name),
        object_id: MONSTER,
        direction: if name == "a" {
            MirDirection::Right
        } else {
            MirDirection::Up
        },
        spell: 0,
        level: 0,
        attack_type: 0,
        damage,
        now_ms,
    });
    out.extend(manager.tick_all(now_ms));
    out
}
fn awards(out: &[ZoneOutbound]) -> Vec<(&SessionId, &ZoneMonsterKillAward)> {
    out.iter()
        .filter_map(|event| match event {
            ZoneOutbound::MonsterKillAward { session_id, award } => Some((session_id, award)),
            ZoneOutbound::OwnedMonsterKillAward {
                session_id, award, ..
            } => Some((session_id, award)),
            _ => None,
        })
        .collect()
}
fn kill(manager: &mut ZoneManager) -> GroundDropSnapshot {
    let out = hit(manager, "a", 100, 100);
    let awards = awards(&out);
    assert_eq!(
        awards.len(),
        1,
        "fixture must perform a real positive fatal impact: {out:?}"
    );
    assert_eq!(awards[0].1.experience, 123);
    awards[0].1.drops[0].clone()
}
fn claim(
    manager: &mut ZoneManager,
    name: &str,
    drop: &GroundDropSnapshot,
    at: u64,
) -> Option<GroundDropClaimTicket> {
    manager
        .handle(ZoneCommand::ClaimGroundDrop {
            session_id: sid(name),
            object_id: Some(drop.object_id),
            target: point(drop.x, drop.y),
            group_members: vec![],
            now_ms: at,
        })
        .into_iter()
        .find_map(|event| match event {
            ZoneOutbound::GroundDropClaimedWithTicket { ticket, .. } => Some(ticket),
            _ => None,
        })
}

#[test]
fn owner_cross_map_retains_experience_and_source_drop() {
    let mut manager = fixture();
    assert!(awards(&hit(&mut manager, "a", 20, 100)).is_empty());
    admit(&mut manager, "a", DEST);
    let out = hit(&mut manager, "b", 80, 1_100);
    let awarded = awards(&out);
    assert_eq!(
        awarded.len(),
        1,
        "ordinary death emits one owner award: {out:?}"
    );
    assert_eq!(
        awarded[0].0,
        &sid("a"),
        "map membership must not despawn Crystal EXPOwner"
    );
    assert_eq!(awarded[0].1.drops[0].owner_object_id, Some(101));
    assert_eq!(
        manager
            .zone(&ZoneKey::for_map(SOURCE))
            .unwrap()
            .ground_drop_count(),
        1
    );
    assert_eq!(
        manager
            .zone(&ZoneKey::for_map(DEST))
            .unwrap()
            .ground_drop_count(),
        0
    );
}

#[test]
fn owner_return_to_source_does_not_revoke_claim() {
    let mut manager = fixture();
    hit(&mut manager, "a", 20, 100);
    admit(&mut manager, "a", DEST);
    admit(&mut manager, "a", SOURCE);
    let out = hit(&mut manager, "b", 80, 1_100);
    let awarded = awards(&out);
    assert_eq!(awarded.len(), 1);
    assert_eq!(
        awarded[0].0,
        &sid("a"),
        "return Join is membership, not a new spawned owner"
    );
}

#[test]
fn owned_native_drop_has_strict_original_one_minute_boundary() {
    for (at, open) in [(60_099, false), (60_100, false), (60_101, true)] {
        let mut manager = fixture();
        let drop = kill(&mut manager);
        assert_eq!(
            claim(&mut manager, "b", &drop, at).is_some(),
            open,
            "owned native monster ItemOwner expires strictly after killedAt+60000, at={at}"
        );
    }
}

#[test]
fn repeated_sync_does_not_restart_original_protection() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    manager.handle(ZoneCommand::SyncGroundDrops {
        session_id: sid("a"),
        drops: vec![drop.clone()],
        now_ms: 50_000,
    });
    assert!(
        claim(&mut manager, "b", &drop, 60_101).is_some(),
        "sync is projection, not a new birth"
    );
}

#[test]
fn cancelled_claim_does_not_restart_original_protection() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    let ticket = claim(&mut manager, "a", &drop, 15_000).expect("actual owner can reserve drop");
    manager.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: sid("a"),
        ticket,
        now_ms: 50_000,
    });
    assert!(
        claim(&mut manager, "b", &drop, 60_101).is_some(),
        "rollback must retain original birth"
    );
}

#[test]
fn true_leave_from_other_map_releases_original_ground_owner() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    admit(&mut manager, "a", DEST);
    manager.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    assert!(
        claim(&mut manager, "b", &drop, 1_000).is_some(),
        "global Node despawn releases source ItemOwner"
    );
}

#[test]
fn fresh_join_cannot_inherit_old_epoch_even_with_reused_ids() {
    let mut manager = fixture();
    hit(&mut manager, "a", 20, 100);
    admit(&mut manager, "a", DEST);
    let old = manager.online_owner_proof_for_session(&sid("a")).unwrap();
    manager.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    admit(&mut manager, "a", SOURCE);
    let new = manager.online_owner_proof_for_session(&sid("a")).unwrap();
    assert_ne!(old, new);
    assert!(!manager.online_owner_proof_is_current(&old));
    let out = hit(&mut manager, "b", 80, 1_100);
    assert_eq!(awards(&out)[0].0, &sid("b"));
}

#[test]
fn encoded_owner_binds_manager_epoch_account_character_object_and_session() {
    let manager = fixture();
    let original = manager.online_owner_proof_for_session(&sid("a")).unwrap();
    assert!(manager.online_owner_proof_matches_session(&original, &sid("a")));
    assert!(!manager.online_owner_proof_matches_session(&original, &sid("b")));
    for (field, value) in [
        ("namespace", serde_json::json!("forged")),
        ("epoch", serde_json::json!(999)),
        ("account_id", serde_json::json!("b-account")),
        ("character_index", serde_json::json!(1)),
        ("object_id", serde_json::json!(102)),
        ("session_id", serde_json::json!("b")),
        ("unknown_authority", serde_json::json!(true)),
    ] {
        let mut forged: serde_json::Value = serde_json::from_str(&original).unwrap();
        forged[field] = value;
        assert!(
            !manager.online_owner_proof_is_current(&forged.to_string()),
            "accepted forged {field}"
        );
    }
    assert!(
        !fixture().online_owner_proof_is_current(&original),
        "another manager is another Node namespace"
    );
}

#[test]
fn strict_expiration_can_be_renewed_only_by_source_legal_current_owner() {
    for (at, expected) in [(5_100, "a"), (5_101, "b")] {
        let mut manager = fixture();
        hit(&mut manager, "a", 20, 100);
        admit(&mut manager, "a", DEST);
        let out = hit(&mut manager, "b", 80, at);
        assert_eq!(
            awards(&out)[0].0,
            &sid(expected),
            "five second strict boundary at={at}"
        );
    }
}

#[test]
fn periodic_poison_renews_owner_after_caster_changed_map() {
    let mut manager = fixture_hp(12);
    manager.handle(ZoneCommand::sync_player_combat_state(
        sid("a"),
        MirClass::Taoist,
        true,
        false,
        false,
        false,
        false,
        false,
    ));
    let cast = manager.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("a"),
        object_id: MONSTER,
        spell: Spell::Poisoning,
        direction: MirDirection::Right,
        target: point(10, 10),
        cast: true,
        level: 3,
        damage: 5,
        mp_cost: 0,
        cooldown_ms: 0,
        item_param: 1,
        now_ms: 100,
    });
    assert!(
        !cast.is_empty(),
        "public prepared poison command must be admitted"
    );
    admit(&mut manager, "a", DEST);
    assert!(awards(&manager.tick_all(2_100)).is_empty());
    assert!(awards(&manager.tick_all(4_100)).is_empty());
    let out = manager.tick_all(6_100);
    let award = awards(&out);
    assert_eq!(
        award.len(),
        1,
        "periodic positive poison renews across initial 5s: {out:?}"
    );
    assert_eq!(award[0].0, &sid("a"));
}

#[test]
fn detached_custody_retains_deadline_ttl_and_stable_ledger_key() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    let before = manager
        .ground_drop_absolute_clocks_for_key(&ZoneKey::for_map(SOURCE), drop.object_id)
        .unwrap();
    let ticket = claim(&mut manager, "a", &drop, 15_000).unwrap();
    let source = manager
        .detach_ground_drop_claim(&sid("a"), &ticket)
        .unwrap();
    let mut forged = ticket.clone();
    forged.drop_generation += 1;
    assert!(!manager.has_detached_ground_drop_claim_ticket(&source, &forged));
    manager
        .restore_detached_ground_drop_claim(&source, &ticket, 50_000)
        .unwrap();
    assert_eq!(
        manager
            .ground_drop_absolute_clocks_for_key(&source, drop.object_id)
            .unwrap(),
        before
    );
    let next = claim(&mut manager, "b", &drop, 60_101).unwrap();
    assert_ne!(next.claim_id, ticket.claim_id);
    assert_eq!(next.idempotency_key, ticket.idempotency_key);
    assert_eq!(next.payload_digest, ticket.payload_digest);
}

#[test]
fn expired_held_ground_ttl_never_resurrects_on_cancel() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    let ttl = manager
        .ground_drop_absolute_clocks_for_key(&ZoneKey::for_map(SOURCE), drop.object_id)
        .unwrap()
        .1
        .unwrap();
    let ticket = claim(&mut manager, "a", &drop, 1_000).unwrap();
    manager.handle(ZoneCommand::CancelGroundDropClaimWithTicket {
        session_id: sid("a"),
        ticket,
        now_ms: ttl + 1,
    });
    assert!(manager
        .ground_drop_snapshot_for_key(&ZoneKey::for_map(SOURCE), drop.object_id)
        .is_none());
    assert!(claim(&mut manager, "b", &drop, ttl + 1).is_none());
}

#[test]
fn cold_manager_checkpoint_never_recreates_original_online_nodes() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    let proof = manager.online_owner_proof_for_session(&sid("a")).unwrap();
    let bytes = manager.checkpoint_bytes().unwrap();
    let mut restored = ZoneManager::restore_checkpoint(&bytes).unwrap();
    assert!(!restored.online_owner_proof_is_current(&proof));
    admit(&mut restored, "a", SOURCE);
    admit(&mut restored, "b", SOURCE);
    assert!(
        claim(&mut restored, "b", &drop, 1_000).is_some(),
        "cold owner Nodes no longer exist"
    );
    let mut forged: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged["online_identities"]["next_epoch"] = serde_json::json!(99_999);
    assert!(
        ZoneManager::restore_checkpoint(&serde_json::to_vec(&forged).unwrap())
            .unwrap_err()
            .contains("root mismatch")
    );
}

#[test]
fn source_issuance_binds_original_reward_before_and_after_map_membership_change() {
    let mut manager = fixture();
    let killed = hit(&mut manager, "a", 100, 100);
    let (proof, award) = killed
        .iter()
        .find_map(|out| match out {
            ZoneOutbound::OwnedMonsterKillAward {
                online_owner,
                award,
                ..
            } => Some((online_owner.clone(), award.clone())),
            _ => None,
        })
        .unwrap();
    let source = ZoneKey::for_map(SOURCE);
    let destination = ZoneKey::for_map(DEST);
    assert!(manager.issued_monster_award_is_current(&source, &sid("a"), &proof, &award));
    admit(&mut manager, "a", DEST);
    assert!(manager.issued_monster_award_is_current(&source, &sid("a"), &proof, &award));
    assert!(!manager.issued_monster_award_is_current(&destination, &sid("a"), &proof, &award));
    assert!(!manager.issued_monster_award_is_current(&source, &sid("b"), &proof, &award));
    let mut forged = award.clone();
    forged.experience += 1;
    assert!(!manager.issued_monster_award_is_current(&source, &sid("a"), &proof, &forged));
    forged = award.clone();
    forged.drops[0].quantity += 1;
    assert!(!manager.issued_monster_award_is_current(&source, &sid("a"), &proof, &forged));
    let cold = ZoneManager::restore_checkpoint(&manager.checkpoint_bytes().unwrap()).unwrap();
    assert!(!cold.issued_monster_award_is_current(&source, &sid("a"), &proof, &award));
    manager.acknowledge_issued_monster_award(&source, &sid("a"), &award);
    assert!(!manager.issued_monster_award_is_current(&source, &sid("a"), &proof, &award));
}

#[test]
fn same_map_fresh_admission_releases_old_ground_owner_and_experience() {
    let mut manager = fixture();
    hit(&mut manager, "a", 20, 100);
    let old = manager.online_owner_proof_for_session(&sid("a")).unwrap();
    // A same-map re-admission is a newly spawned object, unlike atomic transfer.
    admit(&mut manager, "a", SOURCE);
    assert_ne!(
        manager.online_owner_proof_for_session(&sid("a")).unwrap(),
        old
    );
    assert_eq!(awards(&hit(&mut manager, "b", 80, 1_100))[0].0, &sid("b"));
}

#[test]
fn manual_unowned_drop_projection_never_invents_native_item_owner() {
    let mut manager = fixture();
    let mut drop = GroundDropSnapshot {
        object_id: 91_001,
        name: "manual-gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 10,
        y: 11,
        quantity: 2,
        source_monster: "manual-or-player-death".into(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: 2 },
    };
    manager.handle(ZoneCommand::SyncGroundDrops {
        session_id: sid("a"),
        drops: vec![drop.clone()],
        now_ms: 100,
    });
    assert_eq!(
        manager.native_ground_drop_deadline_for_key(&ZoneKey::for_map(SOURCE), drop.object_id),
        None
    );
    assert!(claim(&mut manager, "b", &drop, 101).is_some());
    // An unrelated legacy supplied policy is kept separate from native births.
    drop.object_id += 1;
    drop.owner_object_id = Some(101);
    drop.ownership_remaining_ticks = Some(20);
    manager.handle(ZoneCommand::SyncGroundDrops {
        session_id: sid("a"),
        drops: vec![drop.clone()],
        now_ms: 100,
    });
    assert_eq!(
        manager.native_ground_drop_deadline_for_key(&ZoneKey::for_map(SOURCE), drop.object_id),
        None
    );
}

#[test]
fn dead_online_owner_keeps_source_ground_protection_until_true_despawn() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    admit(&mut manager, "a", DEST);
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("a"),
        hp: 100,
        max_hp: 100,
        mp: 0,
        dead: true,
    });
    assert!(
        claim(&mut manager, "b", &drop, 1_000).is_none(),
        "full-HP Dead still has an online global Node"
    );
    manager.handle(ZoneCommand::Leave {
        session_id: sid("a"),
    });
    assert!(claim(&mut manager, "b", &drop, 1_001).is_some());
}

#[test]
fn irregular_tick_cadence_does_not_replace_birth_with_latest_sync_or_tick() {
    let mut manager = fixture();
    let drop = kill(&mut manager);
    let source = ZoneKey::for_map(SOURCE);
    let clocks = manager
        .ground_drop_absolute_clocks_for_key(&source, drop.object_id)
        .unwrap();
    for at in [999, 3_124, 18_441, 50_009, 60_100] {
        manager.tick_all(at);
        assert_eq!(
            manager
                .ground_drop_absolute_clocks_for_key(&source, drop.object_id)
                .unwrap(),
            clocks
        );
    }
    assert!(claim(&mut manager, "b", &drop, 60_100).is_none());
    assert!(claim(&mut manager, "b", &drop, 60_101).is_some());
}

#[test]
fn parallel_source_zone_tick_matches_sequential_cross_map_owner_and_payload() {
    fn prepared() -> ZoneManager {
        let mut m = fixture_hp(12);
        m.handle(ZoneCommand::sync_player_combat_state(
            sid("a"),
            MirClass::Taoist,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        m.handle(ZoneCommand::PlayerCastMagicWithItem {
            session_id: sid("a"),
            object_id: MONSTER,
            spell: Spell::Poisoning,
            direction: MirDirection::Right,
            target: point(10, 10),
            cast: true,
            level: 3,
            damage: 5,
            mp_cost: 0,
            cooldown_ms: 0,
            item_param: 1,
            now_ms: 100,
        });
        admit(&mut m, "a", DEST);
        for (i, map) in ["cross-map-extra-1", "cross-map-extra-2"]
            .into_iter()
            .enumerate()
        {
            let mut j = join(&format!("extra-{i}"), map);
            j.object_id = 200 + i as u32;
            m.join(j);
        }
        m
    }
    let mut parallel = prepared();
    let mut sequential = prepared();
    for at in [2_100, 4_100] {
        assert!(awards(&parallel.tick_all(at)).is_empty());
        assert!(awards(&sequential.tick_all_sequential(at)).is_empty());
    }
    let p = parallel.tick_all(6_100);
    let s = sequential.tick_all_sequential(6_100);
    let pa = awards(&p);
    let sa = awards(&s);
    assert_eq!(pa.len(), 1);
    assert_eq!(sa.len(), 1);
    assert_eq!(pa[0].0, sa[0].0);
    assert_eq!(pa[0].1, sa[0].1);
    assert_eq!(pa[0].0, &sid("a"));
    for (m, out) in [(&parallel, &p), (&sequential, &s)] {
        let (sid, source, proof, award) = out
            .iter()
            .find_map(|o| match o {
                ZoneOutbound::OwnedMonsterKillAward {
                    session_id,
                    source,
                    online_owner,
                    award,
                } => Some((session_id, source, online_owner, award)),
                _ => None,
            })
            .unwrap();
        assert!(m.issued_monster_award_is_current(source, sid, proof, award));
    }
}
