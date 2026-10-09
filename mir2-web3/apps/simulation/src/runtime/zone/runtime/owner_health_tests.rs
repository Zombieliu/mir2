//! Prepared fixtures call real pool operations; display never settles life.
use super::*;
use crate::{ZoneChatProfile, ZonePlayerCombatStats};
use mir2_protocol::MirGender;

fn join(session: &str, id: u32, class: MirClass, hp: i32, max_hp: i32, mp: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(session),
        account_id: format!("health-{session}"),
        character_index: 0,
        object_id: id,
        name: session.into(),
        class,
        gender: MirGender::Male,
        level: 30,
        hp,
        max_hp,
        mp,
        map_file_name: "owner-health".into(),
        position: Point {
            x: id as i32 + 10,
            y: 10,
        },
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile {
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: ZonePlayerCombatStats {
            max_sc: 10,
            ..Default::default()
        },
    }
}

fn fixture(hp: i32, max_hp: i32, mp: i32) -> (ZoneRuntime, SessionId) {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("owner-health"),
        ZoneCollision::unbounded(),
    );
    let player = join("owner", 1, MirClass::Warrior, hp, max_hp, mp);
    let session = player.session_id.clone();
    zone.handle(ZoneCommand::Join(player));
    (zone, session)
}

fn changes(out: &[ZoneOutbound]) -> Vec<crate::ZoneOwnerHealthChange> {
    out.iter()
        .filter_map(|out| match out {
            ZoneOutbound::OwnerHealthChanged { change } => Some(change.clone()),
            _ => None,
        })
        .collect()
}

fn contains(out: &ZoneOutbound, expected: impl Fn(&ServerPacket) -> bool) -> bool {
    match out {
        ZoneOutbound::ToSession { packets, .. }
        | ZoneOutbound::ToMany { packets, .. }
        | ZoneOutbound::ToAll { packets } => packets.iter().any(expected),
        _ => false,
    }
}

#[test]
fn owner_health_flame_mp_only_toggle_order_and_duplicate_rejection() {
    let (mut zone, session) = fixture(100, 200, 100);
    let out = zone.handle(ZoneCommand::PreparePlayerFlamingSword {
        session_id: session.clone(),
        level: 0,
        now_ms: 0,
    });
    let events = changes(&out);
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].hp_before, events[0].hp), (100, 100));
    assert!(events[0].mp < 100);
    let toggle = out
        .iter()
        .position(|o| {
            contains(o, |p| {
                matches!(p, ServerPacket::SpellToggle { can_use: true, .. })
            })
        })
        .unwrap();
    let health = out
        .iter()
        .position(|o| matches!(o, ZoneOutbound::OwnerHealthChanged { .. }))
        .unwrap();
    let mana = out
        .iter()
        .position(|o| contains(o, |p| matches!(p, ServerPacket::ObjectMana { .. })))
        .unwrap();
    assert!(toggle < health && health < mana);
    assert!(
        changes(&zone.handle(ZoneCommand::PreparePlayerFlamingSword {
            session_id: session,
            level: 0,
            now_ms: 1
        }))
        .is_empty()
    );
}

#[test]
fn owner_health_normal_magic_cost_precedes_action_and_rejection_is_silent() {
    let (mut zone, session) = fixture(100, 200, 100);
    zone.players.get_mut(&session).unwrap().class = MirClass::Taoist;
    zone.handle(ZoneCommand::sync_player_combat_state(
        session.clone(),
        MirClass::Taoist,
        false,
        false,
        true,
        false,
        false,
        false,
    ));
    // A self cast needs no prepared party relation. The delayed heal must not
    // collapse into the immediate MP-cost notification under test.
    let cast = |now_ms| ZoneCommand::PlayerCastMagic {
        session_id: session.clone(),
        object_id: 1,
        spell: Spell::Healing,
        direction: MirDirection::Right,
        target: Point { x: 11, y: 10 },
        cast: true,
        level: 0,
        damage: 5,
        mp_cost: 3,
        cooldown_ms: 1800,
        now_ms,
    };
    let out = zone.handle(cast(0));
    assert!(
        out.iter()
            .any(|o| contains(o, |p| matches!(p, ServerPacket::Magic { cast: true, .. }))),
        "prepared spell admission failed: {out:?}"
    );
    assert_eq!(
        changes(&out)
            .iter()
            .map(|e| (e.hp, e.mp))
            .collect::<Vec<_>>(),
        vec![(100, 97)]
    );
    let health = out
        .iter()
        .position(|o| matches!(o, ZoneOutbound::OwnerHealthChanged { .. }))
        .unwrap();
    let action = out
        .iter()
        .position(|o| contains(o, |p| matches!(p, ServerPacket::Magic { cast: true, .. })))
        .unwrap();
    assert!(health < action);
    assert!(changes(&zone.handle(cast(1))).is_empty());
    assert_eq!(zone.player_vitals(&session).unwrap().0, 100);
}

