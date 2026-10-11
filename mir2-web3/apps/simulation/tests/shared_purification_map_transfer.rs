//! Ordinary shared Manager admission and live map changes. Prepared actor
//! stats/maps are explicit; poison, Curse, buffs and Purification come only
//! from accepted native spell commands, never from edited checkpoints.
use mir2_protocol::{MirClass, MirDirection, MirGender, Point, ServerPacket, Spell};
use mir2_simulation::{
    SessionId, ZoneCommand, ZoneJoin, ZoneKey, ZoneManager, ZoneOutbound,
    ZoneRuntime,
};
use serde_json::Value;

// Distinct ordinary bundled maps, with the same logical actor spacing on each.
const MAP_A: &str = "D024";
const MAP_B: &str = "D022";
const MAP_C: &str = "D023";

fn map_point(map: &str, x: i32, y: i32) -> Point {
    let (dx, dy) = match map {
        MAP_A => (17, 41),
        MAP_B => (221, 183),
        MAP_C => (196, 158),
        _ => panic!("unknown prepared map"),
    };
    Point {
        x: x + dx,
        y: y + dy,
    }
}

fn ordinary_zone(map: &str) -> ZoneRuntime {
    let zone = ZoneRuntime::new(ZoneKey::for_map(map));
    assert!(zone.has_available_collision(), "ordinary terrain {map}");
    zone
}

fn sid(name: &str) -> SessionId {
    SessionId::new(name)
}

fn actor(name: &str, map: &str) -> ZoneJoin {
    let (object_id, x) = match name {
        "healer" => (101, 10),
        "friend" => (102, 12),
        "enemy" => (103, 14),
        "witness" => (104, 16),
        _ => panic!("unknown prepared actor"),
    };
    let mut join = ZoneJoin {
        session_id: sid(name),
        account_id: format!("purification-map-{name}"),
        character_index: 1,
        object_id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 50,
        hp: 1_000,
        max_hp: 1_000,
        mp: 1_000,
        map_file_name: map.into(),
        position: map_point(map, x, 10),
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    };
    if name == "enemy" {
        join.chat_profile.attack_mode = 5;
    }
    join
}

fn packets(out: &[ZoneOutbound], recipient: &str) -> Vec<ServerPacket> {
    out.iter()
        .flat_map(|message| match message {
            ZoneOutbound::ToSession {
                session_id,
                packets,
            } if session_id == &sid(recipient) => packets.clone(),
            ZoneOutbound::ToMany {
                session_ids,
                packets,
            } if session_ids.contains(&sid(recipient)) => packets.clone(),
            ZoneOutbound::ToAll { packets } => packets.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn player(manager: &ZoneManager, map: &str, name: &str) -> Value {
    let checkpoint: Value = serde_json::from_slice(
        &manager
            .zone(&ZoneKey::for_map(map))
            .unwrap()
            .checkpoint_bytes()
            .unwrap(),
    )
    .unwrap();
    checkpoint["players"][name].clone()
}

fn spell(
    manager: &mut ZoneManager,
    caster: &str,
    target: &str,
    kind: Spell,
    now: u64,
) -> Vec<ZoneOutbound> {
    let map = manager.zone_key_for_session(&sid(target)).unwrap();
    let target = actor(target, &map.map_file_name);
    manager.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid(caster),
        object_id: if kind == Spell::Curse {
            0
        } else {
            target.object_id
        },
        spell: kind,
        direction: MirDirection::Right,
        target: target.position,
        cast: true,
        level: 3,
        damage: if kind == Spell::Curse { 30 } else { 0 },
        mp_cost: 1,
        cooldown_ms: 500,
        now_ms: now,
    })
}

fn source_fixture() -> (ZoneManager, u64) {
    source_fixture_with_revived_actors(false)
}

fn source_fixture_with_revived_actors(revive: bool) -> (ZoneManager, u64) {
    let mut manager = ZoneManager::new();
    for map in [MAP_A, MAP_B, MAP_C] {
        assert!(manager.install_empty_zone(ordinary_zone(map)));
    }
    for name in ["healer", "friend", "enemy"] {
        manager.join(actor(name, MAP_A));
        if revive {
            change_life(&mut manager, name, true);
        }
    }
    let beneficial = spell(
        &mut manager,
        "healer",
        "friend",
        Spell::UltimateEnhancer,
        10,
    );
    assert!(packets(&beneficial, "friend").iter().any(|p| matches!(p,
        ServerPacket::AddBuff { buff } if buff.object_id == 102 && buff.buff_type == 9)));
    manager.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("enemy"),
        object_id: 102,
        spell: Spell::Poisoning,
        direction: MirDirection::Left,
        target: map_point(MAP_A, 12, 10),
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 1,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 20,
    });
    let poisoned = manager.tick_all(520);
    assert!(packets(&poisoned, "friend").iter().any(|p| matches!(p,
        ServerPacket::ObjectPoisoned { object_id: 102, poison } if *poison != 0)));
    for attempt in 0..20 {
        let now = 2_000 + attempt * 2_001;
        let cast = spell(&mut manager, "enemy", "friend", Spell::Curse, now);
        assert!(
            packets(&cast, "enemy").iter().any(|p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::Curse,
                    cast: true,
                    ..
                }
            )),
            "{cast:?}"
        );
        manager.tick_all(now + 500);
        if player(&manager, MAP_A, "friend")["buffs"]
            .get("12")
            .is_some()
        {
            assert_ne!(
                player(&manager, MAP_A, "friend")["poison"],
                0
            );
            return (manager, now + 520);
        }
    }
    panic!("normal Curse failed to land within bounded real attempts");
}

