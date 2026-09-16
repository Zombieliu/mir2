use super::shared_relationships::bank_mentor_gain;
use crate::config::{Stage5FriendIdentity, Stage5MentorState};

fn student() -> Stage5MentorState {
    let mut state = Stage5MentorState::default();
    state.partner_identity = Some(Stage5FriendIdentity {
        account_id: "teacher".into(),
        character_index: 7,
    });
    state.ledger.relationship_epoch = 9;
    state
}

#[test]
fn mentor_bank_receipt_retry_is_once_and_checkpoint_rollback_restores_sequence() {
    let mut state = student();
    assert!(bank_mentor_gain(&mut state, 250, Some("zone:kill:12")).unwrap());
    assert_eq!(state.ledger.bank_earned, 2);
    let persisted = serde_json::to_string(&state).unwrap();
    let mut restored: Stage5MentorState = serde_json::from_str(&persisted).unwrap();
    assert!(!bank_mentor_gain(&mut restored, 999, Some("zone:kill:12")).unwrap());
    assert_eq!(restored.ledger.bank_earned, 2);
    let checkpoint = restored.clone();
    bank_mentor_gain(&mut restored, 199, None).unwrap();
    assert_eq!(restored.ledger.local_event_sequence, 1);
    restored = checkpoint;
    assert_eq!(restored.ledger.local_event_sequence, 0);
    bank_mentor_gain(&mut restored, 300, None).unwrap();
    assert_eq!(restored.ledger.bank_earned, 5);
}

#[test]
fn mentor_bank_overflow_fails_without_consuming_receipt_or_sequence() {
    let mut state = student();
    state.ledger.bank_earned = i64::MAX as u64;
    let before = state.clone();
    assert!(bank_mentor_gain(&mut state, 100, None).is_err());
    assert_eq!(state, before);
    state.ledger.bank_earned = 0;
    state.ledger.local_event_sequence = u64::MAX;
    let before = state.clone();
    assert!(bank_mentor_gain(&mut state, 100, None).is_err());
    assert_eq!(state, before);
    state.is_mentor = true;
    assert!(!bank_mentor_gain(&mut state, 100, Some("teacher-gain")).unwrap());
}

fn accounting_fixture() -> (
    crate::SimulationConfig,
    Stage5FriendIdentity,
    Stage5FriendIdentity,
    crate::SharedMentorLiveCheckpoint,
) {
    let config = crate::SimulationConfig::default();
    let teacher = Stage5FriendIdentity {
        account_id: "teacher".into(),
        character_index: 7,
    };
    let pupil = Stage5FriendIdentity {
        account_id: "demo".into(),
        character_index: 0,
    };
    let mut record = config.default_character.clone();
    record.index = 7;
    record.name = "Teacher".into();
    record.level = 30;
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .insert("teacher".into(), crate::AccountRecord::new(record));
    config
        .commit_shared_mentor_mutation(&teacher, crate::SharedMentorMutation::TogglePermission, 1)
        .unwrap();
    config
        .commit_shared_mentor_mutation(
            &teacher,
            crate::SharedMentorMutation::Accept {
                student: pupil.clone(),
            },
            10,
        )
        .unwrap();
    let mut save = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    let mut state: crate::Stage5SystemsState =
        serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    bank_mentor_gain(&mut state.mentor, 300, Some("kill:stable:1")).unwrap();
    state.economy_projection_event_ids.insert("a".repeat(64));
    save.experience += 300;
    save.stage5_systems_json = Some(serde_json::to_string(&state).unwrap());
    let checkpoint = crate::SharedMentorLiveCheckpoint {
        identity: pupil.clone(),
        save,
    };
    (config, pupil, teacher, checkpoint)
}

#[test]
fn mentor_bank_transfer_saves_source_xp_marker_and_bank_in_one_transaction() {
    let (config, pupil, teacher, checkpoint) = accounting_fixture();
    let epoch = config
        .shared_mentor_profile_for(&pupil)
        .unwrap()
        .mentor
        .ledger
        .relationship_epoch;
    config
        .commit_shared_mentor_accounting(&pupil, epoch, &[checkpoint.clone()], None, 20, None)
        .unwrap();
    let source = config.account_store.lock().unwrap().accounts["demo"].saves[&0].clone();
    assert_eq!(source.experience, checkpoint.save.experience);
    let mut state: crate::Stage5SystemsState =
        serde_json::from_str(source.stage5_systems_json.as_deref().unwrap()).unwrap();
    assert!(state.economy_projection_event_ids.contains(&"a".repeat(64)));
    assert_eq!(state.mentor.ledger.bank_earned, 3);
    assert_eq!(state.mentor.ledger.bank_settled, 3);
    assert!(!bank_mentor_gain(&mut state.mentor, 300, Some("kill:stable:1")).unwrap());
    assert_eq!(
        config
            .shared_mentor_profile_for(&teacher)
            .unwrap()
            .mentor
            .mentee_exp,
        3
    );
    assert!(config
        .commit_shared_mentor_accounting(&pupil, epoch, &[checkpoint], None, 21, None)
        .is_err());
    config
        .commit_shared_mentor_accounting(
            &pupil,
            epoch,
            &[],
            Some(crate::SharedMentorBreakReason::Manual),
            22,
            None,
        )
        .unwrap();
    let teacher_save = config.account_store.lock().unwrap().accounts["teacher"].saves[&7].clone();
    assert_eq!(teacher_save.experience, 3);
    assert_eq!(teacher_save.character.level, 30);
    let mentor = config.shared_mentor_profile_for(&teacher).unwrap().mentor;
    assert_eq!(mentor.mentee_exp, 0);
    assert_eq!(mentor.ledger.balance_credit, 3);
    assert_eq!(mentor.ledger.balance_applied, 3);
    assert!(config
        .commit_shared_mentor_accounting(
            &pupil,
            epoch,
            &[],
            Some(crate::SharedMentorBreakReason::Manual),
            23,
            None
        )
        .is_err());
    assert_eq!(
        config.account_store.lock().unwrap().accounts["teacher"].saves[&7].experience,
        3
    );
}

#[test]
fn mentor_source_checkpoint_and_both_banks_roll_back_on_persist_failure() {
    let (config, pupil, teacher, checkpoint) = accounting_fixture();
    let epoch = config
        .shared_mentor_profile_for(&pupil)
        .unwrap()
        .mentor
        .ledger
        .relationship_epoch;
    let before = serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    config
        .inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(config
        .commit_shared_mentor_accounting(
            &pupil,
            epoch,
            &[checkpoint.clone()],
            Some(crate::SharedMentorBreakReason::Manual),
            20,
            None
        )
        .is_err());
    assert_eq!(
        serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        config
            .shared_mentor_profile_for(&teacher)
            .unwrap()
            .mentor
            .mentee_exp,
        0
    );
    config
        .commit_shared_mentor_accounting(
            &pupil,
            epoch,
            &[checkpoint],
            Some(crate::SharedMentorBreakReason::Manual),
            21,
            None,
        )
        .unwrap();
    assert_eq!(
        config.account_store.lock().unwrap().accounts["teacher"].saves[&7].experience,
        3
    );
}
