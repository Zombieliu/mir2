//! Ordinary authenticated player packets through the shared Gateway route.
//! Fixtures are in-memory test accounts; no live save, admin command or client
//! supplied timing scalar is used to authorize a cast.
use super::super::gateway_zone_magic_targets_self;
use super::*;

fn fixture(
    name: &str,
    class: MirClass,
    spells: &[(Spell, &str)],
) -> (SharedInProcessZoneSessionRuntime, WorldEntitySnapshot) {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared);
    start_new_runtime_with_class(&mut runtime, name, name, class);
    // A durable learned-skill fixture is loaded through the trusted personal
    // save interface, exactly as an already progressed ordinary character.
    let mut save = runtime.inner.active_character_checkpoint().unwrap();
    save.character.level = 60;
    save.mp = 200;
    save.max_mp = 200;
    if spells
        .iter()
        .any(|(spell, _)| *spell == Spell::SoulFireBall)
    {
        let amulet = mir2_game_data::crystal_item_by_name("Amulet").unwrap();
        save.equipment_items_json = vec![serde_json::json!({
            "key": format!("crystal-item-{}", amulet.item_index),
            "slot": mir2_simulation::EquipmentSlot::Amulet,
            "quantity": 100, "name": amulet.name, "icon": amulet.image,
            "shape": amulet.shape, "description": "",
            "durability_current": amulet.durability.max(1),
            "durability_max": amulet.durability.max(1), "attack": 0, "defence": 0,
        })
        .to_string()];
        save.equipment_items_explicit_empty = false;
    }
    save.skill_states_json = spells
        .iter()
        .map(|(spell, key)| {
            serde_json::json!({
                "key": key, "name": format!("{spell:?}"), "description": "",
                "level": 0, "experience": 0, "cooldown_ticks": 0, "cooldown_ends_at": 0,
            })
            .to_string()
        })
        .collect();
    runtime
        .inner
        .restore_active_character_checkpoint(&save)
        .unwrap();
    runtime.sync_zone_snapshot();
    let (target, _) = prepare_gateway_range_fixture(&mut runtime, 1);
    (runtime, target)
}

fn command(
    runtime: &SharedInProcessZoneSessionRuntime,
    spell: Spell,
    target: &WorldEntitySnapshot,
) -> WorldCommand {
    let owner = runtime
        .world_snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == WorldEntityKind::SelfPlayer)
        .unwrap();
    let self_spell = gateway_zone_magic_targets_self(spell);
    WorldCommand::ClientPacket(ClientPacket::Magic {
        object_id: owner.object_id,
        spell,
        direction: MirDirection::Right,
        target_id: if self_spell {
            owner.object_id
        } else {
            target.object_id
        },
        location: if self_spell {
            Point {
                x: owner.x,
                y: owner.y,
            }
        } else {
            Point {
                x: target.x,
                y: target.y,
            }
        },
        spell_target_lock: true,
    })
}

fn accepted(packets: &[ServerPacket], spell: Spell) -> bool {
    packets.iter().any(|packet| {
        matches!(packet,
        ServerPacket::Magic { spell: actual, cast: true, .. } if *actual == spell)
            || matches!(packet, ServerPacket::MagicCast { spell: Spell::ShoulderDash }
            if spell == Spell::ShoulderDash)
    })
}

fn remaining(runtime: &SharedInProcessZoneSessionRuntime, spell: Spell) -> u32 {
    runtime
        .world_snapshot()
        .known_skills
        .into_iter()
        .find(|skill| skill.spell.as_deref() == Some(format!("{spell:?}").as_str()))
        .unwrap()
        .cooldown_remaining_ms
        .expect("shared clock must supply exact time")
}

