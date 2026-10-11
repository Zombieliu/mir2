use super::*;
use serde_json::json;

#[test]
fn native_five_class_status_packets_keep_real_fields_and_monotonic_effect_sequence() {
    let mut bridge = NativeGameplayAdapter::default();
    for (packet, payload) in [
        (
            "SetElemental",
            json!({"objectId":7,"enabled":true,"casted":false,"value":120,"elementType":1,"expLast":200}),
        ),
        (
            "SetConcentration",
            json!({"objectId":7,"enabled":true,"interrupted":true}),
        ),
        (
            "SetBindingShot",
            json!({"objectId":7,"enabled":true,"value":80}),
        ),
        ("RemoveDelayedExplosion", json!({"objectId":8})),
    ] {
        let before = bridge.effect_sequence;
        assert!(bridge.observe_packet(&PacketEvent::Other {
            packet: packet.into(),
            payload: payload.clone()
        }));
        let effect = bridge.effect_events.back().unwrap();
        assert_eq!(effect.sequence, before + 1);
        assert_eq!(effect.packet, packet);
        assert_eq!(effect.payload, payload);
    }
}

#[test]
fn native_magic_retains_only_known_authoritative_actor_context() {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.actor_sound_contexts.insert(
        7,
        json!({"objectId":7,"kind":"player","class":"Archer","gender":"Male","buffs":[122]}),
    );
    bridge.observe_packet(&PacketEvent::Other { packet:"ObjectMagic".into(), payload:json!({
        "objectId":7,"spell":"CrippleShot","targetId":8,"target":{"x":5,"y":6},"direction":"Right","cast":true,"level":3
    }) });
    let effect = bridge.effect_events.back().unwrap();
    assert_eq!(effect.payload["_nativeAttacker"]["class"], "Archer");
    assert_eq!(effect.payload["_nativeAttacker"]["buffs"], json!([122]));
    assert!(effect.payload.get("_nativeTarget").is_none());
}

#[test]
fn native_double_slash_body_hint_comes_from_actual_attack_spell() {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.latest_player_object_id = Some(7);
    bridge.zone_entities.insert(
        7,
        serde_json::from_value(json!({"objectId":7,"kind":"selfPlayer","class":"Assassin"}))
            .unwrap(),
    );
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectAttack".into(),
        payload: json!({
            "objectId":7,"spell":92,"level":0,"direction":"Right","location":{"x":5,"y":6}
        }),
    });
    let owner = &bridge.zone_entities[&7];
    assert_eq!(owner["_nativeAnimationAction"], "attack1");
    assert_eq!(owner["_nativeAnimationSpell"], 92);
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectAttack".into(),
        payload: json!({
            "objectId":7,"spell":0,"direction":"Right","location":{"x":5,"y":6}
        }),
    });
    assert_eq!(bridge.zone_entities[&7]["_nativeAnimationSpell"], 0);
}

#[test]
fn native_element_status_marks_the_actual_self_even_when_objectplayer_kind_is_player() {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.latest_player_object_id = Some(7);
    bridge
        .actor_sound_contexts
        .insert(7, json!({"kind":"player","class":"Archer","gender":"Male"}));
    bridge
        .actor_sound_contexts
        .insert(8, json!({"kind":"player","class":"Archer","gender":"Male"}));
    for id in [7, 8] {
        bridge.observe_packet(&PacketEvent::Other {
            packet: "SetElemental".into(),
            payload: json!({
                "objectId":id,"enabled":true,"casted":true,"value":100,"elementType":3,"expLast":200
            }),
        });
        let context = &bridge.effect_events.back().unwrap().payload["_nativeAttacker"];
        assert_eq!(
            context.get("_nativeSelf").and_then(Value::as_bool),
            (id == 7).then_some(true)
        );
    }
}

