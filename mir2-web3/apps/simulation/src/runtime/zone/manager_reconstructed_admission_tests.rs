//! Trusted reconstructed-Session admission is distinct from cold world restore.
use super::*;
use crate::{
    ActiveSessionIdentity, PlayerDropCommittedEffect, PlayerDropEffectState,
    PlayerDropOperationBinding, PlayerDropOutcome, PlayerDropWorldIntent,
};
use mir2_protocol::{MirClass, MirGender};
use serde_json::Value;

fn join(name: &str, object_id: u32, x: i32, y: i32) -> ZoneJoin {
    ZoneJoin {
        session_id: SessionId::new(name),
        account_id: format!("account-{name}"),
        character_index: object_id as i32,
        object_id,
        name: name.into(),
        class: MirClass::Taoist,
        gender: MirGender::Male,
        level: 40,
        hp: 1000,
        max_hp: 1000,
        mp: 1000,
        map_file_name: "0".into(),
        position: Point { x, y },
        direction: MirDirection::Right,
        chat_profile: super::super::ZoneChatProfile {
            pet_mode: 4,
            attack_mode: 5,
            ..Default::default()
        },
        combat_stats: Default::default(),
    }
}
fn identity(join: &ZoneJoin) -> ActiveSessionIdentity {
    ActiveSessionIdentity {
        account_id: join.account_id.clone(),
        character_index: join.character_index,
        character_name: join.name.clone(),
    }
}
fn fixture() -> (ZoneManager, ZoneJoin) {
    let mut manager = ZoneManager::new();
    manager
        .bind_logical_world(
            LogicalWorldId::legacy_execution_domain("reconstructed-admission").unwrap(),
        )
        .unwrap();
    // Strict cold restore reloads the signed map's collision identity, which
    // participates in its root. A synthetic unbounded map is not that source.
    assert!(manager.install_empty_zone(ZoneRuntime::new(ZoneKey::for_map("0"))));
    let owner = join("owner", 101, 332, 270);
    manager.join(owner.clone());
    manager.join(join("peer", 102, 332, 271));
    manager.handle(ZoneCommand::Turn {
        session_id: owner.session_id.clone(),
        direction: MirDirection::Down,
        now_ms: 100,
    });
    (manager, owner)
}
fn world_image(manager: &ZoneManager) -> Value {
    let mut world: Value = serde_json::from_slice(
        &manager
            .zone(&ZoneKey::for_map("0"))
            .unwrap()
            .checkpoint_bytes()
            .unwrap(),
    )
    .unwrap();
    let fields = world.as_object_mut().unwrap();
    // Fresh online presence changes its own root; all retained business/world
    // fields, including custody, player life and pending queues remain exact.
    fields.remove("state_root");
    fields.remove("online_presence");
    world
}
fn add_drop(manager: &mut ZoneManager) -> PlayerDropWorldIntent {
    let key = ZoneKey::for_map("0");
    let binding = PlayerDropOperationBinding::new(
        "reconstructed-drop".into(),
        "source".into(),
        "source-account".into(),
        0,
        "owner-group".into(),
        "original-owner".into(),
        1,
        7,
    )
    .unwrap();
    let intent = PlayerDropWorldIntent::new(
        binding,
        key.clone(),
        Point { x: 333, y: 270 },
        1_000,
        10_000,
        PlayerDropOutcome::Gold { amount: 7 },
    )
    .unwrap();
    let proof =
        PlayerDropCommittedEffect::for_zone_test(intent.clone(), PlayerDropEffectState::Live);
    manager
        .apply_committed_player_drop_for_key(&key, &proof, 2_000)
        .unwrap();
    intent
}

#[test]
fn reconstructed_admission_is_fresh_idempotent_and_preserves_retained_world_and_custody() {
    let (mut original, owner) = fixture();
    let intent = add_drop(&mut original);
    let old_proof = original
        .online_owner_proof_for_session(&owner.session_id)
        .unwrap();
    let mut restored =
        ZoneManager::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
    assert!(restored
        .online_owner_proof_for_session(&owner.session_id)
        .is_none());
    assert!(!restored.online_owner_proof_is_current(&old_proof));
    let before = world_image(&restored);
    let admissions = restored.admitted_action_count();
    let last = restored.last_admitted_action().cloned();
    let retained = (
        restored.player_transform(&owner.session_id),
        restored.player_vitals(&owner.session_id),
        restored.player_life_generation(&owner.session_id),
        restored.player_last_seen_move_seq(&owner.session_id),
    );
    restored
        .readmit_restored_player(&identity(&owner), &owner)
        .unwrap();
    let fresh = restored
        .online_owner_proof_for_session(&owner.session_id)
        .unwrap();
    assert_ne!(fresh, old_proof);
    assert!(restored.online_owner_proof_matches_session(&fresh, &owner.session_id));
    assert!(restored
        .online_owner_proof_for_session(&SessionId::new("peer"))
        .is_none());
    assert_eq!(world_image(&restored), before);
    assert_eq!(restored.admitted_action_count(), admissions);
    assert_eq!(restored.last_admitted_action(), last.as_ref());
    assert_eq!(
        (
            restored.player_transform(&owner.session_id),
            restored.player_vitals(&owner.session_id),
            restored.player_life_generation(&owner.session_id),
            restored.player_last_seen_move_seq(&owner.session_id)
        ),
        retained
    );
    assert!(matches!(
        restored
            .query_player_drop_effect_for_key(
                intent.zone(),
                intent.world_effect_id(),
                intent.intent_digest()
            )
            .unwrap(),
        Some(super::super::PlayerDropCustodyState::Live { .. })
    ));
    let admitted = restored.checkpoint_bytes().unwrap();
    restored
        .readmit_restored_player(&identity(&owner), &owner)
        .unwrap();
    assert_eq!(
        restored.online_owner_proof_for_session(&owner.session_id),
        Some(fresh)
    );
    assert_eq!(restored.checkpoint_bytes().unwrap(), admitted);
}

