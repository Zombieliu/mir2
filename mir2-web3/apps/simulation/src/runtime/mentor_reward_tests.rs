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


fn accounting_fixture() -> (crate::SimulationConfig, Stage5FriendIdentity, Stage5FriendIdentity) {
    let config = crate::SimulationConfig::default();
    let teacher = Stage5FriendIdentity { account_id: "teacher".into(), character_index: 7 };
    let pupil = Stage5FriendIdentity { account_id: "demo".into(), character_index: 0 };
    {
        let mut store = config.account_store.lock().unwrap();
        let source = store.accounts.get_mut("demo").unwrap();
        source.characters[0].level = 10;
        let save = source.saves.get_mut(&0).unwrap();
        save.character.level = 10;
        save.experience = 0;
        save.max_experience = super::leveling::crystal_max_experience_for_level(10);
        let mut record = config.default_character.clone();
        record.index = 7; record.name = "Teacher".into(); record.level = 20;
        store.accounts.insert("teacher".into(), crate::AccountRecord::new(record));
    }
    config.commit_shared_mentor_mutation(&teacher, crate::SharedMentorMutation::TogglePermission, 1).unwrap();
    config.commit_shared_mentor_mutation(&teacher, crate::SharedMentorMutation::Accept { student:pupil.clone() }, 10).unwrap();
    (config, pupil, teacher)
}
fn login(config: &crate::SimulationConfig, id: &Stage5FriendIdentity) -> crate::SimulationSession {
    use mir2_protocol::{ClientPacket,ServerPacket};
    let mut session=crate::SimulationSession::new(config.clone());
    assert!(session.handle_packet(ClientPacket::Login {account_id:id.account_id.clone(),password:"demo".into()})
        .iter().any(|p|matches!(p,ServerPacket::LoginSuccess{..})));
    let packets=session.handle_packet(ClientPacket::StartGame{character_index:id.character_index});
    assert!(packets.iter().any(|p|matches!(p,ServerPacket::UserInformation{..})),"{packets:?}");
    session
}
fn gain(session:&mut crate::SimulationSession,amount:i64)->Result<Vec<mir2_protocol::ServerPacket>,String>{
    let before=session.begin_guild_experience_command(false)?;
    assert!(before.is_some(),"non-Guild mentor source must be captured");
    let packets=super::leveling::apply_experience_gain(session.app.world_mut(),amount);
    session.finish_guild_experience_command(before,packets)
}
#[test]
fn mentor_normal_xp_source_bank_and_sub100_receipts_commit_without_guild(){
    let (config,pupil,_)=accounting_fixture();let mut s=login(&config,&pupil);
    gain(&mut s,99).unwrap();gain(&mut s,99).unwrap();gain(&mut s,300).unwrap();
    let bank=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger;
    assert_eq!(bank.bank_earned,3);assert_eq!(bank.local_event_sequence,3);
    assert_eq!(config.account_store.lock().unwrap().accounts["demo"].saves[&0].experience,498);
}
#[test]
fn mentor_xp_failure_rolls_back_receipt_xp_and_bank_then_retries(){
    let (config,pupil,_)=accounting_fixture();let mut s=login(&config,&pupil);
    let before=s.active_character_checkpoint().unwrap();
    config.inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(gain(&mut s,300).is_err());
    assert_eq!(s.active_character_checkpoint().unwrap().experience,before.experience);
    assert_eq!(config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.bank_earned,0);
    gain(&mut s,300).unwrap();
    assert_eq!(config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.bank_earned,3);
}
#[test]
fn mentor_transfer_does_not_invalidate_peer_revision_and_end_pays_once(){
    let (config,pupil,teacher)=accounting_fixture();let mut s=login(&config,&pupil);let mut t=login(&config,&teacher);
    gain(&mut s,300).unwrap();
    let epoch=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.relationship_epoch;
    let rev=t.active_character_checkpoint().unwrap().revision;
    config.settle_shared_mentor_bank(&pupil,epoch,None,20,true).unwrap();
    assert_eq!(config.account_store.lock().unwrap().accounts["teacher"].saves[&7].revision,rev);
    assert_eq!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.mentee_exp,3);
    config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Manual),22,true).unwrap();
    assert_eq!(config.account_store.lock().unwrap().accounts["teacher"].saves[&7].experience,0);
    t.consume_shared_mentor_credits().unwrap();
    assert_eq!(t.active_character_checkpoint().unwrap().experience,3);
    assert!(t.consume_shared_mentor_credits().unwrap().is_empty());
    assert!(config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Manual),23,true).is_err());
    assert_eq!(config.account_store.lock().unwrap().accounts["teacher"].saves[&7].experience,3);
    assert!(config.shared_mentor_profile_for(&pupil).unwrap().mentor.cooldown_until_ms>22);
    assert_eq!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.cooldown_until_ms,0);
}
#[test]
fn mentor_pair_persist_failure_and_credit_consume_failure_are_retryable(){
    let (config,pupil,teacher)=accounting_fixture();let mut s=login(&config,&pupil);let mut t=login(&config,&teacher);
    gain(&mut s,300).unwrap();
    let epoch=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.relationship_epoch;
    let before=serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
    config.inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Manual),22,true).is_err());
    assert_eq!(serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap(),before);
    config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Manual),23,true).unwrap();
    config.inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(t.consume_shared_mentor_credits().is_err());
    assert_eq!(t.active_character_checkpoint().unwrap().experience,0);
    assert_eq!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.ledger.leveling_applied,0);
    t.consume_shared_mentor_credits().unwrap();assert_eq!(t.active_character_checkpoint().unwrap().experience,3);
}
#[test]
fn mentor_offline_credit_is_in_login_bootstrap_and_does_not_force_levelup(){
    let (config,pupil,teacher)=accounting_fixture();let mut s=login(&config,&pupil);
    gain(&mut s,300).unwrap();
    let epoch=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.relationship_epoch;
    config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Manual),22,false).unwrap();
    let t=login(&config,&teacher);let save=t.active_character_checkpoint().unwrap();
    assert_eq!(save.experience,3);assert_eq!(save.character.level,20);
    assert_eq!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.ledger.balance_applied,3);
}
#[test]
fn mentor_expiry_is_strict_and_graduation_uses_latest_committed_level(){
    let (config,pupil,teacher)=accounting_fixture();
    let epoch=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.relationship_epoch;
    let deadline=10+crate::SHARED_MENTOR_DURATION_MS;
    assert!(config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::LoginExpired),deadline,true).is_err());
    assert!(config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Graduated),20,true).is_err());
    let mut s=login(&config,&pupil);let amount=s.active_character_checkpoint().unwrap().max_experience;
    gain(&mut s,amount).unwrap();assert_eq!(s.active_character_checkpoint().unwrap().character.level,11);
    config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::Graduated),21,true).unwrap();
    assert!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.partner_identity.is_none());
    assert_eq!(config.shared_mentor_profile_for(&pupil).unwrap().mentor.cooldown_until_ms,0);
    let (config,pupil,_)=accounting_fixture();
    let epoch=config.shared_mentor_profile_for(&pupil).unwrap().mentor.ledger.relationship_epoch;
    config.settle_shared_mentor_bank(&pupil,epoch,Some(crate::SharedMentorBreakReason::LoginExpired),deadline+1,false).unwrap();
}
#[test]
fn mentor_closed_source_and_unminted_credit_merge_are_guarded(){
    use super::shared_mentor_rewards::merge_mentor_state;
    let mut durable=student();let mut local=durable.clone();
    bank_mentor_gain(&mut local,99,None).unwrap();durable.partner_identity=None;
    assert!(merge_mentor_state(&local,&durable,true).is_err());
    let mut forged=durable.clone();forged.ledger.leveling_credit=99;
    assert_eq!(merge_mentor_state(&forged,&durable,true).unwrap().ledger.leveling_credit,0);
    forged.ledger.leveling_applied=99;assert!(merge_mentor_state(&forged,&durable,true).is_err());
    durable.ledger.leveling_credit=15;let mut local=durable.clone();local.ledger.leveling_credit=10;local.ledger.leveling_applied=10;
    let merged=merge_mentor_state(&local,&durable,true).unwrap();assert_eq!(merged.ledger.leveling_credit,15);assert_eq!(merged.ledger.leveling_applied,10);
}