#[test]
fn native_backstep_uses_real_jump_actor_and_preserves_source_facing() {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.latest_player_object_id = Some(7);
    bridge.actor_sound_contexts.insert(
        7,
        json!({"kind":"player","class":"Archer","gender":"Female"}),
    );
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "UserBackStep".into(),
        payload: json!({
            "location":{"x":8,"y":10},"direction":"Right","distance":2
        })
    }));
    let effect = bridge.effect_events.back().unwrap();
    assert_eq!(effect.packet, "UserBackStep");
    assert_eq!(effect.payload["objectId"], 7);
    assert_eq!(effect.payload["_nativeAttacker"]["gender"], "Female");
    assert_eq!(
        bridge
            .authoritative_player_animation
            .as_ref()
            .unwrap()
            .action,
        "jump"
    );
    assert_eq!(
        bridge
            .authoritative_player_transform
            .as_ref()
            .unwrap()
            .direction.as_deref(),
        Some("Right")
    );
}

#[test]
fn native_dark_body_disappearance_records_real_tile_before_removing_clone() {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectPlayer".into(),
        payload: json!({
            "objectId":77,"name":"AssassinClone","class":"Assassin","gender":"Male",
            "location":{"x":15,"y":20},"direction":"Left"
        }),
    });
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectDied".into(),
        payload: json!({
            "objectId":77,"kind":2,"location":{"x":15,"y":20},"direction":"Left"
        })
    }));
    assert!(!bridge.zone_entities.contains_key(&77));
    assert!(bridge.zone_tombstones.contains(&77));
    let effect = bridge.effect_events.back().unwrap();
    assert_eq!(effect.packet, "ObjectDied");
    assert_eq!(effect.payload["kind"], 2);
    assert_eq!(effect.payload["_nativeTarget"]["class"], "Assassin");
    assert_eq!(effect.payload["location"], json!({"x":15,"y":20}));
}

fn elemental_animation_fixture() -> NativeGameplayAdapter {
    let mut bridge = NativeGameplayAdapter::default();
    bridge.latest_player_object_id = Some(7);
    bridge.authoritative_player_transform = Some(AuthoritativePlayerTransform {
        x: 5,
        y: 6,
        direction: Some("Right".into()),
    });
    for (id, class) in [(7, "Archer"), (8, "Archer"), (9, "Warrior")] {
        bridge.actor_sound_contexts.insert(
            id,
            json!({
                "objectId":id,"kind":"player","class":class,"gender":"Male","dead":false
            }),
        );
    }
    bridge
}

fn observe_element_status(
    bridge: &mut NativeGameplayAdapter,
    id: u32,
    enabled: bool,
    casted: bool,
) {
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "SetElemental".into(),
        payload: json!({"objectId":id,"enabled":enabled,"casted":casted,
            "value":100,"elementType":3,"expLast":200}),
    }));
}

fn observe_elemental_magic(bridge: &mut NativeGameplayAdapter, id: u32) {
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectMagic".into(),
        payload: json!({"objectId":id,"spell":"ElementalShot","targetId":10,
            "target":{"x":8,"y":6},"direction":"Right","cast":true,"level":3}),
    }));
}

