use super::*;
use mir2_protocol::MirGender;

fn sid(name: &str) -> SessionId {
    SessionId::new(name)
}

fn join(name: &str, object_id: u32, x: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: sid(name),
        account_id: format!("purification-{name}"),
        character_index: 1,
        object_id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 50,
        hp: 1_000,
        max_hp: 1_000,
        mp: 1_000,
        map_file_name: "purification-source".into(),
        position: Point { x, y: 10 },
        direction: MirDirection::Right,
        chat_profile: Default::default(),
        combat_stats: Default::default(),
    }
}

fn contains(outbounds: &[ZoneOutbound], check: impl Fn(&ServerPacket) -> bool) -> bool {
    outbounds.iter().any(|outbound| match outbound {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.iter().any(&check),
        _ => false,
    })
}

fn cleared(outbounds: &[ZoneOutbound]) -> bool {
    contains(outbounds, |packet| {
        matches!(
            packet,
            ServerPacket::ObjectPoisoned {
                object_id: 102,
                poison: 0
            } | ServerPacket::RemoveBuff { object_id: 102, .. }
        )
    })
}

fn cast(zone: &mut ZoneRuntime, now: u64, level: u8) -> Vec<ZoneOutbound> {
    let out = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid("healer"),
        object_id: 102,
        spell: Spell::Purification,
        direction: MirDirection::Right,
        target: zone.players[&sid("friend")].position.clone(),
        cast: true,
        level,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: now,
    });
    assert!(
        contains(&out, |p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::Purification,
                cast: true,
                ..
            }
        )),
        "ordinary Purification was not admitted: {out:?}"
    );
    out
}

fn poison_targets(zone: &ZoneRuntime) -> Vec<serde_json::Value> {
    let checkpoint: serde_json::Value =
        serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    let store = checkpoint["entity_combat"]["owned_pet"]
        .as_object()
        .expect("normal Poisoning created an owned-human store");
    // The existing checkpoint omits an empty poison vector.
    store.get("poisons").map_or_else(Vec::new, |poisons| {
        poisons
            .as_array()
            .expect("a present poison lease collection must be an array")
            .iter()
            .map(|poison| poison["target"].clone())
            .collect()
    })
}

fn self_cast(zone: &mut ZoneRuntime, now: u64) -> Vec<ZoneOutbound> {
    zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid("friend"),
        object_id: 102,
        spell: Spell::Purification,
        direction: MirDirection::Left,
        target: Point { x: 12, y: 10 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 7,
        cooldown_ms: 1_000,
        now_ms: now,
    })
}

// Poison, Curse and the beneficial buff all come from accepted ordinary Zone
// spell commands. No private poison, permission or pending proof is assigned.
fn source_fixture() -> (ZoneRuntime, u64) {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("purification-source"),
        ZoneCollision::unbounded(),
    );
    zone.handle(ZoneCommand::Join(join("healer", 101, 10)));
    zone.handle(ZoneCommand::Join(join("friend", 102, 12)));
    let mut enemy = join("enemy", 103, 14);
    enemy.chat_profile.attack_mode = 5;
    zone.handle(ZoneCommand::Join(enemy));
    let beneficial = zone.handle(ZoneCommand::PlayerCastMagic {
        session_id: sid("healer"),
        object_id: 102,
        spell: Spell::UltimateEnhancer,
        direction: MirDirection::Right,
        target: Point { x: 12, y: 10 },
        cast: true,
        level: 3,
        damage: 0,
        mp_cost: 1,
        cooldown_ms: 1_000,
        now_ms: 10,
    });
    assert!(contains(&beneficial, |p| matches!(p,
        ServerPacket::AddBuff { buff } if buff.object_id == 102 && buff.buff_type == 9)));
    let poison = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("enemy"),
        object_id: 102,
        spell: Spell::Poisoning,
        direction: MirDirection::Left,
        target: Point { x: 12, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 1,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: 20,
    });
    assert!(!contains(&poison, |p| matches!(
        p,
        ServerPacket::ObjectPoisoned { .. }
    )));
    let poison = zone.tick(520);
    assert!(contains(&poison, |p| matches!(p,
        ServerPacket::ObjectPoisoned { object_id: 102, poison } if *poison != 0)));
    // Curse has its original chance, so use bounded real casts and clocks.
    // Every miss is retained as gameplay; the fixture never assigns a result.
    for attempt in 0..20 {
        let now = 2_000 + attempt * 2_001;
        let curse = zone.handle(ZoneCommand::PlayerCastMagic {
            session_id: sid("enemy"),
            object_id: 0,
            spell: Spell::Curse,
            direction: MirDirection::Left,
            target: Point { x: 12, y: 10 },
            cast: true,
            level: 3,
            damage: 30,
            mp_cost: 1,
            cooldown_ms: 500,
            now_ms: now,
        });
        assert!(
            contains(&curse, |p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::Curse,
                    cast: true,
                    ..
                }
            )),
            "normal Curse cast at {now} was not admitted: {curse:?}"
        );
        let curse = zone.tick(now + 500);
        if contains(&curse, |p| {
            matches!(p,
            ServerPacket::AddBuff { buff } if buff.object_id == 102 && buff.buff_type == 12)
        }) {
            assert_ne!(zone.players[&sid("friend")].poison, 0);
            return (zone, now + 520);
        }
    }
    panic!("ordinary Curse did not land within the bounded source fixture");
}

