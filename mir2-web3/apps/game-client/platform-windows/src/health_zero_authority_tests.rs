//! Real packet adapter paths must not manufacture death from rounded display HP.
use super::*;
use serde_json::json;

fn payload() -> Value {
    json!({
        "playerObjectId":1000,"playerHp":1,"playerMaxHp":200,
        "sceneView":{"center":{"x":10,"y":10}},
        "entities":[
            {"objectId":1000,"kind":"selfPlayer","name":"Owner","x":10,"y":10,"hp":1,"maxHp":200,"dead":false},
            {"objectId":77,"kind":"monster","name":"Scarecrow","x":11,"y":10,"hp":1,"maxHp":200,"dead":false}
        ]
    })
}

fn packet(adapter: &mut NativeGameplayAdapter, name: &str, body: Value) {
    assert!(adapter.observe_packet(&PacketEvent::Other {
        packet: name.into(),
        payload: body
    }));
}

#[test]
fn health_zero_self_keeps_exact_positive_hp_and_live_action_state() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":1000,"percent":0,"expire":5}),
    );
    let mut live = payload();
    adapter.apply_authoritative_overlay(&mut live);
    assert_eq!(live["playerHp"], json!(1));
    assert_eq!(live["entities"][0]["hp"], json!(1));
    assert_eq!(live["entities"][0]["dead"], json!(false));
    assert_eq!(live["entities"][0]["_healthPercent"], json!(0));
}

#[test]
fn health_zero_monster_keeps_live_position_and_real_positive_hp() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":77,"percent":0,"expire":5}),
    );
    packet(
        &mut adapter,
        "ObjectWalk",
        json!({"objectId":77,"location":{"x":12,"y":10},"direction":"Right"}),
    );
    let mut live = payload();
    adapter.apply_authoritative_overlay(&mut live);
    assert_eq!(live["entities"][1]["hp"], json!(1));
    assert_eq!(live["entities"][1]["dead"], json!(false));
    assert_eq!(live["entities"][1]["x"], json!(12));
    assert_eq!(live["entities"][1]["_healthPercent"], json!(0));
}

#[test]
fn health_zero_without_exact_hp_does_not_fabricate_absolute_death() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "ObjectMonster",
        json!({"objectId":78,"name":"Scarecrow","location":{"x":12,"y":10},"direction":"Down","dead":false}),
    );
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":78,"percent":0,"expire":5}),
    );
    let mut live = payload();
    adapter.apply_authoritative_overlay(&mut live);
    let actor = live["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["objectId"] == json!(78))
        .unwrap();
    assert_eq!(actor["dead"], json!(false));
    assert!(
        actor.get("hp").is_none(),
        "percentage zero has no exact HP authority"
    );
    assert_eq!(actor["_healthPercent"], json!(0));
}

#[test]
fn health_zero_real_death_survives_stale_positive_health_and_snapshot() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "ObjectDied",
        json!({"objectId":77,"location":{"x":11,"y":10},"direction":"Down","kind":0}),
    );
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":77,"percent":100,"expire":5}),
    );
    let mut stale = payload();
    adapter.apply_authoritative_overlay(&mut stale);
    assert_eq!(stale["entities"][1]["dead"], json!(true));
    assert_eq!(stale["entities"][1]["hp"], json!(0));
}

#[test]
fn health_zero_actual_self_death_and_revive_keep_dedicated_lifecycle() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "Death",
        json!({"location":{"x":10,"y":10},"direction":"Down"}),
    );
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":1000,"percent":100,"expire":5}),
    );
    let mut dead = payload();
    adapter.apply_authoritative_overlay(&mut dead);
    assert_eq!(dead["playerHp"], json!(0));
    assert_eq!(dead["entities"][0]["dead"], json!(true));
    packet(&mut adapter, "Revived", json!({}));
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":1000,"percent":0,"expire":5}),
    );
    let mut revived = payload();
    adapter.apply_authoritative_overlay(&mut revived);
    assert_eq!(revived["playerHp"], json!(1));
    assert_eq!(revived["entities"][0]["dead"], json!(false));
}