fn assert_rejected_without_effect(
    runtime: &mut SharedInProcessZoneSessionRuntime,
    command: WorldCommand,
) {
    let before = runtime.world_snapshot();
    let packets = runtime.execute(command).unwrap();
    assert!(
        !packets.iter().any(|packet| matches!(
            packet,
            ServerPacket::Magic { cast: true, .. }
                | ServerPacket::ObjectMagic { cast: true, .. }
                | ServerPacket::ObjectProjectile { .. }
                | ServerPacket::ObjectAttack { .. }
                | ServerPacket::MagicCast { .. }
        )),
        "{packets:?}"
    );
    let after = runtime.world_snapshot();
    assert_eq!(
        after.player_mp, before.player_mp,
        "rejection must not charge mana"
    );
    assert_eq!(
        after.inventory_items, before.inventory_items,
        "rejection must not consume an inventory item"
    );
    assert_eq!(
        after.equipment_items, before.equipment_items,
        "rejection must not consume an equipped reagent"
    );
    for old in before
        .entities
        .iter()
        .filter(|entity| entity.kind == WorldEntityKind::Monster)
    {
        if let Some(new) = after
            .entities
            .iter()
            .find(|entity| entity.object_id == old.object_id)
        {
            assert_eq!(
                new.hp, old.hp,
                "rejection must not damage {}",
                old.object_id
            );
        }
    }
}

#[test]
fn ordinary_warrior_wizard_and_taoist_cooldowns_gate_retries_and_expire() {
    for (name, class, spell, key) in [
        (
            "CadenceWarrior",
            MirClass::Warrior,
            Spell::ShoulderDash,
            "shoulderdash",
        ),
        (
            "CadenceWizard",
            MirClass::Wizard,
            Spell::FireBall,
            "fireball",
        ),
        (
            "CadenceTaoist",
            MirClass::Taoist,
            Spell::Healing,
            "minor-heal",
        ),
    ] {
        let (mut runtime, target) = fixture(name, class, &[(spell, key)]);
        let launch = command(&runtime, spell, &target);
        let packets = runtime.execute(launch).unwrap();
        assert!(accepted(&packets, spell), "{class:?}: {packets:?}");
        assert_eq!(
            packets
                .iter()
                .filter(|p| matches!(
                    p,
                    ServerPacket::Magic { cast: true, .. } | ServerPacket::MagicCast { .. }
                ))
                .count(),
            1,
            "one accepted cast produces one owner cooldown acknowledgement"
        );
        assert!(remaining(&runtime, spell) > 0);
        let retry = command(&runtime, spell, &target);
        assert_rejected_without_effect(&mut runtime, retry);
        // Packet/private ticks cannot shorten the wall-clock Zone gate.
        for _ in 0..8 {
            runtime.inner.execute(WorldCommand::Tick).unwrap();
        }
        assert!(remaining(&runtime, spell) > 0);
        let retry = command(&runtime, spell, &target);
        assert_rejected_without_effect(&mut runtime, retry);
        let delay = remaining(&runtime, spell);
        assert!(delay < 30_000);
        std::thread::sleep(Duration::from_millis(u64::from(delay) + 20));
        assert_eq!(remaining(&runtime, spell), 0);
        let retry = command(&runtime, spell, &target);
        let packets = runtime.execute(retry).unwrap();
        assert!(accepted(&packets, spell), "{class:?} expiry: {packets:?}");
    }
}

#[test]
fn ordinary_wizard_alternating_spells_cannot_escape_the_global_gate() {
    let (mut runtime, target) = fixture(
        "CadenceSwitch",
        MirClass::Wizard,
        &[
            (Spell::FireBall, "fireball"),
            (Spell::GreatFireBall, "greatfireball"),
        ],
    );
    let cast = command(&runtime, Spell::FireBall, &target);
    assert!(accepted(&runtime.execute(cast).unwrap(), Spell::FireBall));
    std::thread::sleep(Duration::from_millis(320));
    let delay = remaining(&runtime, Spell::GreatFireBall);
    assert!(
        delay > 0,
        "another spell must inherit Crystal's 1800ms global delay"
    );
    let early = command(&runtime, Spell::GreatFireBall, &target);
    assert_rejected_without_effect(&mut runtime, early);
    std::thread::sleep(Duration::from_millis(
        u64::from(remaining(&runtime, Spell::GreatFireBall)) + 20,
    ));
    let ready = command(&runtime, Spell::GreatFireBall, &target);
    assert!(accepted(
        &runtime.execute(ready).unwrap(),
        Spell::GreatFireBall
    ));
}