#[test]
fn purification_normal_source_resolves_at_500_once_and_stops_poison() {
    let (mut zone, now) = source_fixture();
    let target =
        serde_json::to_value(zone.native_entity_player_ref(&sid("friend")).unwrap()).unwrap();
    assert_eq!(poison_targets(&zone), vec![target]);
    let cast = cast(&mut zone, now, 3);
    assert!(contains(&cast, |p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::Purification,
            cast: true,
            ..
        }
    )));
    assert!(!cleared(&cast));
    let before = zone.tick(now + 499);
    assert!(!cleared(&before));
    assert_ne!(zone.players[&sid("friend")].poison, 0);
    assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
    let completed = zone.tick(now + 500);
    assert!(cleared(&completed));
    assert!(contains(&completed, |p| matches!(
        p,
        ServerPacket::RemoveBuff {
            object_id: 102,
            buff_type: 12
        }
    )));
    assert_eq!(zone.players[&sid("friend")].poison, 0);
    assert!(zone.players[&sid("friend")]
        .native_status_poison_deadlines
        .is_empty());
    assert!(zone.players[&sid("friend")].buffs.contains_key(&9));
    assert!(
        poison_targets(&zone).is_empty(),
        "completion removes the actual lease immediately"
    );
    let mut fork = zone.transaction_fork();
    assert!(poison_targets(&fork).is_empty());
    let fork_hp = fork.players[&sid("friend")].hp;
    assert!(!fork.tick(now + 6_000).iter().any(|o| matches!(o,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &sid("friend"))));
    assert!(
        fork.players[&sid("friend")].hp >= fork_hp,
        "ordinary regeneration may heal; poison cannot damage"
    );
    assert!(poison_targets(&fork).is_empty());
    let mut cold = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
    assert!(poison_targets(&cold).is_empty());
    let cold_hp = cold.players[&sid("friend")].hp;
    assert!(!cold.tick(now + 6_000).iter().any(|o| matches!(o,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &sid("friend"))));
    assert!(
        cold.players[&sid("friend")].hp >= cold_hp,
        "ordinary regeneration may heal; poison cannot damage"
    );
    assert!(poison_targets(&cold).is_empty());
    assert!(
        !cleared(&zone.tick(now + 500)),
        "completion cannot run twice"
    );
    let hp = zone.players[&sid("friend")].hp;
    let later = zone.tick(now + 6_000);
    assert!(!later.iter().any(|o| matches!(o,
        ZoneOutbound::PlayerDamaged { session_id, .. } if session_id == &sid("friend"))));
    assert!(
        zone.players[&sid("friend")].hp >= hp,
        "ordinary regeneration may heal; poison cannot damage"
    );
    let late = zone.handle(ZoneCommand::Join(join("observer", 104, 13)));
    assert!(contains(&late, |p| matches!(p,
        ServerPacket::ObjectPlayer { info }
            if info.object_id == 102 && info.poison == 0
                && !info.buffs.contains(&12) && info.buffs.contains(&9))));
}

#[test]
fn purification_clears_every_source_debuff_and_keeps_other_projections() {
    let (mut zone, now) = source_fixture();
    // Rhino/Blindness are prepared trusted server buff projections, not a claim
    // that their monster producer was exercised by this player-spell fixture.
    let kinds = [53, 57, 4, 50, 107];
    zone.handle(ZoneCommand::BroadcastPackets {
        session_id: sid("friend"),
        owner_local_object_id: 1_000,
        packets: kinds
            .into_iter()
            .map(|kind| ServerPacket::AddBuff {
                buff: ClientBuff {
                    buff_type: kind,
                    visible: true,
                    object_id: 1_000,
                    expire_time: 30_000,
                    infinite: false,
                    paused: false,
                    stats: Vec::new(),
                    values: Vec::new(),
                },
            })
            .collect(),
        now_ms: now,
    });
    assert!(!cleared(&cast(&mut zone, now, 3)));
    let completion = zone.tick(now + 500);
    for kind in [12, 53, 57] {
        assert!(contains(&completion, |p| matches!(p,
            ServerPacket::RemoveBuff { object_id: 102, buff_type } if *buff_type == kind)));
        assert!(!zone.players[&sid("friend")].buffs.contains_key(&kind));
    }
    for kind in [9, 4, 50, 107] {
        assert!(zone.players[&sid("friend")].buffs.contains_key(&kind));
        assert!(!contains(&completion, |p| matches!(p,
            ServerPacket::RemoveBuff { object_id: 102, buff_type } if *buff_type == kind)));
    }
}