#[test]
fn health_zero_does_not_override_real_snapshot_hp_zero() {
    let mut adapter = NativeGameplayAdapter::default();
    packet(
        &mut adapter,
        "ObjectHealth",
        json!({"objectId":1000,"percent":0,"expire":5}),
    );
    let mut actual = payload();
    actual["playerHp"] = json!(0);
    actual["entities"][0]["hp"] = json!(0);
    adapter.apply_authoritative_overlay(&mut actual);
    assert_eq!(actual["playerHp"], json!(0));
    assert_eq!(actual["entities"][0]["hp"], json!(0));
}

fn typed_zero(dead: bool) -> Value {
    let mut actual = payload();
    actual["playerHp"] = json!(0);
    actual["entities"][0]["hp"] = json!(0);
    actual["entities"][0]["dead"] = json!(dead);
    actual
}

fn positive_owner_health(adapter: &mut NativeGameplayAdapter, percent: u8) {
    packet(
        adapter,
        "ObjectHealth",
        json!({"objectId":1000,"percent":percent,"expire":5}),
    );
}

#[test]
fn health_zero_stale_positive_owner_health_cannot_replace_typed_dead_snapshot() {
    let mut adapter = NativeGameplayAdapter::default();
    positive_owner_health(&mut adapter, 100);
    let mut actual = typed_zero(true);
    adapter.apply_authoritative_overlay(&mut actual);
    assert_eq!(actual["playerHp"], json!(0));
    assert_eq!(actual["entities"][0]["hp"], json!(0));
    assert_eq!(actual["entities"][0]["dead"], json!(true));
    assert_eq!(adapter.animation_sequence, 0);
    assert!(adapter.effect_events.is_empty());
}

#[test]
fn health_zero_typed_alive_hp_zero_stays_zero_without_fabricated_death() {
    let mut adapter = NativeGameplayAdapter::default();
    positive_owner_health(&mut adapter, 100);
    let mut actual = typed_zero(false);
    adapter.apply_authoritative_overlay(&mut actual);
    assert_eq!(actual["playerHp"], json!(0));
    assert_eq!(actual["entities"][0]["hp"], json!(0));
    assert_eq!(actual["entities"][0]["dead"], json!(false));
    assert!(adapter.zone_entities[&1000].get("dead").is_none());
    assert!(adapter.authoritative_player_dead.is_none());
}

#[test]
fn health_zero_typed_self_entity_zero_is_preserved_when_root_hp_differs() {
    let mut adapter = NativeGameplayAdapter::default();
    positive_owner_health(&mut adapter, 100);
    let mut actual = payload();
    actual["playerHp"] = json!(200);
    actual["entities"][0]["hp"] = json!(0);
    adapter.apply_authoritative_overlay(&mut actual);
    assert_eq!(actual["playerHp"], json!(0));
    assert_eq!(actual["entities"][0]["hp"], json!(0));
    assert_eq!(actual["entities"][0]["dead"], json!(false));
}

#[test]
fn health_zero_typed_self_dead_flag_reaches_hud_without_death_packet() {
    let mut adapter = NativeGameplayAdapter::default();
    positive_owner_health(&mut adapter, 100);
    let mut actual = payload();
    actual["playerHp"] = json!(200);
    actual["entities"][0]["dead"] = json!(true);
    adapter.apply_authoritative_overlay(&mut actual);
    assert_eq!(actual["playerHp"], json!(0));
    assert_eq!(actual["entities"][0]["hp"], json!(0));
    assert_eq!(actual["entities"][0]["dead"], json!(true));
}

#[test]
fn health_zero_explicit_revived_beats_old_dead_snapshot_then_generation_clears_it() {
    let mut adapter = NativeGameplayAdapter::default();
    adapter.set_generation(1);
    let old = typed_zero(true);
    adapter.observe_world_snapshot(&old);
    packet(&mut adapter, "Revived", json!({}));
    positive_owner_health(&mut adapter, 50);
    let mut revived = old.clone();
    adapter.apply_authoritative_overlay(&mut revived);
    assert_eq!(revived["playerHp"], json!(100));
    assert_eq!(revived["entities"][0]["hp"], json!(100));
    assert_eq!(revived["entities"][0]["dead"], json!(false));
    assert_eq!(
        revived["entities"][0]["_nativeAnimationAction"],
        json!("standing")
    );
    adapter.set_generation(2);
    positive_owner_health(&mut adapter, 100);
    let mut current = old;
    adapter.apply_authoritative_overlay(&mut current);
    assert_eq!(current["playerHp"], json!(0));
    assert_eq!(current["entities"][0]["hp"], json!(0));
    assert_eq!(current["entities"][0]["dead"], json!(true));
}

