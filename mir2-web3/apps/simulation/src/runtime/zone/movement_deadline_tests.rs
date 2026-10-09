use super::super::types::{ZoneChatProfile, ZonePlayerCombatStats};
use super::*;
use mir2_protocol::{MirClass, MirGender};

fn fixture() -> (ZoneRuntime, SessionId) {
    let mut zone = ZoneRuntime::new_with_collision(
        ZoneKey::for_map("deadline-test"),
        ZoneCollision::unbounded(),
    );
    let id = SessionId::new("deadline-player");
    zone.handle(ZoneCommand::Join(ZoneJoin {
        session_id: id.clone(),
        account_id: "temporary-deadline-account".into(),
        character_index: 1,
        object_id: 1000,
        name: "Deadline".into(),
        class: MirClass::Warrior,
        gender: MirGender::Male,
        level: 7,
        hp: 60,
        max_hp: 60,
        mp: 100,
        map_file_name: "deadline-test".into(),
        position: Point { x: 10, y: 10 },
        direction: MirDirection::Right,
        chat_profile: ZoneChatProfile::default(),
        combat_stats: ZonePlayerCombatStats::default(),
    }));
    (zone, id)
}
fn walk(zone: &mut ZoneRuntime, id: &SessionId, seq: u64, time: u64) {
    zone.handle(ZoneCommand::Walk {
        session_id: id.clone(),
        direction: MirDirection::Right,
        seq,
        now_ms: time,
    });
}
#[test]
fn movement_deadline_301_to_901_and_repeated_steps_do_not_accumulate_tick_cost() {
    let (mut zone, id) = fixture();
    walk(&mut zone, &id, 1, 301);
    assert_eq!(zone.players[&id].position.x, 11);
    for step in 1..8_u64 {
        let due = 301 + 600 * step;
        walk(&mut zone, &id, step + 1, due - 599);
        assert_eq!(zone.next_pending_movement_deadline_ms(), Some(due));
        assert!(zone.tick_pending_movement(due - 1).is_empty());
        zone.tick_pending_movement(due);
        assert_eq!(zone.players[&id].position.x, 11 + step as i32);
        assert_eq!(zone.players[&id].movement_ready_at_ms, due + 600);
        assert_eq!(zone.next_pending_movement_deadline_ms(), None);
    }
}
#[test]
fn movement_deadline_late_wake_does_not_replay_steps_or_shorten_next_cooldown() {
    let (mut zone, id) = fixture();
    walk(&mut zone, &id, 1, 301);
    walk(&mut zone, &id, 2, 302);
    zone.tick_pending_movement(3000);
    assert_eq!(zone.players[&id].position.x, 12);
    assert_eq!(zone.players[&id].movement_ready_at_ms, 3600);
    assert!(zone.tick_pending_movement(3000).is_empty());
    assert_eq!(zone.next_pending_movement_deadline_ms(), None);
}
#[test]
fn movement_deadline_cancel_leave_and_frozen_poison_do_not_leave_due_work() {
    let (mut zone, id) = fixture();
    walk(&mut zone, &id, 1, 301);
    walk(&mut zone, &id, 2, 302);
    zone.handle(ZoneCommand::CancelPendingMovement {
        session_id: id.clone(),
    });
    assert_eq!(zone.next_pending_movement_deadline_ms(), None);
    walk(&mut zone, &id, 3, 303);
    zone.players.get_mut(&id).unwrap().native_status_poison = CRYSTAL_POISON_FROZEN;
    zone.players
        .get_mut(&id)
        .unwrap()
        .native_status_poison_expires_at_ms = Some(5000);
    zone.tick_pending_movement(901);
    assert_eq!(zone.players[&id].position.x, 11);
    assert_eq!(zone.next_pending_movement_deadline_ms(), None);
    zone.players.get_mut(&id).unwrap().native_status_poison = 0;
    walk(&mut zone, &id, 4, 1000);
    walk(&mut zone, &id, 5, 1001);
    zone.handle(ZoneCommand::Leave { session_id: id });
    assert_eq!(zone.next_pending_movement_deadline_ms(), None);
}