#[test]
fn purification_completion_rechecks_current_friendship() {
    for change in ["all", "group", "guild", "war", "red"] {
        let (mut zone, now) = source_fixture();
        assert!(!cleared(&cast(&mut zone, now, 3)));
        let mut healer = zone.players[&sid("healer")].chat_profile.clone();
        let mut friend = zone.players[&sid("friend")].chat_profile.clone();
        match change {
            "all" => healer.attack_mode = 5,
            "group" => healer.attack_mode = 1,
            "guild" => {
                healer.attack_mode = 2;
                healer.guild_name = Some("A".into());
                friend.guild_name = Some("B".into());
            }
            "war" => {
                healer.attack_mode = 3;
                healer.guild_name = Some("A".into());
                friend.guild_name = Some("B".into());
                friend.active_guild_wars.push("A".into());
            }
            "red" => {
                healer.attack_mode = 4;
                friend.pk_points = 200;
            }
            _ => unreachable!(),
        }
        zone.handle(ZoneCommand::UpdateChatProfile {
            session_id: sid("healer"),
            profile: healer,
        });
        zone.handle(ZoneCommand::UpdateChatProfile {
            session_id: sid("friend"),
            profile: friend,
        });
        let completion = zone.tick(now + 500);
        assert!(!cleared(&completion), "changed relationship: {change}");
        assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
        assert_ne!(zone.players[&sid("friend")].poison, 0);
    }
}

#[test]
fn purification_rejects_death_revive_and_reused_session_nodes() {
    for actor in ["healer", "friend"] {
        for transition in ["dead", "revive", "logout", "rejoin"] {
            let (mut zone, now) = source_fixture();
            assert!(!cleared(&cast(&mut zone, now, 3)));
            if matches!(transition, "dead" | "revive") {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: sid(actor),
                    hp: 0,
                    max_hp: 1_000,
                    mp: 1_000,
                    dead: true,
                });
                if transition == "revive" {
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: sid(actor),
                        hp: 1_000,
                        max_hp: 1_000,
                        mp: 1_000,
                        dead: false,
                    });
                }
            } else {
                zone.handle(ZoneCommand::Leave {
                    session_id: sid(actor),
                });
                if transition == "rejoin" {
                    zone.handle(ZoneCommand::Join(join(
                        actor,
                        if actor == "healer" { 101 } else { 102 },
                        if actor == "healer" { 10 } else { 12 },
                    )));
                }
            }
            assert!(!cleared(&zone.tick(now + 500)), "{actor} {transition}");
        }
    }
}

#[test]
fn purification_rejects_missing_nodes_and_changed_owner_map() {
    for actor in ["healer", "friend"] {
        for change in ["missing", "map", "life"] {
            let (mut zone, now) = source_fixture();
            assert!(!cleared(&cast(&mut zone, now, 3)));
            let mut presence = zone.online_presence.clone();
            match change {
                "map" => presence.get_mut(&sid(actor)).unwrap().key = ZoneKey::for_map("other-map"),
                "life" => presence.get_mut(&sid(actor)).unwrap().life_generation += 1,
                "missing" => {
                    presence.remove(&sid(actor));
                }
                _ => unreachable!(),
            }
            // The existing manager-facing trusted API updates global Nodes.
            // No positive authority or permission is manufactured here.
            zone.ingest_online_presence(presence, true);
            assert!(!cleared(&zone.tick(now + 500)), "{actor} Node={change}");
            assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
        }
    }
}