#[test]
fn health_zero_self_revived_releases_old_died_overlay_before_next_positive_health() {
    let mut adapter = NativeGameplayAdapter::default();
    let old = typed_zero(true);
    adapter.observe_world_snapshot(&old);
    packet(&mut adapter, "Death", json!({"location":{"x":10,"y":10}}));
    packet(
        &mut adapter,
        "ObjectDied",
        json!({"objectId":1000,"location":{"x":10,"y":10},"direction":"Down","kind":0}),
    );
    packet(&mut adapter, "Revived", json!({}));
    positive_owner_health(&mut adapter, 50);
    let mut revived = old;
    adapter.apply_authoritative_overlay(&mut revived);
    assert_eq!(revived["playerHp"], json!(100));
    assert_eq!(revived["entities"][0]["hp"], json!(100));
    assert_eq!(revived["entities"][0]["dead"], json!(false));
    assert_eq!(adapter.zone_entities[&1000]["_healthPercent"], json!(50));
    assert_eq!(
        revived["entities"][0]["_nativeAnimationAction"],
        json!("standing")
    );
    assert_eq!(adapter.effect_events.len(), 3);
}

#[test]
fn health_zero_object_revived_and_fresh_owner_spawn_keep_new_life() {
    for lifecycle in ["ObjectRevived", "ObjectPlayer"] {
        let mut adapter = NativeGameplayAdapter::default();
        let old = typed_zero(true);
        adapter.observe_world_snapshot(&old);
        packet(
            &mut adapter,
            "ObjectDied",
            json!({"objectId":1000,"location":{"x":10,"y":10},"direction":"Down","kind":0}),
        );
        packet(
            &mut adapter,
            lifecycle,
            json!({"objectId":1000,"dead":false,"location":{"x":10,"y":10}}),
        );
        positive_owner_health(&mut adapter, 50);
        let mut revived = old;
        adapter.apply_authoritative_overlay(&mut revived);
        assert_eq!(revived["playerHp"], json!(100), "{lifecycle}");
        assert_eq!(revived["entities"][0]["hp"], json!(100), "{lifecycle}");
        assert_eq!(revived["entities"][0]["dead"], json!(false), "{lifecycle}");
    }
}

#[test]
fn health_zero_new_death_after_revive_wins_over_old_alive_snapshot_and_health() {
    let mut adapter = NativeGameplayAdapter::default();
    let live = payload();
    adapter.observe_world_snapshot(&live);
    packet(&mut adapter, "Death", json!({"location":{"x":10,"y":10}}));
    packet(&mut adapter, "Revived", json!({}));
    positive_owner_health(&mut adapter, 50);
    let mut revived = typed_zero(true);
    adapter.apply_authoritative_overlay(&mut revived);
    assert_eq!(revived["playerHp"], json!(100));
    assert_eq!(revived["entities"][0]["dead"], json!(false));

    packet(&mut adapter, "Death", json!({"location":{"x":10,"y":10}}));
    positive_owner_health(&mut adapter, 100);
    let mut stale_live = live;
    adapter.apply_authoritative_overlay(&mut stale_live);
    assert_eq!(stale_live["playerHp"], json!(0));
    assert_eq!(stale_live["entities"][0]["hp"], json!(0));
    assert_eq!(stale_live["entities"][0]["dead"], json!(true));
    assert_eq!(
        stale_live["entities"][0]["_nativeAnimationAction"],
        json!("die")
    );
    assert_eq!(adapter.authoritative_player_dead, Some(true));
}

