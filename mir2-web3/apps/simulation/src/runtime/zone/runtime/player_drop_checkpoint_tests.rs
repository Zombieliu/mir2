use super::*;
use crate::config::{GroundDropLootSnapshot, GroundDropSnapshot};
use crate::player_drop::{
    PlayerDropCommittedEffect, PlayerDropEffectState, PlayerDropOperationBinding,
    PlayerDropOutcome, PlayerDropWorldIntent,
};

fn intent(key: &ZoneKey) -> PlayerDropWorldIntent {
    let binding = PlayerDropOperationBinding::new(
        "checkpoint-effect".into(),
        "source-rpc".into(),
        "source-account".into(),
        0,
        crate::zone_id_for_key(key),
        "old-owner".into(),
        7,
        8,
    )
    .unwrap();
    PlayerDropWorldIntent::new(
        binding,
        key.clone(),
        Point { x: 10, y: 10 },
        1_000,
        10_000,
        PlayerDropOutcome::Gold { amount: 25 },
    )
    .unwrap()
}
fn legacy_drop(object_id: u32) -> GroundDropSnapshot {
    GroundDropSnapshot {
        object_id,
        name: "Gold".into(),
        name_colour_argb: -1,
        icon: 0,
        x: 3,
        y: 4,
        quantity: 1,
        source_monster: String::new(),
        owner_object_id: None,
        ownership_remaining_ticks: None,
        loot: GroundDropLootSnapshot::Gold { amount: 4 },
    }
}

#[test]
fn player_drop_checkpoint_v9_commits_identity_registry_and_requires_complete_custody() {
    let key = ZoneKey::for_map("player-drop-checkpoint");
    let mut zone = ZoneRuntime::new(key.clone());
    let intent = intent(&key);
    let evidence =
        PlayerDropCommittedEffect::for_zone_test(intent.clone(), PlayerDropEffectState::Live);
    let id = zone
        .apply_committed_player_drop(&evidence, 2_000)
        .unwrap()
        .object_id
        .unwrap();
    let bytes = zone.checkpoint_bytes().unwrap();
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(original["version"], 9);
    assert!(ZoneRuntime::restore_checkpoint(&bytes).is_ok());
    for field in ["metadata", "intent", "state", "missing", "malformed"] {
        let mut tamper = original.clone();
        match field {
            "metadata" => {
                tamper["ground_drops"][id.to_string()]["player_drop_effect"]["expires_at_unix_ms"] =
                    100_000.into()
            }
            "intent" => {
                tamper["player_drop_custody"]["effects"][intent.world_effect_id()]["intent"]
                    ["expires_at_unix_ms"] = 100_000.into()
            }
            "state" => {
                tamper["player_drop_custody"]["effects"][intent.world_effect_id()]["state"] =
                    "Consumed".into()
            }
            "missing" => {
                tamper
                    .as_object_mut()
                    .unwrap()
                    .remove("player_drop_custody");
            }
            "malformed" => {
                tamper["player_drop_custody"] =
                    serde_json::json!({"effects":{},"reservations":"forged"})
            }
            _ => unreachable!(),
        }
        assert!(
            ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&tamper).unwrap()).is_err(),
            "{field}"
        );
    }
    let public_snapshot = serde_json::to_value(&zone.ground_drops[&id].drop).unwrap();
    assert!(public_snapshot.get("player_drop_effect").is_none());
    assert!(public_snapshot.get("world_effect_id").is_none());
}