#[test]
fn purification_cleanup_does_not_remove_a_new_life_poison_with_the_same_object_id() {
    let (mut zone, now) = source_fixture();
    let old = zone.native_entity_player_ref(&sid("friend")).unwrap();
    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("friend"),
        hp: 0,
        max_hp: 1_000,
        mp: 1_000,
        dead: true,
    });
    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
        session_id: sid("friend"),
        hp: 1_000,
        max_hp: 1_000,
        mp: 1_000,
        dead: false,
    });
    let current = zone.native_entity_player_ref(&sid("friend")).unwrap();
    assert_eq!(old.object_id(), current.object_id());
    assert_ne!(old, current);
    let poison = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("enemy"),
        object_id: 102,
        spell: Spell::Poisoning,
        direction: MirDirection::Left,
        target: Point { x: 12, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 1,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: now + 2_000,
    });
    assert!(contains(&poison, |p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::Poisoning,
            cast: true,
            ..
        }
    )));
    let applied = zone.tick(now + 2_500);
    assert!(contains(&applied, |p| matches!(p,
        ServerPacket::ObjectPoisoned { object_id: 102, poison } if *poison != 0)));
    let current_json = serde_json::to_value(&current).unwrap();
    assert_eq!(poison_targets(&zone), vec![current_json.clone()]);
    zone.clear_owned_human_player_poison_leases(&old);
    assert_eq!(poison_targets(&zone), vec![current_json]);
    zone.clear_owned_human_player_poison_leases(&current);
    assert!(poison_targets(&zone).is_empty());
}

#[test]
fn purification_cold_decode_cannot_rebind_the_old_cast() {
    let (mut zone, now) = source_fixture();
    assert!(!cleared(&cast(&mut zone, now, 3)));
    let checkpoint = zone.checkpoint_bytes().unwrap();
    let mut cold = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    assert!(cold.pending_native_purifications.is_empty());
    assert!(cold.online_presence.is_empty());
    assert!(!cleared(&cold.tick(now + 500)));
    assert!(cold.players[&sid("friend")].buffs.contains_key(&12));
    for (actor, id, x) in [("healer", 101, 10), ("friend", 102, 12), ("enemy", 103, 14)] {
        cold.handle(ZoneCommand::Leave {
            session_id: sid(actor),
        });
        cold.handle(ZoneCommand::Join(join(actor, id, x)));
    }
    assert!(!cleared(&cold.tick(now + 500)));
}

#[test]
fn purification_transaction_fork_preserves_live_cast_without_mutating_original() {
    let (mut zone, now) = source_fixture();
    assert!(!cleared(&cast(&mut zone, now, 3)));
    let mut fork = zone.transaction_fork();
    assert!(!cleared(&fork.tick(now + 499)));
    assert!(cleared(&fork.tick(now + 500)));
    assert!(poison_targets(&fork).is_empty());
    assert_ne!(zone.players[&sid("friend")].poison, 0);
    assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
    assert_eq!(zone.pending_native_purifications.len(), 1);
    assert!(!poison_targets(&zone).is_empty());
    assert!(!cleared(&zone.tick(now + 499)));
    assert!(cleared(&zone.tick(now + 500)));
    assert!(poison_targets(&zone).is_empty());
}

#[test]
fn player_magic_item_preflight_admits_the_real_hostile_player_target() {
    let (mut zone, now) = source_fixture();
    let now = now + 2_000;
    assert!(zone.can_player_cast_magic(
        &sid("enemy"),
        102,
        Spell::Poisoning,
        MirDirection::Left,
        &Point { x: 12, y: 10 },
        true,
        10,
        5,
        500,
        now,
    ));
    let before_mp = zone.players[&sid("enemy")].mp;
    let cast = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("enemy"),
        object_id: 102,
        spell: Spell::Poisoning,
        direction: MirDirection::Left,
        target: Point { x: 12, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 5,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: now,
    });
    assert!(contains(&cast, |packet| matches!(
        packet,
        ServerPacket::Magic {
            spell: Spell::Poisoning,
            cast: true,
            ..
        }
    )));
    assert_eq!(zone.players[&sid("enemy")].mp, before_mp - 5);
}