#[test]
fn owner_health_one_hp_change_with_unchanged_percent_still_notifies() {
    let (mut zone, session) = fixture(905, 1000, 20);
    let (packets, out) = zone
        .apply_native_player_pvp_damage(&session, 99, 1, 0, 10)
        .unwrap();
    let events = changes(&out);
    assert_eq!(
        events
            .iter()
            .map(|e| (e.hp_before, e.hp, e.mp))
            .collect::<Vec<_>>(),
        vec![(905, 904, 20)]
    );
    assert_eq!(
        native_player_health_percent(905, 1000),
        native_player_health_percent(904, 1000)
    );
    assert!(packets
        .iter()
        .all(|p| !matches!(p, ServerPacket::ObjectDied { .. })));
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, ZoneOutbound::PlayerDamaged { damage: 1, .. }))
            .count(),
        1
    );
    assert!(zone.drain_owner_health_changes().is_empty());
}

#[test]
fn owner_health_energy_shield_intermediate_heal_is_not_lost_when_net_hp_is_zero() {
    let (mut zone, session) = fixture(100, 200, 20);
    zone.apply_native_player_buff(
        &session,
        20,
        10000,
        vec![
            UserItemStat {
                stat: 125,
                value: 100,
            },
            UserItemStat {
                stat: 126,
                value: 10,
            },
        ],
        0,
    );
    let (_, out) = zone
        .apply_native_player_pvp_damage(&session, 99, 10, 0, 1)
        .unwrap();
    let events = changes(&out);
    assert_eq!(
        events
            .iter()
            .map(|e| (e.hp_before, e.hp, e.mp))
            .collect::<Vec<_>>(),
        vec![(100, 110, 20), (110, 100, 20)]
    );
    assert!(events[0].cursor.health_sequence < events[1].cursor.health_sequence);
    assert_eq!(zone.player_vitals(&session).unwrap().0, 100);
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, ZoneOutbound::PlayerHealed { amount: 10, .. }))
            .count(),
        1
    );
    assert_eq!(
        out.iter()
            .filter(|o| matches!(o, ZoneOutbound::PlayerDamaged { damage: 10, .. }))
            .count(),
        1
    );
}

#[test]
fn owner_health_plague_mp_zero_operation_precedes_lethal_hp_and_clears_poison() {
    let (mut zone, target) = fixture(10, 100, 0);
    let caster_join = join("caster", 2, MirClass::Taoist, 100, 100, 100);
    let caster = caster_join.session_id.clone();
    zone.handle(ZoneCommand::Join(caster_join));
    let action = PendingNativeGroundSpellAction {
        spell: Spell::Plague,
        skill_level: 3,
        caster_session_id: caster,
        caster_object_id: 2,
        owned_caster: None,
        locations: vec![Point { x: 11, y: 10 }],
        spell_object_ids: vec![],
        direction: MirDirection::Left,
        spell_param: 2,
        damage: 10,
        spell_packets_at_ms: 0,
        spell_packets_sent: true,
        next_damage_at_ms: 0,
        expires_at_ms: 10000,
        tick_interval_ms: 1000,
        journey_event: None,
    };
    let out = zone.resolve_native_plague(&action, 1);
    let events = changes(&out);
    assert_eq!(
        events.iter().map(|e| (e.hp, e.mp)).collect::<Vec<_>>(),
        vec![(10, 0), (0, 0)]
    );
    let death = out
        .iter()
        .position(|o| contains(o, |p| matches!(p, ServerPacket::Death { .. })))
        .unwrap();
    let positions = out
        .iter()
        .enumerate()
        .filter_map(|(i, o)| matches!(o, ZoneOutbound::OwnerHealthChanged { .. }).then_some(i))
        .collect::<Vec<_>>();
    assert!(positions[0] < death && death < positions[1]);
    assert_eq!(zone.players[&target].poison, 0);
    assert!(zone.players[&target].dead);
}

#[test]
fn owner_health_reincarnation_new_life_health_precedes_revived_and_settles_once() {
    let (mut zone, session) = fixture(10, 100, 20);
    let caster_join = join("caster", 2, MirClass::Taoist, 100, 100, 100);
    let caster = caster_join.session_id.clone();
    zone.handle(ZoneCommand::Join(caster_join));
    let (_, death) = zone
        .apply_native_player_pvp_damage(&session, 2, 10, 0, 1)
        .unwrap();
    let prior_life = changes(&death)[0].cursor.life_generation;
    zone.players.get_mut(&session).unwrap().reincarnation_offer =
        Some(super::super::types::ZoneReincarnationOffer {
            caster_session_id: caster,
            caster_object_id: 2,
            ready_at_ms: 0,
            expires_at_ms: 10000,
            effect_sent: true,
            requested: true,
            will_succeed: true,
        });
    let out = zone.handle(ZoneCommand::ResolveReincarnation {
        session_id: session.clone(),
        accept: true,
        now_ms: 2,
    });
    let events = changes(&out);
    assert_eq!(events.len(), 1);
    assert!(events[0].cursor.life_generation > prior_life);
    assert_eq!(events[0].hp, 50);
    let health = out
        .iter()
        .position(|o| matches!(o, ZoneOutbound::OwnerHealthChanged { .. }))
        .unwrap();
    let revived = out
        .iter()
        .position(|o| contains(o, |p| matches!(p, ServerPacket::Revived)))
        .unwrap();
    assert!(health < revived);
    assert!(changes(&zone.handle(ZoneCommand::ResolveReincarnation {
        session_id: session,
        accept: true,
        now_ms: 3
    }))
    .is_empty());
}