#[test]
fn ordinary_melee_and_magic_share_the_crystal_action_lock() {
    let (mut runtime, target) = fixture(
        "CadenceAction",
        MirClass::Wizard,
        &[(Spell::FireBall, "fireball")],
    );
    let cast = command(&runtime, Spell::FireBall, &target);
    assert!(accepted(&runtime.execute(cast).unwrap(), Spell::FireBall));
    let attack = || {
        WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        })
    };
    assert_rejected_without_effect(&mut runtime, attack());
    std::thread::sleep(Duration::from_millis(610));
    let packets = runtime.execute(attack()).unwrap();
    assert!(
        packets
            .iter()
            .any(|p| matches!(p, ServerPacket::ObjectAttack { .. })),
        "{packets:?}"
    );
}

#[test]
fn ordinary_taoist_cooldown_rejection_preserves_soulfire_reagent() {
    let (mut runtime, target) = fixture(
        "CadenceReagent",
        MirClass::Taoist,
        &[(Spell::SoulFireBall, "soulfireball")],
    );
    let before = runtime.world_snapshot().equipment_items;
    let cast = command(&runtime, Spell::SoulFireBall, &target);
    assert!(accepted(
        &runtime.execute(cast).unwrap(),
        Spell::SoulFireBall
    ));
    let after_first = runtime.world_snapshot().equipment_items;
    assert_ne!(
        after_first, before,
        "an accepted SoulFire consumes its actual amulet"
    );
    let early = command(&runtime, Spell::SoulFireBall, &target);
    assert_rejected_without_effect(&mut runtime, early);
    assert_eq!(runtime.world_snapshot().equipment_items, after_first);
    std::thread::sleep(Duration::from_millis(
        u64::from(remaining(&runtime, Spell::SoulFireBall)) + 20,
    ));
    let ready = command(&runtime, Spell::SoulFireBall, &target);
    assert!(accepted(
        &runtime.execute(ready).unwrap(),
        Spell::SoulFireBall
    ));
    assert_ne!(runtime.world_snapshot().equipment_items, after_first);
}

fn flame_prepare(toggle_state: i8) -> WorldCommand {
    WorldCommand::ClientPacket(ClientPacket::SpellToggle {
        spell: Spell::FlamingSword,
        toggle_state,
    })
}

fn flame_ack(packets: &[ServerPacket], can_use: bool) -> usize {
    packets
        .iter()
        .filter(|p| {
            matches!(p,
        ServerPacket::SpellToggle { spell: Spell::FlamingSword, can_use: actual, .. }
            if *actual == can_use)
        })
        .count()
}

fn flame_deadline(runtime: &SharedInProcessZoneSessionRuntime) -> u64 {
    let key = runtime.current_presence_key().unwrap();
    let state = runtime.zone_state.lock().unwrap();
    let session = &state.zone_sessions[&key];
    let map = &state.players[&key].map_file_name;
    // Reading remaining time at zero exposes the absolute Zone deadline,
    // without advancing any clock or depending on compilation load/latency.
    state
        .zone_manager
        .zone(&ZoneKey::for_map(map))
        .unwrap()
        .player_magic_cooldown_remaining_ms(session, Spell::FlamingSword, 0)
        .unwrap()
}