#[test]
fn native_elemental_shot_pose_obeys_real_owner_and_observer_latch() {
    let mut bridge = elemental_animation_fixture();
    observe_elemental_magic(&mut bridge, 7);
    assert_eq!(bridge.zone_entities[&7]["_nativeAnimationAction"], "spell");
    for id in [7, 8, 9, 77] {
        observe_element_status(&mut bridge, id, true, true);
    }
    assert!(!bridge.elemental_animation_state.contains_key(&9));
    assert!(!bridge.elemental_animation_state.contains_key(&77));

    // Owner ignores Casted=true; observer consumes it once in SetAction.
    assert!(!bridge.elemental_animation_state[&7].casted);
    assert!(bridge.elemental_animation_state[&8].casted);
    observe_elemental_magic(&mut bridge, 7);
    assert_eq!(
        bridge.zone_entities[&7]["_nativeAnimationAction"],
        "attackRange2"
    );
    observe_elemental_magic(&mut bridge, 8);
    assert_eq!(bridge.zone_entities[&8]["_nativeAnimationAction"], "spell");
    assert!(!bridge.elemental_animation_state[&8].casted);
    observe_elemental_magic(&mut bridge, 8);
    assert_eq!(
        bridge.zone_entities[&8]["_nativeAnimationAction"],
        "attackRange2"
    );
    observe_element_status(&mut bridge, 7, false, false);
    observe_elemental_magic(&mut bridge, 7);
    assert_eq!(bridge.zone_entities[&7]["_nativeAnimationAction"], "spell");
    observe_elemental_magic(&mut bridge, 9);
    assert_eq!(bridge.zone_entities[&9]["_nativeAnimationAction"], "spell");

    // Compact owner Magic shares the same typed state without an ObjectMagic echo.
    observe_element_status(&mut bridge, 7, true, true);
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "Magic".into(),
        payload: json!({"spell":"ElementalShot","targetId":10,
            "target":{"x":8,"y":6},"cast":true,"level":3}),
    }));
    assert_eq!(
        bridge
            .authoritative_player_animation
            .as_ref()
            .unwrap()
            .action,
        "attackRange2"
    );
}

#[test]
fn native_elemental_aoi_restore_cannot_replay_stale_state_or_infer_from_snapshot() {
    let mut bridge = elemental_animation_fixture();
    let arrival = json!({"objectId":8,"name":"Observer","class":"Archer","gender":"Male",
        "location":{"x":5,"y":6},"direction":"Right",
        "elementOrbEffect":3,"elementOrbLevel":100,"elementOrbMax":200});
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectPlayer".into(),
        payload: arrival.clone(),
    }));
    observe_elemental_magic(&mut bridge, 8);
    assert_eq!(
        bridge.zone_entities[&8]["_nativeAnimationAction"],
        "attackRange2"
    );
    observe_element_status(&mut bridge, 8, false, false);
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectPlayer".into(),
        payload: arrival.clone(),
    });
    observe_elemental_magic(&mut bridge, 8);
    assert_eq!(bridge.zone_entities[&8]["_nativeAnimationAction"], "spell");
    assert!(bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectRemove".into(),
        payload: json!({"objectId":8}),
    }));
    assert!(!bridge.elemental_animation_state.contains_key(&8));
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectPlayer".into(),
        payload: arrival,
    });
    observe_elemental_magic(&mut bridge, 8);
    assert_eq!(
        bridge.zone_entities[&8]["_nativeAnimationAction"],
        "attackRange2"
    );

    bridge.observe_world_snapshot(&json!({"playerObjectId":7,"entities":[
        {"objectId":77,"kind":"player","class":"Archer","elementOrbEffect":4,
            "elementOrbLevel":200,"elementOrbMax":200}
    ]}));
    observe_elemental_magic(&mut bridge, 77);
    assert_eq!(bridge.zone_entities[&77]["_nativeAnimationAction"], "spell");
}

