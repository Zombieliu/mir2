use mir2_protocol::{ClientPacket, MirClass, MirGender};
use mir2_simulation::{
    validate_shared_mentor_request, AccountRecord, CharacterRecord, SharedMentorMutation,
    SimulationConfig, SimulationSession, Stage5FriendIdentity,
};

#[test]
fn social_permissions_default_to_crystal_blocked_and_preserve_explicit_old_true() {
    let defaults = mir2_simulation::Stage5SystemsState::default();
    assert!(
        !defaults.mentor.allow_mentor
            && !defaults.relationship.allow_marriage
            && !defaults.relationship.allow_lover_recall
    );
    let missing: mir2_simulation::Stage5SystemsState =
        serde_json::from_value(serde_json::json!({"mentor":{},"relationship":{}})).unwrap();
    assert!(!missing.mentor.allow_mentor && !missing.relationship.allow_marriage);
    let explicit: mir2_simulation::Stage5SystemsState = serde_json::from_value(
        serde_json::json!({"mentor":{"allowMentor":true},"relationship":{"allowMarriage":true}}),
    )
    .unwrap();
    assert!(explicit.mentor.allow_mentor && explicit.relationship.allow_marriage);
}

fn id(account: &str, index: i32) -> Stage5FriendIdentity {
    Stage5FriendIdentity {
        account_id: account.into(),
        character_index: index,
    }
}
fn fixture() -> SimulationConfig {
    let config = SimulationConfig::default();
    {
        let mut store = config.account_store.lock().unwrap();
        for (index, account, level, class) in [
            (1, "teacher", 30, MirClass::Warrior),
            (2, "other", 30, MirClass::Wizard),
            (3, "young", 10, MirClass::Warrior),
        ] {
            store.accounts.insert(
                account.into(),
                AccountRecord::new(CharacterRecord {
                    index,
                    name: account.into(),
                    level,
                    class,
                    gender: MirGender::Male,
                }),
            );
        }
    }
    config
        .commit_shared_mentor_mutation(&id("teacher", 1), SharedMentorMutation::TogglePermission, 0)
        .unwrap();
    config
}

#[test]
fn mentor_authority_checks_identity_class_level_permission_and_reciprocal_cancel() {
    let config = fixture();
    let pupil = id("demo", 0);
    let teacher = id("teacher", 1);
    let pp = config.shared_mentor_profile_for(&pupil).unwrap();
    assert!(validate_shared_mentor_request(&pp, &pp, 100).is_err());
    assert!(validate_shared_mentor_request(
        &pp,
        &config.shared_mentor_profile_for(&id("other", 2)).unwrap(),
        100
    )
    .is_err());
    assert!(validate_shared_mentor_request(
        &pp,
        &config.shared_mentor_profile_for(&id("young", 3)).unwrap(),
        100
    )
    .is_err());
    config
        .commit_shared_mentor_mutation(&teacher, SharedMentorMutation::TogglePermission, 100)
        .unwrap();
    assert!(config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept {
                student: pupil.clone()
            },
            100
        )
        .is_err());
    config
        .commit_shared_mentor_mutation(&teacher, SharedMentorMutation::TogglePermission, 100)
        .unwrap();
    config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept {
                student: pupil.clone(),
            },
            100,
        )
        .unwrap();
    assert_eq!(
        config
            .shared_mentor_profile_for(&pupil)
            .unwrap()
            .mentor
            .partner_identity,
        Some(teacher.clone())
    );
    assert!(config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept {
                student: id("young", 3)
            },
            100
        )
        .is_err());
    config
        .commit_shared_mentor_mutation(&pupil, SharedMentorMutation::Cancel, 200)
        .unwrap();
    assert!(config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept {
                student: pupil.clone()
            },
            300
        )
        .is_err());
    config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept { student: pupil },
            7 * 24 * 60 * 60 * 1000 + 201,
        )
        .unwrap();
}