#[test]
fn mentor_large_credit_is_consumed_in_bounded_owner_turns() {
    let (config, _, teacher) = accounting_fixture();
    let mut t = login(&config, &teacher);
    let credit = u64::from(u32::MAX) * 5;
    config.commit_account_store_transaction(&[teacher.account_id.clone()], |store| {
        let mut mentor = super::shared_relationships::profile(store, &teacher)?.mentor;
        mentor.ledger.leveling_credit = credit;
        super::shared_relationships::write_mentor(store, &teacher, mentor)
    }).unwrap();
    let packets = t.consume_shared_mentor_credits().unwrap();
    assert!(packets.iter().filter(|p| matches!(p,
        mir2_protocol::ServerPacket::GainExperience { .. })).count() <= 4);
    let applied = config.shared_mentor_profile_for(&teacher).unwrap().mentor.ledger.leveling_applied;
    assert!(applied > 0 && applied < credit, "remaining durable credit must survive a bounded turn");
    t.consume_shared_mentor_credits().unwrap();
    assert_eq!(config.shared_mentor_profile_for(&teacher).unwrap().mentor.ledger.leveling_applied, credit);
    assert!(t.consume_shared_mentor_credits().unwrap().is_empty());
}

#[test]
fn mentor_recipient_credit_does_not_authorize_revive_and_failure_keeps_dead() {
    let (config, _, teacher) = accounting_fixture();
    let mut t = login(&config, &teacher);
    let credit = t.active_character_checkpoint().unwrap().max_experience as u64;
    config.commit_account_store_transaction(&[teacher.account_id.clone()], |store| {
        let mut mentor = super::shared_relationships::profile(store, &teacher)?.mentor;
        mentor.ledger.leveling_credit = credit;
        super::shared_relationships::write_mentor(store, &teacher, mentor)
    }).unwrap();
    t.force_authoritative_player_life(true);
    config.inject_account_store_transaction_fault(crate::AccountStoreTransactionFault::BeforePersist);
    assert!(t.consume_shared_mentor_credits().is_err());
    assert!(super::components::current_player_is_dead(t.app.world()));
    let packets = t.consume_shared_mentor_credits().unwrap();
    assert!(packets.iter().any(|p| matches!(p, mir2_protocol::ServerPacket::LevelChanged { .. })));
    assert!(super::components::current_player_is_dead(t.app.world()));
    assert!(!packets.iter().any(|p| matches!(p, mir2_protocol::ServerPacket::ObjectRevived { .. })));
}