#[test]
fn health_zero_acknowledged_new_life_cannot_mask_a_later_typed_death() {
    for lifecycle in ["Revived", "ObjectRevived", "ObjectPlayer"] {
        let mut adapter = NativeGameplayAdapter::default();
        let old = typed_zero(true);
        adapter.observe_world_snapshot(&old);
        packet(
            &mut adapter,
            lifecycle,
            json!({"objectId":1000,"dead":false,"location":{"x":10,"y":10}}),
        );
        positive_owner_health(&mut adapter, 50);
        // A delayed old-life snapshot must still yield to the new life packet.
        let mut stale = old;
        adapter.apply_authoritative_overlay(&mut stale);
        adapter.observe_world_snapshot(&stale);
        assert_eq!(stale["playerHp"], json!(100), "{lifecycle}");
        assert_eq!(stale["entities"][0]["dead"], json!(false), "{lifecycle}");

        // Only a raw typed live snapshot acknowledges the new incarnation;
        // the overlaid old snapshot above is not an acknowledgement.
        let mut acknowledged = payload();
        acknowledged["playerHp"] = json!(100);
        acknowledged["entities"][0]["hp"] = json!(100);
        adapter.observe_world_snapshot_life(&acknowledged);
        adapter.apply_authoritative_overlay(&mut acknowledged);
        adapter.observe_world_snapshot(&acknowledged);

        let mut newer_dead = typed_zero(true);
        adapter.apply_authoritative_overlay(&mut newer_dead);
        assert_eq!(newer_dead["playerHp"], json!(0), "{lifecycle}");
        assert_eq!(newer_dead["entities"][0]["hp"], json!(0), "{lifecycle}");
        assert_eq!(
            newer_dead["entities"][0]["dead"],
            json!(true),
            "{lifecycle}"
        );
    }
}

#[test]
fn health_zero_acknowledged_new_life_does_not_turn_later_exact_zero_into_positive_hp() {
    let mut adapter = NativeGameplayAdapter::default();
    adapter.observe_world_snapshot(&typed_zero(true));
    packet(&mut adapter, "Revived", json!({}));
    positive_owner_health(&mut adapter, 50);
    let mut acknowledged = payload();
    adapter.observe_world_snapshot_life(&acknowledged);
    adapter.apply_authoritative_overlay(&mut acknowledged);
    adapter.observe_world_snapshot(&acknowledged);
    let mut newer_zero = typed_zero(false);
    adapter.apply_authoritative_overlay(&mut newer_zero);
    assert_eq!(newer_zero["playerHp"], json!(0));
    assert_eq!(newer_zero["entities"][0]["hp"], json!(0));
    assert_eq!(newer_zero["entities"][0]["dead"], json!(false));
}

#[test]
fn health_zero_cached_packet_replay_cannot_acknowledge_a_revived_incarnation() {
    for lifecycle in ["Revived", "ObjectRevived", "ObjectPlayer"] {
        let mut adapter = NativeGameplayAdapter::default();
        let old = typed_zero(true);
        adapter.observe_world_snapshot(&old);
        packet(
            &mut adapter,
            lifecycle,
            json!({"objectId":1000,"dead":false,"location":{"x":10,"y":10}}),
        );
        positive_owner_health(&mut adapter, 50);
        let mut cached = old.clone();
        adapter.observe_world_snapshot_life(&cached);
        adapter.apply_authoritative_overlay(&mut cached);
        adapter.observe_world_snapshot(&cached);
        assert_eq!(cached["playerHp"], json!(100), "{lifecycle}");

        // Match gateway.rs packet-first replay: this cache is already overlaid,
        // so replay must project it without observing raw snapshot life.
        adapter.apply_authoritative_overlay(&mut cached);
        adapter.observe_world_snapshot(&cached);
        let mut delayed_old = old;
        adapter.observe_world_snapshot_life(&delayed_old);
        adapter.apply_authoritative_overlay(&mut delayed_old);
        assert_eq!(delayed_old["playerHp"], json!(100), "{lifecycle}");
        assert_eq!(
            delayed_old["entities"][0]["dead"],
            json!(false),
            "{lifecycle}"
        );
    }
}