#[test]
fn player_drop_legacy_v8_strips_all_forward_custody_even_malformed_and_preserves_random_cursor() {
    let key = ZoneKey::for_map("player-drop-legacy-v8");
    let mut zone = ZoneRuntime::new(key);
    zone.combat_random = AuthoritativeCombatRandom::from_seed([7; 32]);
    zone.combat_random.next_u32().unwrap();
    let stored = zone.new_zone_ground_drop(legacy_drop(701), None).unwrap();
    zone.ground_drops.insert(701, stored);
    let stored = zone.new_zone_ground_drop(legacy_drop(702), None).unwrap();
    let session_id = SessionId::new("legacy-claimant");
    let ticket = GroundDropClaimTicket {
        player_drop_claimant: None,
        player_drop_effect: None,
        claim_id: zone.allocate_ground_drop_claim_id().unwrap(),
        object_id: 702,
        drop_generation: stored.drop_generation,
        payload_digest: stored.payload_digest.clone(),
        idempotency_key: canonical_ground_drop_claim_idempotency_key(
            &zone.key,
            702,
            stored.drop_generation,
            &stored.payload_digest,
        ),
        session_id: session_id.clone(),
        owner_object_id: None,
        drop: stored.drop.clone(),
    };
    let custody = zone.capture_ground_drop_custody(&stored);
    zone.claimed_ground_drops.insert(
        702,
        ZoneGroundDropClaim {
            session_id,
            drop: stored.drop,
            ticket: Some(ticket),
            custody: Some(custody),
        },
    );
    zone.removed_object_ids.insert(702);
    let mut value: serde_json::Value =
        serde_json::from_slice(&zone.checkpoint_bytes().unwrap()).unwrap();
    value["version"] = 8.into();
    value["state_root"] = zone
        .canonical_state_root_for_schema(
            8,
            LEGACY_V8_CANONICAL_ZONE_STATE_DOMAIN,
            &zone.native_monsters,
            &zone.pending_native_hits,
        )
        .unwrap()
        .into();
    value["player_drop_custody"] =
        serde_json::json!({"effects":"malformed unauthenticated data","reservations":1});
    value["ground_drops"]["701"]["player_drop_effect"] =
        serde_json::json!({"world_effect_id":"forged"});
    value["claimed_ground_drops"]["702"]["ticket"]["player_drop_effect"] = 17.into();
    value["claimed_ground_drops"]["702"]["custody"]["player_drop_effect"] = "malformed".into();
    let restored = ZoneRuntime::restore_checkpoint(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(restored.player_drop_effects.is_empty());
    assert!(restored.player_drop_reservations.is_empty());
    assert!(restored.ground_drops[&701].player_drop_effect.is_none());
    assert!(restored.claimed_ground_drops[&702]
        .ticket
        .as_ref()
        .unwrap()
        .player_drop_effect
        .is_none());
    assert!(restored.claimed_ground_drops[&702]
        .custody
        .as_ref()
        .unwrap()
        .player_drop_effect
        .is_none());
    assert_eq!(
        restored.combat_random, zone.combat_random,
        "v8 already authenticated the business random stream"
    );
}

#[test]
fn player_drop_old_checkpoint_cannot_revive_a_canonically_consumed_effect() {
    let key = ZoneKey::for_map("player-drop-consumed-old-checkpoint");
    let intent = intent(&key);
    let live =
        PlayerDropCommittedEffect::for_zone_test(intent.clone(), PlayerDropEffectState::Live);
    let mut zone = ZoneRuntime::new(key);
    zone.apply_committed_player_drop(&live, 2_000).unwrap();
    let stale_checkpoint = zone.checkpoint_bytes().unwrap();
    let consumed =
        PlayerDropCommittedEffect::for_zone_test(intent.clone(), PlayerDropEffectState::Consumed);
    let mut recovered = ZoneRuntime::restore_checkpoint(&stale_checkpoint).unwrap();
    assert_eq!(recovered.ground_drop_count(), 1);
    recovered
        .reconcile_committed_player_drops(&[consumed], 3_000)
        .unwrap();
    assert_eq!(recovered.ground_drop_count(), 0);
    assert_eq!(
        recovered.player_drop_effect_state(intent.world_effect_id()),
        Some(crate::runtime::zone::PlayerDropCustodyState::Consumed)
    );
    assert_eq!(
        recovered
            .apply_committed_player_drop(&live, 3_001)
            .unwrap()
            .object_id,
        None
    );
    let recovered_again =
        ZoneRuntime::restore_checkpoint(&recovered.checkpoint_bytes().unwrap()).unwrap();
    assert_eq!(recovered_again.ground_drop_count(), 0);
    assert_eq!(
        recovered_again.player_drop_effect_state(intent.world_effect_id()),
        Some(crate::runtime::zone::PlayerDropCustodyState::Consumed)
    );
}