#[test]
fn native_elemental_animation_state_is_revoked_at_identity_map_and_life_boundaries() {
    for boundary in 0..7 {
        let mut bridge = elemental_animation_fixture();
        observe_element_status(&mut bridge, 7, true, false);
        assert!(bridge.elemental_animation_state[&7].enabled);
        match boundary {
            0 => bridge.clear_zone_state(),
            1 => bridge.clear_zone_state_for_map_change(),
            2 => {
                bridge.observe_packet(&PacketEvent::Other {
                    packet: "LogOutSuccess".into(),
                    payload: json!({}),
                });
            }
            3 => bridge.set_generation(2),
            4 => bridge.observe_world_snapshot(&json!({"playerObjectId":8,"entities":[]})),
            5 => {
                bridge.observe_packet(&PacketEvent::UserInformation(
                    crate::native_protocol::UserInformation {
                        object_id: Some(7),
                        name: Some("OtherCharacter".into()),
                        class_name: Some("Archer".into()),
                        gender: Some("Male".into()),
                        level: Some(20),
                        payload: json!({"objectId":7}),
                    },
                ));
            }
            _ => {
                bridge.observe_packet(&PacketEvent::Other {
                    packet: "Death".into(),
                    payload: json!({}),
                });
            }
        }
        assert!(
            bridge.elemental_animation_state.is_empty(),
            "boundary={boundary}"
        );
        assert_eq!(
            bridge.source_object_magic_animation_action(&json!({
                "objectId":7,"spell":"ElementalShot"
            })),
            "spell",
            "boundary={boundary}"
        );
    }
    let mut bridge = elemental_animation_fixture();
    observe_element_status(&mut bridge, 8, true, false);
    bridge.observe_packet(&PacketEvent::Other {
        packet: "ObjectDied".into(),
        payload: json!({
            "objectId":8,"kind":0,"location":{"x":5,"y":6},"direction":"Right"
        }),
    });
    assert!(!bridge.elemental_animation_state.contains_key(&8));
    observe_element_status(&mut bridge, 8, true, false);
    assert!(!bridge.elemental_animation_state.contains_key(&8));
}

#[test]
fn native_double_slash_enriched_raw_sequence_matches_actual_owner_and_observer_body() {
    for object_id in [7, 8] {
        let mut bridge = NativeGameplayAdapter::default();
        let mut world = json!({"playerObjectId":7,"entities":[
            {"objectId":7,"kind":"selfPlayer","class":"Assassin","x":4,"y":6,"direction":"Right"},
            {"objectId":8,"kind":"player","class":"Assassin","x":5,"y":6,"direction":"Right"}
        ]});
        bridge.observe_world_snapshot(&world);
        bridge.authoritative_player_transform = Some(AuthoritativePlayerTransform {
            x: 4,
            y: 6,
            direction: Some("Right".into()),
        });
        bridge.observe_packet(&PacketEvent::Other {
            packet: "UserLocation".into(),
            payload: json!({
                "location":{"x":5,"y":6},"direction":"Right"
            }),
        });
        bridge.observe_packet(&PacketEvent::Other {
            packet: "ObjectPlayer".into(),
            payload: json!({
                "objectId":object_id,"name":"Assassin","class":"Assassin","gender":"Male",
                "location":{"x":5,"y":6},"direction":"Right"
            }),
        });
        bridge.observe_packet(&PacketEvent::Other {packet:"ObjectAttack".into(),payload:json!({
            "objectId":object_id,"spell":92,"level":0,"direction":"Right","location":{"x":5,"y":6}
        })});
        let overlay = &bridge.zone_entities[&object_id];
        let raw_sequence = overlay["_nativeAnimationSequence"].as_u64().unwrap();
        assert!(raw_sequence > 0);
        let effect = bridge.effect_events.back().unwrap();
        assert_eq!(effect.packet, "ObjectAttack");
        assert_eq!(
            effect.payload["_nativeAttacker"]["_nativeAnimationSequence"],
            raw_sequence
        );
        assert_eq!(
            effect.payload["_nativeAttacker"]["_nativeAnimationSpell"],
            92
        );
        assert_eq!(
            effect.payload["_nativeAttacker"]["_nativeAnimationAction"],
            "attack1"
        );
        bridge.apply_authoritative_overlay(&mut world);
        let actual = world["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|actor| actor["objectId"].as_u64() == Some(u64::from(object_id)))
            .unwrap();
        assert_eq!(actual["_nativeAnimationSequence"], raw_sequence);
        assert_eq!(actual["_nativeAnimationSpell"], 92);
        assert_eq!(actual["_nativeAnimationAction"], "attack1");
    }
}