#[test]
fn player_magic_item_preflight_and_dispatch_recheck_the_same_live_rejections() {
    for rejected in [
        "peace",
        "group",
        "guild",
        "safe_caster",
        "safe_target",
        "dead_caster",
        "dead_target",
        "point",
        "range",
        "mp",
        "spell_clock",
        "action_clock",
        "movement_clock",
        "status",
        "hallucination",
    ] {
        let (mut zone, now) = source_fixture();
        let now = now + 2_000;
        let mut point = Point { x: 12, y: 10 };
        let mut spell = Spell::Poisoning;
        match rejected {
            "peace" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .chat_profile
                    .attack_mode = 0
            }
            "group" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .chat_profile
                    .attack_mode = 1;
                zone.players
                    .get_mut(&sid("friend"))
                    .unwrap()
                    .chat_profile
                    .group_members = vec!["enemy".into()];
            }
            "guild" => {
                let enemy = zone.players.get_mut(&sid("enemy")).unwrap();
                enemy.chat_profile.attack_mode = 2;
                enemy.chat_profile.guild_name = Some("same-source-guild".into());
                zone.players
                    .get_mut(&sid("friend"))
                    .unwrap()
                    .chat_profile
                    .guild_name = Some("same-source-guild".into());
            }
            "safe_caster" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .chat_profile
                    .in_safe_zone = true
            }
            "safe_target" => {
                zone.players
                    .get_mut(&sid("friend"))
                    .unwrap()
                    .chat_profile
                    .in_safe_zone = true
            }
            "dead_caster" | "dead_target" => {
                let name = if rejected == "dead_caster" {
                    "enemy"
                } else {
                    "friend"
                };
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: sid(name),
                    hp: 0,
                    max_hp: 1_000,
                    mp: 1_000,
                    dead: true,
                });
            }
            "point" => point.x += 1,
            "range" => {
                point.x = 80;
                zone.handle(ZoneCommand::SyncPlayerTransform {
                    session_id: sid("friend"),
                    position: point.clone(),
                    direction: MirDirection::Left,
                });
            }
            "mp" => zone.players.get_mut(&sid("enemy")).unwrap().mp = 0,
            "spell_clock" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .magic_ready_at_ms
                    .insert(Spell::Poisoning as u8, now + 1);
            }
            "action_clock" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .next_spell_ready_at_ms = now + 1
            }
            "movement_clock" => {
                zone.players
                    .get_mut(&sid("enemy"))
                    .unwrap()
                    .movement_ready_at_ms = now + 1
            }
            "status" => {
                let enemy = zone.players.get_mut(&sid("enemy")).unwrap();
                enemy.native_status_poison |= CRYSTAL_POISON_PARALYSIS;
                enemy.native_status_poison_expires_at_ms = Some(now + 1_000);
            }
            "hallucination" => spell = Spell::Hallucination,
            _ => unreachable!(),
        }
        assert!(
            !zone.can_player_cast_magic(
                &sid("enemy"),
                102,
                spell,
                MirDirection::Left,
                &point,
                true,
                10,
                5,
                500,
                now,
            ),
            "{rejected}"
        );
        let before_mp = zone.players[&sid("enemy")].mp;
        let before_clock = zone.players[&sid("enemy")].magic_ready_at_ms.clone();
        let cast = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
            session_id: sid("enemy"),
            object_id: 102,
            spell,
            direction: MirDirection::Left,
            target: point,
            cast: true,
            level: 3,
            damage: 10,
            mp_cost: 5,
            cooldown_ms: 500,
            item_param: 1,
            now_ms: now,
        });
        assert!(
            !contains(&cast, |p| matches!(
                p,
                ServerPacket::Magic { cast: true, .. }
                    | ServerPacket::ObjectMagic { cast: true, .. }
            )),
            "{rejected}"
        );
        assert_eq!(zone.players[&sid("enemy")].mp, before_mp, "{rejected}");
        assert_eq!(
            zone.players[&sid("enemy")].magic_ready_at_ms,
            before_clock,
            "{rejected}"
        );
        if matches!(
            rejected,
            "mp" | "spell_clock" | "action_clock" | "movement_clock" | "status"
        ) {
            assert!(
                contains(&cast, |p| matches!(p, ServerPacket::UserLocation { .. })),
                "source readiness rejection keeps owner correction: {rejected}: {cast:?}"
            );
        }
    }
}

#[test]
fn player_magic_item_preflight_is_read_only_and_never_grants_a_changed_target() {
    let (mut zone, now) = source_fixture();
    let now = now + 2_000;
    let before = zone.checkpoint_bytes().unwrap();
    assert!(zone.can_player_cast_magic(
        &sid("enemy"),
        102,
        Spell::Poisoning,
        MirDirection::Left,
        &Point { x: 12, y: 10 },
        true,
        10,
        5,
        500,
        now,
    ));
    assert_eq!(zone.checkpoint_bytes().unwrap(), before);
    zone.players
        .get_mut(&sid("friend"))
        .unwrap()
        .chat_profile
        .in_safe_zone = true;
    let before_mp = zone.players[&sid("enemy")].mp;
    let cast = zone.player_cast_native_player_magic(
        &sid("enemy"),
        &sid("friend"),
        Spell::Poisoning,
        MirDirection::Left,
        Point { x: 12, y: 10 },
        true,
        3,
        10,
        5,
        500,
        1,
        now,
    );
    assert!(cast.is_empty());
    assert_eq!(zone.players[&sid("enemy")].mp, before_mp);
}