#[test]
fn ordinary_flaming_sword_prepare_and_consumed_hit_retain_ten_second_cooldown() {
    let (mut runtime, _) = fixture(
        "CadenceFlame",
        MirClass::Warrior,
        &[(Spell::FlamingSword, "flamingsword")],
    );
    let before = runtime.world_snapshot().player_mp.unwrap();
    let (_, _, cost, _) = runtime
        .inner
        .zone_magic_attack_profile(Spell::FlamingSword)
        .unwrap();
    // Crystal treats either positive enable/disable argument as preparation.
    let armed = runtime.execute(flame_prepare(0)).unwrap();
    assert_eq!(flame_ack(&armed, true), 1, "{armed:?}");
    assert_eq!(runtime.world_snapshot().player_mp, Some(before - cost));
    let first_deadline = flame_deadline(&runtime);
    assert!((1..=10_000).contains(&remaining(&runtime, Spell::FlamingSword)));
    for toggle in [1, 0] {
        let repeated = runtime.execute(flame_prepare(toggle)).unwrap();
        assert_eq!(flame_ack(&repeated, true), 0);
        assert_eq!(runtime.world_snapshot().player_mp, Some(before - cost));
        assert_eq!(flame_deadline(&runtime), first_deadline);
    }
    let hit = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::None,
        }))
        .unwrap();
    assert!(
        hit.iter().any(|p| matches!(p,
        ServerPacket::ObjectAttack { info } if info.spell == Spell::FlamingSword as u8)),
        "{hit:?}"
    );
    assert_eq!(
        flame_ack(&hit, false),
        1,
        "consumed Flame produces one owner reset"
    );
    assert_eq!(flame_deadline(&runtime), first_deadline);
    assert!(remaining(&runtime, Spell::FlamingSword) > 0);
    let early = runtime.execute(flame_prepare(1)).unwrap();
    assert_eq!(
        flame_ack(&early, true),
        0,
        "a hit consumes the charge, not its cooldown"
    );
    assert_eq!(runtime.world_snapshot().player_mp, Some(before - cost));
    assert_eq!(flame_deadline(&runtime), first_deadline);
    for _ in 0..8 {
        runtime.inner.execute(WorldCommand::Tick).unwrap();
    }
    assert!(remaining(&runtime, Spell::FlamingSword) > 0);
    let accelerated = runtime.execute(flame_prepare(1)).unwrap();
    assert_eq!(
        flame_ack(&accelerated, true),
        0,
        "private ticks cannot advance Flame's wall clock"
    );
    std::thread::sleep(Duration::from_millis(
        u64::from(remaining(&runtime, Spell::FlamingSword)) + 20,
    ));
    assert_eq!(remaining(&runtime, Spell::FlamingSword), 0);
    let ready = runtime.execute(flame_prepare(1)).unwrap();
    assert_eq!(flame_ack(&ready, true), 1, "expiry: {ready:?}");
    assert_eq!(runtime.world_snapshot().player_mp, Some(before - 2 * cost));
}

#[test]
fn ordinary_flaming_sword_mana_boundary_and_idle_expiry_reject_forged_burst() {
    let (mut runtime, _) = fixture(
        "CadenceFlameIdle",
        MirClass::Warrior,
        &[(Spell::FlamingSword, "flamingsword")],
    );
    let (_, _, cost, _) = runtime
        .inner
        .zone_magic_attack_profile(Spell::FlamingSword)
        .unwrap();
    let session_id = runtime.current_zone_session_id().unwrap();
    let hp = runtime.world_snapshot().player_hp.unwrap();
    let max_hp = runtime.world_snapshot().player_max_hp.unwrap();
    let set_mp = |runtime: &mut SharedInProcessZoneSessionRuntime, mp| {
        runtime.dispatch_zone_player_command(
            ZoneCommand::SyncPlayerVitals {
                session_id: session_id.clone(),
                hp,
                max_hp,
                mp,
            },
            false,
        );
        runtime
            .inner
            .force_authoritative_player_vitals(None, Some(mp));
    };
    set_mp(&mut runtime, cost);
    let denied = runtime.execute(flame_prepare(1)).unwrap();
    assert_eq!(flame_ack(&denied, true), 0);
    assert_eq!(runtime.world_snapshot().player_mp, Some(cost));
    assert_eq!(remaining(&runtime, Spell::FlamingSword), 0);
    set_mp(&mut runtime, cost + 1);
    let armed = runtime.execute(flame_prepare(1)).unwrap();
    assert_eq!(flame_ack(&armed, true), 1);
    assert_eq!(runtime.world_snapshot().player_mp, Some(1));
    let owner_id = runtime.local_self_object_id().unwrap();
    let max_mp = runtime.world_snapshot().player_max_mp.unwrap().max(1);
    let expected_percent = (100 / max_mp).clamp(0, 100) as u8;
    assert!(
        armed.iter().any(|p| matches!(p,
        ServerPacket::ObjectMana { info }
            if info.object_id == owner_id && info.percent == expected_percent)),
        "prepared mana is addressed to the local owner using its actual maximum: {armed:?}"
    );
    std::thread::sleep(Duration::from_millis(
        u64::from(remaining(&runtime, Spell::FlamingSword)) + 20,
    ));
    assert_eq!(remaining(&runtime, Spell::FlamingSword), 0);
    // No shared Tick was dispatched during the wait. Profile and Zone admission
    // must still expire the charge before an explicit client spell byte.
    let packets = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::FlamingSword,
        }))
        .unwrap();
    assert_eq!(flame_ack(&packets, false), 1, "{packets:?}");
    assert!(
        !packets.iter().any(|p| matches!(p,
        ServerPacket::ObjectAttack { info } if info.spell == Spell::FlamingSword as u8)),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|p| matches!(p,
        ServerPacket::ObjectAttack { info } if info.spell == Spell::None as u8)),
        "{packets:?}"
    );
    assert_ne!(
        runtime.inner.zone_melee_attack_profile(Spell::None).0,
        Spell::FlamingSword
    );
    let later = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::KeepAlive {
            time: 1,
        }))
        .unwrap();
    assert_eq!(flame_ack(&later, false), 0, "expiration must publish once");
}