#[test]
fn reconstructed_admission_rejects_private_route_object_and_retained_name_mismatches_atomically() {
    let (original, owner) = fixture();
    let restored = ZoneManager::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
    let mut cases = Vec::new();
    let mut changed = owner.clone();
    changed.account_id.push_str("-foreign");
    cases.push((identity(&owner), changed));
    let mut changed = owner.clone();
    changed.character_index += 1;
    cases.push((identity(&owner), changed));
    let mut changed = owner.clone();
    changed.object_id += 99;
    cases.push((identity(&owner), changed));
    let mut changed = owner.clone();
    changed.map_file_name = "d024".into();
    cases.push((identity(&owner), changed));
    let mut changed = owner.clone();
    changed.session_id = SessionId::new("not-reconstructed");
    cases.push((identity(&owner), changed));
    let mut changed = owner.clone();
    changed.name.push_str("-another-incarnation");
    cases.push((identity(&changed), changed));
    let mut changed = owner.clone();
    changed.account_id.push_str("-wrong-retained");
    cases.push((identity(&changed), changed));
    for (trusted, join) in cases {
        let mut candidate = restored.transaction_fork();
        let before = candidate.checkpoint_bytes().unwrap();
        assert!(candidate.readmit_restored_player(&trusted, &join).is_err());
        assert_eq!(candidate.checkpoint_bytes().unwrap(), before);
        assert!(candidate
            .online_owner_proof_for_session(&owner.session_id)
            .is_none());
    }
}

#[test]
fn reconstructed_admission_never_turns_serialized_online_identity_into_authority() {
    let (mut original, owner) = fixture();
    let first = original
        .online_owner_proof_for_session(&owner.session_id)
        .unwrap();
    original
        .readmit_restored_player(&identity(&owner), &owner)
        .unwrap();
    assert_eq!(
        original.online_owner_proof_for_session(&owner.session_id),
        Some(first.clone())
    );
    let mut restored =
        ZoneManager::restore_checkpoint(&original.checkpoint_bytes().unwrap()).unwrap();
    assert!(!restored.online_owner_proof_is_current(&first));
    restored
        .readmit_restored_player(&identity(&owner), &owner)
        .unwrap();
    let second = restored
        .online_owner_proof_for_session(&owner.session_id)
        .unwrap();
    assert_ne!(second, first);
    let cold_again =
        ZoneManager::restore_checkpoint(&restored.checkpoint_bytes().unwrap()).unwrap();
    assert!(cold_again
        .online_owner_proof_for_session(&owner.session_id)
        .is_none());
    assert!(!cold_again.online_owner_proof_is_current(&first));
    assert!(!cold_again.online_owner_proof_is_current(&second));
}

#[test]
fn manager_transaction_fork_retains_live_pet_and_monster_state_and_remains_isolated() {
    let (mut original, owner) = fixture();
    let key = ZoneKey::for_map("0");
    {
        let zone = original.zones.get_mut(&key).unwrap();
        zone.prepare_presence_publication_threat_clock(&owner.session_id);
        zone.prepare_presence_publication_pet_target(&owner.session_id);
        zone.prepare_presence_publication_health_and_poison(&owner.session_id);
    }
    original.refresh_online_presence();
    let before = original.checkpoint_bytes().unwrap();
    let proof = original
        .online_owner_proof_for_session(&owner.session_id)
        .unwrap();
    let pets = original.capture_player_saved_pets(&owner.session_id, 100);
    let mut fork = original.transaction_fork();
    assert_eq!(fork.checkpoint_bytes().unwrap(), before);
    assert_eq!(fork.capture_player_saved_pets(&owner.session_id, 100), pets);
    assert!(fork.online_owner_proof_is_current(&proof));
    let original_out = original.tick_all(900);
    let fork_out = fork.tick_all(900);
    assert_eq!(fork_out, original_out);
    assert_eq!(
        fork.checkpoint_bytes().unwrap(),
        original.checkpoint_bytes().unwrap()
    );
    fork.handle(ZoneCommand::Leave {
        session_id: owner.session_id.clone(),
    });
    assert!(original.online_owner_proof_is_current(&proof));
    assert!(!fork.online_owner_proof_is_current(&proof));
    assert_eq!(original.zone(&key).unwrap().player_count(), 2);
    assert_eq!(fork.zone(&key).unwrap().player_count(), 1);
}