#[test]
fn purification_accepts_current_source_group_guild_enemy_guild_and_pk_199() {
    for relation in ["group", "guild", "enemy_guild", "no_caster_guild", "pk199"] {
        let (mut zone, now) = source_fixture();
        let mut healer = zone.players[&sid("healer")].chat_profile.clone();
        let mut friend = zone.players[&sid("friend")].chat_profile.clone();
        match relation {
            "group" => {
                healer.attack_mode = 1;
                assert!(healer.group_members.is_empty());
                friend.group_members.push("healer".into());
            }
            "guild" => {
                healer.attack_mode = 2;
                healer.guild_name = Some("A".into());
                friend.guild_name = Some("A".into());
            }
            "enemy_guild" => {
                healer.attack_mode = 3;
                healer.guild_name = Some("A".into());
                friend.guild_name = Some("B".into());
            }
            "no_caster_guild" => {
                healer.attack_mode = 3;
                friend.guild_name = Some("B".into());
            }
            "pk199" => {
                healer.attack_mode = 4;
                friend.pk_points = 199;
            }
            _ => unreachable!(),
        }
        zone.handle(ZoneCommand::UpdateChatProfile {
            session_id: sid("healer"),
            profile: healer,
        });
        zone.handle(ZoneCommand::UpdateChatProfile {
            session_id: sid("friend"),
            profile: friend,
        });
        assert!(!cleared(&cast(&mut zone, now, 3)));
        assert!(!cleared(&zone.tick(now + 499)), "{relation}");
        assert!(cleared(&zone.tick(now + 500)), "{relation}");
        assert!(poison_targets(&zone).is_empty());
    }
}

#[test]
fn purification_completion_uses_current_relationship_and_target_position() {
    let (mut zone, now) = source_fixture();
    assert!(!cleared(&cast(&mut zone, now, 3)));
    let mut profile = zone.players[&sid("healer")].chat_profile.clone();
    profile.attack_mode = 5;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid("healer"),
        profile,
    });
    assert!(!cleared(&zone.tick(now + 499)));
    let mut profile = zone.players[&sid("healer")].chat_profile.clone();
    profile.attack_mode = 0;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid("healer"),
        profile,
    });
    zone.handle(ZoneCommand::SyncPlayerTransform {
        session_id: sid("friend"),
        position: Point { x: 80, y: 80 },
        direction: MirDirection::Left,
    });
    let completion = zone.tick(now + 500);
    assert!(
        cleared(&completion),
        "Source completion has no distance or original-position gate"
    );
    assert!(completion.iter().any(|out| matches!(out,
        ZoneOutbound::ToMany { session_ids, packets }
            if session_ids.contains(&sid("friend")) && !session_ids.contains(&sid("healer"))
                && packets.iter().any(|p| matches!(p,
                    ServerPacket::ObjectPoisoned { object_id: 102, poison: 0 })))));
}

#[test]
fn purification_level_chance_is_applied_at_completion() {
    for level in 0..=3 {
        let mut successes = 0;
        for offset in 0..4 {
            let (mut zone, now) = source_fixture();
            let now = now + offset;
            assert!(!cleared(&cast(&mut zone, now, level)));
            assert!(!cleared(&zone.tick(now + 499)));
            if cleared(&zone.tick(now + 500)) {
                successes += 1;
                assert!(poison_targets(&zone).is_empty());
            } else {
                assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
                assert!(!poison_targets(&zone).is_empty());
            }
        }
        assert_eq!(successes, usize::from(level) + 1);
    }
}

#[test]
fn purification_self_cast_remains_friendly_in_all_attack_mode() {
    let (mut zone, now) = source_fixture();
    assert_ne!(zone.players[&sid("friend")].poison, 0);
    assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
    let mut profile = zone.players[&sid("friend")].chat_profile.clone();
    profile.attack_mode = 5;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid("friend"),
        profile,
    });
    let out = self_cast(&mut zone, now);
    assert!(contains(&out, |p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::Purification,
            cast: true,
            ..
        }
    )));
    assert!(!cleared(&out));
    assert!(!cleared(&zone.tick(now + 499)));
    assert!(cleared(&zone.tick(now + 500)));
    assert!(poison_targets(&zone).is_empty());
    assert!(zone.players[&sid("friend")].buffs.contains_key(&9));
}