#[test]
fn old_online_snapshot_save_cannot_erase_new_pair_or_resurrect_cancelled_pair() {
    let config = fixture();
    let pupil = id("demo", 0);
    let teacher = id("teacher", 1);
    let mut session = SimulationSession::new(config.clone());
    session.handle_packet(ClientPacket::Login {
        account_id: "demo".into(),
        password: "demo".into(),
    });
    session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    config
        .commit_shared_mentor_mutation(
            &teacher,
            SharedMentorMutation::Accept {
                student: pupil.clone(),
            },
            100,
        )
        .unwrap();
    session.save_active_character().unwrap();
    assert_eq!(
        config
            .shared_mentor_profile_for(&pupil)
            .unwrap()
            .mentor
            .partner_identity,
        Some(teacher.clone())
    );
    session.refresh_shared_mentor(&Default::default(), true);
    config
        .commit_shared_mentor_mutation(&teacher, SharedMentorMutation::Cancel, 200)
        .unwrap();
    session.save_active_character().unwrap();
    assert!(config
        .shared_mentor_profile_for(&pupil)
        .unwrap()
        .mentor
        .partner_identity
        .is_none());
    assert!(config
        .shared_mentor_profile_for(&teacher)
        .unwrap()
        .mentor
        .partner_identity
        .is_none());
}

#[test]
fn concurrent_full_save_and_bilateral_commit_preserve_both_sides() {
    for _ in 0..4 {
        let config = fixture();
        let mut session = SimulationSession::new(config.clone());
        session.handle_packet(ClientPacket::Login {
            account_id: "demo".into(),
            password: "demo".into(),
        });
        session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let worker_barrier = barrier.clone();
        let worker_config = config.clone();
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            worker_config
                .commit_shared_mentor_mutation(
                    &id("teacher", 1),
                    SharedMentorMutation::Accept {
                        student: id("demo", 0),
                    },
                    100,
                )
                .unwrap();
        });
        barrier.wait();
        session.save_active_character().unwrap();
        worker.join().unwrap();
        assert_eq!(
            config
                .shared_mentor_profile_for(&id("demo", 0))
                .unwrap()
                .mentor
                .partner_identity,
            Some(id("teacher", 1))
        );
        assert_eq!(
            config
                .shared_mentor_profile_for(&id("teacher", 1))
                .unwrap()
                .mentor
                .partner_identity,
            Some(id("demo", 0))
        );
    }
}

#[test]
fn client_social_view_keeps_ui_state_without_account_identities_or_revisions() {
    let session =
        mir2_simulation::SimulationSession::new(mir2_simulation::SimulationConfig::default());
    let mut snapshot = session.world_snapshot();
    let identity = mir2_simulation::Stage5FriendIdentity {
        account_id: "private-account".into(),
        character_index: 42,
    };
    snapshot.stage5_systems.social.friends = vec!["PublicName".into()];
    snapshot
        .stage5_systems
        .social
        .friend_identities
        .insert("PublicName".into(), identity.clone());
    snapshot.stage5_systems.mentor.partner_identity = Some(identity.clone());
    snapshot.stage5_systems.mentor.name = "Teacher".into();
    snapshot.stage5_systems.mentor.level = 30;
    snapshot.stage5_systems.mentor.online = true;
    snapshot.stage5_systems.mentor.allow_mentor = true;
    snapshot.stage5_systems.mentor.authority_revision = 7;
    snapshot.stage5_systems.relationship.partner_identity = Some(identity);
    snapshot.stage5_systems.relationship.partner_name = "Spouse".into();
    snapshot.stage5_systems.relationship.allow_lover_recall = true;
    snapshot.stage5_systems.relationship.authority_revision = 8;
    let private = serde_json::to_value(&snapshot).unwrap();
    let public = serde_json::to_value(snapshot.client_view()).unwrap();
    assert_eq!(
        private["stage5Systems"]["mentor"]["partnerIdentity"]["accountId"],
        "private-account"
    );
    assert_eq!(
        private["stage5Systems"]["social"]["friendIdentities"]["PublicName"]["characterIndex"],
        42
    );
    assert!(!public.to_string().contains("private-account"));
    let systems = &public["stage5Systems"];
    assert!(systems["social"].get("friendIdentities").is_none());
    for domain in ["mentor", "relationship"] {
        assert!(systems[domain].get("partnerIdentity").is_none());
        assert!(systems[domain].get("authorityRevision").is_none());
    }
    assert_eq!(systems["social"]["friends"][0], "PublicName");
    assert_eq!(systems["mentor"]["name"], "Teacher");
    assert_eq!(systems["mentor"]["level"], 30);
    assert_eq!(systems["mentor"]["online"], true);
    assert_eq!(systems["mentor"]["allowMentor"], true);
    assert_eq!(systems["relationship"]["partnerName"], "Spouse");
    assert_eq!(systems["relationship"]["allowLoverRecall"], true);
}