fn cast(manager: &mut ZoneManager, caster: &str, now: u64) {
    let out = spell(manager, caster, "friend", Spell::Purification, now);
    assert!(
        packets(&out, caster).iter().any(|p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::Purification,
                cast: true,
                ..
            }
        )),
        "{out:?}"
    );
}

fn transfer(manager: &mut ZoneManager, name: &str, destination: &str) {
    let current = manager.player_vitals(&sid(name)).unwrap();
    let mut join = actor(name, destination);
    join.hp = current.0;
    join.mp = current.2;
    manager.join(join);
}

fn assert_curse_and_poison(manager: &ZoneManager, map: &str) {
    let target = player(manager, map, "friend");
    assert!(
        target["buffs"].get("12").is_some(),
        "real Curse vanished before completion: {target}"
    );
    assert!(
        target["buffs"].get("9").is_some(),
        "beneficial buff vanished: {target}"
    );
    assert_ne!(
        target["poison"], 0,
        "real poison vanished before completion"
    );
}

fn assert_cleared_once(manager: &mut ZoneManager, map: &str, now: u64) {
    let out = manager.tick_all(now + 500);
    let target = player(manager, map, "friend");
    assert_eq!(target["poison"], 0);
    assert!(target["buffs"].get("12").is_none());
    assert!(target["buffs"].get("9").is_some());
    assert_eq!(
        packets(&out, "friend")
            .iter()
            .filter(|p| matches!(
                p,
                ServerPacket::RemoveBuff {
                    object_id: 102,
                    buff_type: 12
                }
            ))
            .count(),
        1,
        "{out:?}"
    );
    assert!(!packets(&manager.tick_all(now + 501), "friend")
        .iter()
        .any(|p| matches!(
            p,
            ServerPacket::RemoveBuff {
                object_id: 102,
                buff_type: 12
            }
        )));
}

#[test]
fn normal_map_join_preserves_real_hiding_flag_projection_and_original_expiry() {
    for kind in [Spell::Hiding, Spell::MassHiding] {
        let mut manager = ZoneManager::new();
        for map in [MAP_A, MAP_B] {
            assert!(manager.install_empty_zone(ordinary_zone(map)));
        }
        manager.join(actor("healer", MAP_A));
        manager.join(actor("witness", MAP_B));
        let accepted = spell(&mut manager, "healer", "healer", kind, 10);
        assert!(packets(&accepted, "healer").iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic { spell, cast: true, .. } if *spell == kind
        )));
        manager.tick_all(510);
        let before = player(&manager, MAP_A, "healer");
        assert_eq!(
            before["hidden"], true,
            "{kind:?} did not actually hide the actor"
        );
        let expiry = before["buffs"]["2"]["expires_at_ms"].as_u64().unwrap();
        let proof = manager
            .online_owner_proof_for_session(&sid("healer"))
            .unwrap();
        let vitals = manager.player_vitals(&sid("healer")).unwrap();
        let mut join = actor("healer", MAP_B);
        join.hp = vitals.0;
        join.mp = vitals.2;
        let moved = manager.join(join);
        let after = player(&manager, MAP_B, "healer");
        assert_eq!(
            after["hidden"], true,
            "live map Join lost the actual {kind:?} flag"
        );
        assert_eq!(after["buffs"], before["buffs"]);
        assert_eq!(
            manager
                .online_owner_proof_for_session(&sid("healer"))
                .as_ref(),
            Some(&proof)
        );
        assert!(
            packets(&moved, "witness").iter().any(|packet| matches!(
            packet,
            ServerPacket::ObjectPlayer { info }
                if info.object_id == 101 && info.hidden
            )),
            "first destination AOI projection lost {kind:?}: {moved:?}"
        );
        manager.tick_all(expiry - 1);
        assert_eq!(
            player(&manager, MAP_B, "healer")["hidden"],
            true
        );
        let expired = manager.tick_all(expiry);
        assert_eq!(
            player(&manager, MAP_B, "healer")["hidden"],
            false
        );
        assert!(player(&manager, MAP_B, "healer")["buffs"]
            .get("2")
            .is_none());
        assert_eq!(
            packets(&expired, "witness")
                .iter()
                .filter(|packet| matches!(
                    packet,
                    ServerPacket::ObjectHidden {
                        object_id: 101,
                        hidden: false
                    }
                ))
                .count(),
            1,
            "{expired:?}"
        );
        assert!(!packets(&manager.tick_all(expiry + 1), "witness")
            .iter()
            .any(|packet| matches!(
                packet,
                ServerPacket::ObjectHidden {
                    object_id: 101,
                    hidden: false
                }
            )));
    }
}