#[test]
fn purification_self_admission_requires_current_node_life_and_map() {
    for invalid in ["missing", "map", "life", "cold", "dead"] {
        let (mut zone, now) = source_fixture();
        match invalid {
            "cold" => {
                zone = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap()
            }
            "dead" => {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: sid("friend"),
                    hp: 0,
                    max_hp: 1_000,
                    mp: 1_000,
                    dead: true,
                });
            }
            _ => {
                let mut nodes = zone.online_presence.clone();
                match invalid {
                    "missing" => {
                        nodes.remove(&sid("friend"));
                    }
                    "map" => {
                        nodes.get_mut(&sid("friend")).unwrap().key = ZoneKey::for_map("other-map")
                    }
                    "life" => nodes.get_mut(&sid("friend")).unwrap().life_generation += 1,
                    _ => unreachable!(),
                }
                zone.ingest_online_presence(nodes, true);
            }
        }
        let mp = zone.players[&sid("friend")].mp;
        let out = self_cast(&mut zone, now);
        assert!(
            !contains(&out, |p| matches!(
                p,
                ServerPacket::Magic {
                    spell: Spell::Purification,
                    cast: true,
                    ..
                }
            )),
            "{invalid}"
        );
        assert_eq!(zone.players[&sid("friend")].mp, mp, "{invalid}");
        assert!(zone.pending_native_purifications.is_empty(), "{invalid}");
    }
}

#[test]
fn purification_self_completion_cannot_rebind_old_life_or_node() {
    for invalid in [
        "missing", "map", "life", "cold", "dead", "revive", "logout", "rejoin",
    ] {
        let (mut zone, now) = source_fixture();
        let cast = self_cast(&mut zone, now);
        assert!(contains(&cast, |p| matches!(
            p,
            ServerPacket::Magic {
                spell: Spell::Purification,
                cast: true,
                ..
            }
        )));
        assert!(!cleared(&cast));
        match invalid {
            "cold" => {
                zone = ZoneRuntime::restore_checkpoint(&zone.checkpoint_bytes().unwrap()).unwrap();
                zone.handle(ZoneCommand::Leave {
                    session_id: sid("friend"),
                });
                zone.handle(ZoneCommand::Join(join("friend", 102, 12)));
            }
            "dead" | "revive" => {
                zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                    session_id: sid("friend"),
                    hp: 0,
                    max_hp: 1_000,
                    mp: 1_000,
                    dead: true,
                });
                if invalid == "revive" {
                    zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                        session_id: sid("friend"),
                        hp: 1_000,
                        max_hp: 1_000,
                        mp: 1_000,
                        dead: false,
                    });
                }
            }
            "logout" | "rejoin" => {
                zone.handle(ZoneCommand::Leave {
                    session_id: sid("friend"),
                });
                if invalid == "rejoin" {
                    zone.handle(ZoneCommand::Join(join("friend", 102, 12)));
                }
            }
            _ => {
                let mut nodes = zone.online_presence.clone();
                match invalid {
                    "missing" => {
                        nodes.remove(&sid("friend"));
                    }
                    "map" => {
                        nodes.get_mut(&sid("friend")).unwrap().key = ZoneKey::for_map("other-map")
                    }
                    "life" => nodes.get_mut(&sid("friend")).unwrap().life_generation += 1,
                    _ => unreachable!(),
                }
                zone.ingest_online_presence(nodes, true);
            }
        }
        assert!(!cleared(&zone.tick(now + 499)), "{invalid}");
        assert!(!cleared(&zone.tick(now + 500)), "{invalid}");
        assert!(zone.pending_native_purifications.is_empty(), "{invalid}");
    }
}

#[test]
fn purification_completion_rechecks_real_brown_time_with_strict_expiry() {
    let (mut zone, now) = source_fixture();
    let mut profile = zone.players[&sid("friend")].chat_profile.clone();
    profile.attack_mode = 5;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid("friend"),
        profile,
    });
    // A real hostile Poisoning of the peaceful healer creates BrownTime.
    let hostile = zone.handle(ZoneCommand::PlayerCastMagicWithItem {
        session_id: sid("friend"),
        object_id: 101,
        spell: Spell::Poisoning,
        direction: MirDirection::Left,
        target: Point { x: 10, y: 10 },
        cast: true,
        level: 3,
        damage: 10,
        mp_cost: 1,
        cooldown_ms: 500,
        item_param: 1,
        now_ms: now + 10,
    });
    assert!(contains(&hostile, |p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::Poisoning,
            cast: true,
            ..
        }
    )));
    let applied = zone.tick(now + 510);
    assert!(contains(&applied, |p| matches!(p,
        ServerPacket::ObjectPoisoned { object_id: 101, poison } if *poison != 0)));
    let deadline = zone.owned_pet_brown_until(&zone.players[&sid("friend")]);
    assert!(deadline > now + 1_700);
    let cast_at = now + 1_200; // Keep the original Struck gate after healer damage.
    assert!(!cleared(&cast(&mut zone, cast_at, 3)));
    let mut profile = zone.players[&sid("healer")].chat_profile.clone();
    profile.attack_mode = 4;
    zone.handle(ZoneCommand::UpdateChatProfile {
        session_id: sid("healer"),
        profile,
    });
    assert!(!cleared(&zone.tick(cast_at + 499)));
    assert!(!cleared(&zone.tick(cast_at + 500)));
    assert!(zone.players[&sid("friend")].buffs.contains_key(&12));
    let healer = &zone.players[&sid("healer")];
    let friend = &zone.players[&sid("friend")];
    assert!(!zone.native_purification_players_are_friendly(healer, friend, deadline));
    assert!(zone.native_purification_players_are_friendly(healer, friend, deadline + 1));
}