#[test]
fn owner_health_generic_sync_is_silent_explicit_personal_commit_preserves_hp_then_mp() {
    let (mut zone, session) = fixture(40, 100, 20);
    assert!(changes(&zone.handle(ZoneCommand::SyncPlayerVitals {
        session_id: session.clone(),
        hp: 40,
        max_hp: 200,
        mp: 20
    }))
    .is_empty());
    let out = zone.commit_owner_vitals(&session, 50, 200, 30, false);
    assert_eq!(
        changes(&out)
            .iter()
            .map(|e| (e.hp, e.mp))
            .collect::<Vec<_>>(),
        vec![(50, 20), (50, 30)]
    );
    assert!(changes(&zone.commit_owner_vitals(&session, 50, 300, 30, false)).is_empty());
}

#[test]
fn owner_health_transaction_fork_carries_fifo_but_checkpoint_never_replays_it() {
    let (mut zone, session) = fixture(40, 100, 20);
    zone.players.get_mut(&session).unwrap().change_owner_mp(-1);
    let mut rejected_fork = zone.transaction_fork();
    rejected_fork
        .players
        .get_mut(&session)
        .unwrap()
        .change_owner_mp(-2);
    assert_eq!(
        changes(&rejected_fork.drain_owner_health_changes())
            .iter()
            .map(|e| e.mp)
            .collect::<Vec<_>>(),
        vec![19, 17]
    );
    assert_eq!(zone.players[&session].mp, 19);
    let mut accepted_fork = zone.transaction_fork();
    assert_eq!(
        changes(&accepted_fork.drain_owner_health_changes()).len(),
        1
    );
    let checkpoint = zone.checkpoint_bytes().unwrap();
    let mut restored = ZoneRuntime::restore_checkpoint(&checkpoint).unwrap();
    assert!(restored.drain_owner_health_changes().is_empty());
    assert_eq!(restored.players[&session].owner_health_sequence, 1);
    assert_eq!(changes(&zone.drain_owner_health_changes()).len(), 1);
}

#[test]
fn owner_health_manager_retains_same_life_history_but_rejects_old_admission_and_revived_life() {
    let mut manager = super::super::manager::ZoneManager::new();
    let player = join("owner", 1, MirClass::Warrior, 10, 100, 20);
    let session = player.session_id.clone();
    manager.join(player.clone());
    let live = changes(&manager.commit_owner_vitals(&session, 9, 100, 19, false));
    let dead = changes(&manager.commit_owner_vitals(&session, 0, 100, 19, true));
    assert!(live
        .iter()
        .chain(&dead)
        .all(|event| manager.owner_health_change_is_current(event)));
    manager.commit_owner_vitals(&session, 50, 100, 19, false);
    assert!(live
        .iter()
        .chain(&dead)
        .all(|event| !manager.owner_health_change_is_current(event)));
    let current = changes(&manager.commit_owner_vitals(&session, 49, 100, 19, false));
    manager.handle(ZoneCommand::Leave {
        session_id: session.clone(),
    });
    manager.join(player);
    assert!(!manager.owner_health_change_is_current(&current[0]));
}

#[test]
fn owner_health_manager_dead_level_refill_is_current_without_revive() {
    let mut manager = super::super::manager::ZoneManager::new();
    let player = join("dead-level", 1, MirClass::Warrior, 10, 100, 20);
    let session = player.session_id.clone();
    manager.join(player);
    manager.commit_owner_vitals(&session, 0, 100, 20, true);
    let life = manager
        .owner_health_cursor(&session)
        .unwrap()
        .life_generation;
    let refill = changes(&manager.commit_owner_vitals(&session, 200, 200, 30, true));
    assert_eq!(
        refill.iter().map(|e| (e.hp, e.mp)).collect::<Vec<_>>(),
        vec![(200, 20), (200, 30)]
    );
    assert!(refill.iter().all(|e| e.cursor.dead
        && e.cursor.life_generation == life
        && manager.owner_health_change_is_current(e)));
    assert_eq!(manager.player_is_dead(&session), Some(true));
    manager.commit_owner_vitals(&session, 200, 200, 30, false);
    assert!(refill
        .iter()
        .all(|e| !manager.owner_health_change_is_current(e)));
}