#[test]
fn normal_map_join_preserves_source_buffs_and_their_absolute_expiry() {
    let (mut manager, _) = source_fixture();
    let before = player(&manager, MAP_A, "friend")["buffs"].clone();
    let proof = manager
        .online_owner_proof_for_session(&sid("friend"))
        .unwrap();
    transfer(&mut manager, "friend", MAP_B);
    assert_eq!(
        manager
            .online_owner_proof_for_session(&sid("friend"))
            .as_ref(),
        Some(&proof)
    );
    assert_eq!(
        player(&manager, MAP_B, "friend")["buffs"],
        before,
        "normal same-Node map Join must carry actual buff state, never renew its expiry"
    );
}

#[test]
fn normal_purification_both_map_transfers_preserve_original_actors_and_500_deadline() {
    for (order, revive) in [
        (["healer", "friend"], false),
        (["friend", "healer"], false),
        (["healer", "friend"], true),
        (["friend", "healer"], true),
    ] {
        let (mut manager, now) = source_fixture_with_revived_actors(revive);
        let proofs = order.map(|name| manager.online_owner_proof_for_session(&sid(name)).unwrap());
        let lives = order.map(|name| manager.player_life_generation(&sid(name)).unwrap());
        cast(&mut manager, "healer", now);
        for name in order {
            transfer(&mut manager, name, MAP_B);
        }
        for (index, name) in order.into_iter().enumerate() {
            assert_eq!(
                manager.online_owner_proof_for_session(&sid(name)).as_ref(),
                Some(&proofs[index])
            );
            assert_eq!(
                manager.player_life_generation(&sid(name)),
                Some(lives[index])
            );
        }
        manager.tick_all(now + 499);
        assert_curse_and_poison(&manager, MAP_B);
        assert_cleared_once(&mut manager, MAP_B, now);
    }
}

fn change_life(manager: &mut ZoneManager, name: &str, revive: bool) {
    manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid(name),
        hp: 0,
        max_hp: 1_000,
        mp: 1_000,
        dead: true,
    });
    if revive {
        manager.handle(ZoneCommand::SyncPlayerVitalsAndLife {
            session_id: sid(name),
            hp: 1_000,
            max_hp: 1_000,
            mp: 1_000,
            dead: false,
        });
    }
}

fn no_purification_completion(out: &[ZoneOutbound]) {
    assert!(
        !packets(out, "friend").iter().any(|p| matches!(
            p,
            ServerPacket::RemoveBuff {
                object_id: 102,
                buff_type: 12
            } | ServerPacket::ObjectPoisoned {
                object_id: 102,
                poison: 0
            }
        )),
        "{out:?}"
    );
}

#[test]
fn normal_self_purification_follows_its_single_live_actor_to_the_destination() {
    let (mut manager, now) = source_fixture();
    cast(&mut manager, "friend", now);
    transfer(&mut manager, "friend", MAP_B);
    manager.tick_all(now + 499);
    assert_curse_and_poison(&manager, MAP_B);
    assert_cleared_once(&mut manager, MAP_B, now);
}