#[test]
fn purification_live_transfer_moves_only_the_exact_caster_and_preserves_original_records() {
    let (mut zone, now) = source_fixture();
    cast(&mut zone, now, 3);
    let self_cast = self_cast(&mut zone, now);
    assert!(contains(&self_cast, |p| matches!(
        p,
        ServerPacket::Magic {
            spell: Spell::Purification,
            cast: true,
            ..
        }
    )));
    let original = zone.pending_native_purifications.clone();
    let buffs = serde_json::to_value(&zone.players[&sid("friend")].buffs).unwrap();
    let healer = zone
        .take_online_purification_transfer(&sid("healer"))
        .unwrap();
    assert_eq!(healer.pending.len(), 1);
    assert_eq!(healer.pending[0].caster, original[0].caster);
    assert_eq!(healer.pending[0].target, original[0].target);
    assert_eq!(healer.pending[0].level, original[0].level);
    assert_eq!(healer.pending[0].ready_at_ms, original[0].ready_at_ms);
    assert_eq!(zone.pending_native_purifications.len(), 1);
    assert_eq!(
        zone.pending_native_purifications[0].caster,
        original[1].caster
    );
    let friend = zone
        .take_online_purification_transfer(&sid("friend"))
        .unwrap();
    assert_eq!(serde_json::to_value(&friend.buffs).unwrap(), buffs);
    assert!(zone.pending_native_purifications.is_empty());
    assert!(zone.players[&sid("friend")].buffs.is_empty());
    zone.adopt_online_purification_transfer(&sid("healer"), healer, &mut []);
    zone.adopt_online_purification_transfer(&sid("friend"), friend, &mut []);
    assert_eq!(zone.pending_native_purifications.len(), 2);
    assert_eq!(
        serde_json::to_value(&zone.players[&sid("friend")].buffs).unwrap(),
        buffs
    );
    for (actual, expected) in zone.pending_native_purifications.iter().zip(original) {
        assert_eq!(actual.caster, expected.caster);
        assert_eq!(actual.target, expected.target);
        assert_eq!(actual.level, expected.level);
        assert_eq!(actual.ready_at_ms, expected.ready_at_ms);
    }
}

#[test]
fn purification_live_transfer_cannot_adopt_into_a_rejoined_same_id_node_or_new_life() {
    for replacement in ["rejoin", "revive"] {
        let (mut zone, now) = source_fixture();
        self_cast(&mut zone, now);
        let transfer = zone
            .take_online_purification_transfer(&sid("friend"))
            .unwrap();
        if replacement == "rejoin" {
            zone.handle(ZoneCommand::Join(join("friend", 102, 12)));
        } else {
            zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: sid("friend"),
                hp: 0,
                max_hp: 1_000,
                mp: 1_000,
                dead: true,
            });
            zone.handle(ZoneCommand::SyncPlayerVitalsAndLife {
                session_id: sid("friend"),
                hp: 1_000,
                max_hp: 1_000,
                mp: 1_000,
                dead: false,
            });
        }
        zone.adopt_online_purification_transfer(&sid("friend"), transfer, &mut []);
        assert!(
            zone.pending_native_purifications.is_empty(),
            "{replacement}"
        );
        assert!(
            zone.players[&sid("friend")].buffs.is_empty(),
            "{replacement}"
        );
    }
}

#[test]
fn purification_live_transfer_cannot_restore_owner_authority_after_cold_decode() {
    let (mut zone, now) = source_fixture();
    self_cast(&mut zone, now);
    let transfer = zone
        .take_online_purification_transfer(&sid("friend"))
        .unwrap();
    let checkpoint = zone.checkpoint_bytes().unwrap();
    let mut cold = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    cold.adopt_online_purification_transfer(&sid("friend"), transfer, &mut []);
    assert!(cold.pending_native_purifications.is_empty());
    assert!(cold.players[&sid("friend")].buffs.is_empty());
}