#[test]
fn ordinary_flaming_sword_map_change_preserves_deadline_and_new_login_resets_charge() {
    let (mut runtime, _) = fixture(
        "CadenceFlameMap",
        MirClass::Warrior,
        &[(Spell::FlamingSword, "flamingsword")],
    );
    let character_index = runtime.inner.active_identity().unwrap().character_index;
    assert_eq!(
        flame_ack(&runtime.execute(flame_prepare(1)).unwrap(), true),
        1
    );
    let first_deadline = remaining(&runtime, Spell::FlamingSword);
    // Trusted map-arrival fixture; no debug key is sent through a public client.
    // The following ordinary packet uses the same production transfer/bootstrap.
    runtime
        .execute(WorldCommand::TransferMap {
            key: "crystal:D001:420:91".into(),
        })
        .unwrap();
    assert_eq!(
        runtime.world_snapshot().map_file_name.as_deref(),
        Some("D001")
    );
    let after_transfer = remaining(&runtime, Spell::FlamingSword);
    assert!(after_transfer > 0 && after_transfer <= first_deadline);
    assert_eq!(
        runtime.inner.zone_melee_attack_profile(Spell::None).0,
        Spell::FlamingSword
    );
    let mp = runtime.world_snapshot().player_mp;
    assert_eq!(
        flame_ack(&runtime.execute(flame_prepare(0)).unwrap(), true),
        0
    );
    assert_eq!(runtime.world_snapshot().player_mp, mp);
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::LogOut))
        .unwrap();
    runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index,
        }))
        .unwrap();
    assert_eq!(remaining(&runtime, Spell::FlamingSword), 0);
    assert_ne!(
        runtime.inner.zone_melee_attack_profile(Spell::None).0,
        Spell::FlamingSword,
        "new login must replace the previous personal toggle"
    );
    assert_eq!(
        flame_ack(&runtime.execute(flame_prepare(1)).unwrap(), true),
        1,
        "a fresh login starts with no ephemeral prepare deadline"
    );
}

#[test]
fn ordinary_flaming_sword_cannot_bypass_preparation_as_ranged_magic() {
    let (mut runtime, target) = fixture(
        "CadenceFlameMagic",
        MirClass::Warrior,
        &[(Spell::FlamingSword, "flamingsword")],
    );
    let forged = command(&runtime, Spell::FlamingSword, &target);
    assert_rejected_without_effect(&mut runtime, forged);
    assert_eq!(remaining(&runtime, Spell::FlamingSword), 0);
    assert_eq!(
        flame_ack(&runtime.execute(flame_prepare(1)).unwrap(), true),
        1
    );
    let deadline = remaining(&runtime, Spell::FlamingSword);
    let forged = command(&runtime, Spell::FlamingSword, &target);
    assert_rejected_without_effect(&mut runtime, forged);
    assert!(remaining(&runtime, Spell::FlamingSword) <= deadline);
    let hit = runtime
        .execute(WorldCommand::ClientPacket(ClientPacket::Attack {
            direction: MirDirection::Right,
            spell: Spell::FlamingSword,
        }))
        .unwrap();
    assert!(
        hit.iter().any(|p| matches!(p,
        ServerPacket::ObjectAttack { info } if info.spell == Spell::FlamingSword as u8)),
        "{hit:?}"
    );
}