#[test]
fn normal_purification_repeated_map_hops_keep_one_original_deadline_and_destination_audience() {
    let (mut manager, now) = source_fixture();
    cast(&mut manager, "healer", now);
    for name in ["healer", "friend"] {
        transfer(&mut manager, name, MAP_B);
    }
    manager.tick_all(now + 200);
    manager.join(actor("witness", MAP_C));
    for name in ["friend", "healer"] {
        transfer(&mut manager, name, MAP_C);
    }
    manager.tick_all(now + 499);
    assert_curse_and_poison(&manager, MAP_C);
    let out = manager.tick_all(now + 500);
    assert_eq!(
        packets(&out, "witness")
            .iter()
            .filter(|p| matches!(
                p,
                ServerPacket::RemoveBuff {
                    object_id: 102,
                    buff_type: 12
                }
            ))
            .count(),
        1
    );
    assert!(!packets(&out, "enemy").iter().any(|p| matches!(
        p,
        ServerPacket::RemoveBuff { object_id: 102, .. }
            | ServerPacket::ObjectPoisoned {
                object_id: 102,
                poison: 0
            }
    )));
    assert!(player(&manager, MAP_C, "friend")["buffs"]
        .get("12")
        .is_none());
    no_purification_completion(&manager.tick_all(now + 501));
}

#[test]
fn normal_map_buffs_expire_at_the_source_absolute_deadline_without_renewal() {
    let (mut manager, _) = source_fixture();
    let deadline = player(&manager, MAP_A, "friend")["buffs"]["12"]["expires_at_ms"]
        .as_u64()
        .unwrap();
    transfer(&mut manager, "friend", MAP_B);
    manager.tick_all(deadline - 1);
    assert!(player(&manager, MAP_B, "friend")["buffs"]
        .get("12")
        .is_some());
    let expired = manager.tick_all(deadline);
    assert!(player(&manager, MAP_B, "friend")["buffs"]
        .get("12")
        .is_none());
    assert_eq!(
        packets(&expired, "friend")
            .iter()
            .filter(|p| matches!(
                p,
                ServerPacket::RemoveBuff {
                    object_id: 102,
                    buff_type: 12
                }
            ))
            .count(),
        1
    );
}

#[test]
fn normal_purification_one_actor_or_different_destination_cannot_complete() {
    for case in ["caster_only", "target_only", "different_destinations"] {
        let (mut manager, now) = source_fixture();
        cast(&mut manager, "healer", now);
        if case != "target_only" {
            transfer(&mut manager, "healer", MAP_B);
        }
        if case != "caster_only" {
            transfer(
                &mut manager,
                "friend",
                if case == "different_destinations" {
                    MAP_C
                } else {
                    MAP_B
                },
            );
        }
        no_purification_completion(&manager.tick_all(now + 500));
        let target_map = if case == "caster_only" {
            MAP_A
        } else if case == "different_destinations" {
            MAP_C
        } else {
            MAP_B
        };
        assert_curse_and_poison(&manager, target_map);
    }
}

#[test]
fn normal_purification_target_arriving_after_completion_cannot_resurrect_the_action() {
    let (mut manager, now) = source_fixture();
    cast(&mut manager, "healer", now);
    transfer(&mut manager, "healer", MAP_B);
    no_purification_completion(&manager.tick_all(now + 500));
    transfer(&mut manager, "friend", MAP_B);
    no_purification_completion(&manager.tick_all(now + 501));
    assert_curse_and_poison(&manager, MAP_B);
}

#[test]
fn normal_purification_map_handoff_rejects_current_actor_death_or_new_life() {
    for name in ["healer", "friend"] {
        for revive in [false, true] {
            for before_transfer in [false, true] {
                let (mut manager, now) = source_fixture();
                cast(&mut manager, "healer", now);
                let old_life = manager.player_life_generation(&sid(name)).unwrap();
                if before_transfer {
                    change_life(&mut manager, name, revive);
                }
                for actor in ["healer", "friend"] {
                    transfer(&mut manager, actor, MAP_B);
                }
                if !before_transfer {
                    change_life(&mut manager, name, revive);
                }
                no_purification_completion(&manager.tick_all(now + 500));
                if revive {
                    assert!(manager.player_life_generation(&sid(name)).unwrap() > old_life);
                }
                if name == "healer" {
                    assert_curse_and_poison(&manager, MAP_B);
                }
            }
        }
    }
}

#[test]
fn normal_purification_logout_relogin_and_nonretained_join_never_adopt_old_actions() {
    for name in ["healer", "friend"] {
        for leave in [false, true] {
            let (mut manager, now) = source_fixture();
            cast(&mut manager, "healer", now);
            let old_proof = manager.online_owner_proof_for_session(&sid(name)).unwrap();
            if leave {
                manager.handle(ZoneCommand::Leave {
                    session_id: sid(name),
                });
                manager.join(actor(name, MAP_B));
            } else {
                // Same-map Join is a new admission, never an online transfer.
                manager.join(actor(name, MAP_A));
                transfer(&mut manager, name, MAP_B);
            }
            transfer(
                &mut manager,
                if name == "healer" { "friend" } else { "healer" },
                MAP_B,
            );
            assert!(!manager.online_owner_proof_is_current(&old_proof));
            no_purification_completion(&manager.tick_all(now + 500));
            if name == "healer" {
                assert_curse_and_poison(&manager, MAP_B);
            }
        }
    }
}

#[test]
fn normal_purification_cold_manager_restore_never_rebinds_old_pending_actions() {
    let (mut manager, now) = source_fixture();
    cast(&mut manager, "healer", now);
    for name in ["healer", "friend"] {
        transfer(&mut manager, name, MAP_B);
    }
    let old_proof = manager
        .online_owner_proof_for_session(&sid("healer"))
        .unwrap();
    let checkpoint = manager.checkpoint_bytes().unwrap();
    let mut cold = ZoneManager::restore_checkpoint(&checkpoint).unwrap();
    assert!(!cold.online_owner_proof_is_current(&old_proof));
    no_purification_completion(&cold.tick_all(now + 500));
    assert!(player(&cold, MAP_B, "friend")["buffs"]
        .get("12")
        .is_some());
    for name in ["healer", "friend"] {
        cold.join(actor(name, MAP_B));
    }
    no_purification_completion(&cold.tick_all(now + 501));
}

#[test]
fn normal_purification_destination_object_id_conflict_never_retags_the_original_actor() {
    let (mut manager, now) = source_fixture();
    cast(&mut manager, "healer", now);
    let mut conflict = actor("witness", MAP_B);
    conflict.object_id = 101;
    manager.join(conflict);
    for name in ["healer", "friend"] {
        transfer(&mut manager, name, MAP_B);
    }
    assert_ne!(
        player(&manager, MAP_B, "healer")["object_id"],
        101
    );
    no_purification_completion(&manager.tick_all(now + 500));
    assert_curse_and_poison(&manager, MAP_B);
}

#[test]
fn normal_purification_destination_uses_current_friendship_at_completion() {
    for friendly_at_completion in [false, true] {
        let (mut manager, now) = source_fixture();
        cast(&mut manager, "healer", now);
        for name in ["healer", "friend"] {
            transfer(&mut manager, name, MAP_B);
        }
        let mut hostile = actor("healer", MAP_B).chat_profile;
        hostile.attack_mode = 5;
        manager.handle(ZoneCommand::UpdateChatProfile {
            session_id: sid("healer"),
            profile: hostile,
        });
        if friendly_at_completion {
            manager.handle(ZoneCommand::UpdateChatProfile {
                session_id: sid("healer"),
                profile: actor("healer", MAP_B).chat_profile,
            });
            assert_cleared_once(&mut manager, MAP_B, now);
        } else {
            no_purification_completion(&manager.tick_all(now + 500));
            assert_curse_and_poison(&manager, MAP_B);
        }
    }
}

#[test]
fn normal_walk_and_run_cannot_shortcut_the_original_600ms_cast_action_clock() {
    for run in [false, true] {
        let (mut manager, now) = source_fixture();
        cast(&mut manager, "healer", now);
        let before = manager.player_transform(&sid("healer")).unwrap();
        let command = if run {
            ZoneCommand::Run {
                session_id: sid("healer"),
                direction: MirDirection::Up,
                seq: 1,
                now_ms: now + 499,
            }
        } else {
            ZoneCommand::Walk {
                session_id: sid("healer"),
                direction: MirDirection::Up,
                seq: 1,
                now_ms: now + 499,
            }
        };
        manager.handle(command);
        manager.tick_all(now + 499);
        assert_eq!(
            manager.player_transform(&sid("healer")),
            Some(before.clone())
        );
        assert_cleared_once(&mut manager, MAP_A, now);
        assert_eq!(manager.player_transform(&sid("healer")), Some(before));
        assert_eq!(
            player(&manager, MAP_A, "healer")["movement_ready_at_ms"],
            now + 600
        );
    }
}
