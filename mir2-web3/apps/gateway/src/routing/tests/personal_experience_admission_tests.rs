use super::super::shared_experience_profiles::capture_locked_personal_experience_context;
use super::*;
use mir2_simulation::{Stage5FriendIdentity, ZoneExperiencePartner, ZoneExperienceProfile};
use std::cell::RefCell;
use std::sync::atomic::Ordering;

struct Fixture {
    runtime: SharedInProcessZoneSessionRuntime,
    keys: Vec<ZonePresenceKey>,
    sessions: Vec<SessionId>,
}

fn fixture(count: usize) -> Fixture {
    let shared = Arc::new(Mutex::new(SharedInProcessZoneState::new()));
    let mut runtime = shared_session_runtime(shared.clone());
    start_new_runtime(&mut runtime, "personal-owner", "PersonalOwner");
    let owner = runtime.current_presence_key().unwrap();
    let owner_session = runtime.current_zone_session_id().unwrap();
    runtime.refresh_current_experience_profile().unwrap();
    runtime.inner.save_active_character().unwrap();
    let template = runtime.inner.active_zone_join_snapshot("template").unwrap();
    let config = runtime.inner.shared_mentor_config().unwrap();
    let account = config.account_store.lock().unwrap().accounts[&owner.account_id].clone();
    let mut state = shared.lock().unwrap();
    let profile = state
        .zone_manager
        .player_experience_profile(&owner_session)
        .unwrap();
    let mut keys = vec![owner];
    let mut sessions = vec![owner_session];
    for index in 1..count {
        let key = ZonePresenceKey {
            account_id: format!("personal-peer-{index:03}"),
            character_index: template.character_index,
        };
        let session = SharedInProcessZoneState::zone_session_id_for_key(&key);
        let mut join = template.clone();
        join.session_id = session.clone();
        join.account_id = key.account_id.clone();
        join.object_id = 700_000 + index as u32;
        join.name = format!("PersonalPeer{index:03}");
        join.position.x += index as i32;
        state.zone_manager.join(join);
        let mut next = profile.clone();
        next.account_id = key.account_id.clone();
        assert!(state
            .zone_manager
            .update_experience_profile(&session, Some(next)));
        state.zone_sessions.insert(key.clone(), session.clone());
        state.zone_session_keys.insert(session.clone(), key.clone());
        keys.push(key);
        sessions.push(session);
    }
    drop(state);
    let mut store = config.account_store.lock().unwrap();
    for (index, key) in keys.iter().enumerate().skip(1) {
        let mut next = account.clone();
        for character in &mut next.characters {
            character.name = format!("PersonalPeer{index:03}");
        }
        for save in next.saves.values_mut() {
            save.character.name = format!("PersonalPeer{index:03}");
        }
        store.accounts.insert(key.account_id.clone(), next);
    }
    drop(store);
    Fixture {
        runtime,
        keys,
        sessions,
    }
}

fn partner(key: &ZonePresenceKey) -> ZoneExperiencePartner {
    ZoneExperiencePartner {
        identity: Stage5FriendIdentity {
            account_id: key.account_id.clone(),
            character_index: key.character_index,
        },
        relationship_epoch: 7,
    }
}

fn set_partners(fixture: &mut Fixture, lover: usize, mentor: usize) {
    let mut state = fixture.runtime.zone_state.lock().unwrap();
    let mut owner = state
        .zone_manager
        .player_experience_profile(&fixture.sessions[0])
        .unwrap();
    owner.lover_partner = Some(partner(&fixture.keys[lover]));
    owner.mentee_partner = Some(partner(&fixture.keys[mentor]));
    assert!(state
        .zone_manager
        .update_experience_profile(&fixture.sessions[0], Some(owner)));
}

#[test]
fn personal_experience_admission_rebases_only_current_partners_among_100_registered_players() {
    let mut fixture = fixture(100);
    set_partners(&mut fixture, 1, 2);
    let calls = RefCell::new(Vec::new());
    let rebase = |actor: &Stage5FriendIdentity,
                  _: &mut Option<ZoneExperienceProfile>,
                  _: &mut mir2_simulation::ZoneChatProfile| {
        calls.borrow_mut().push(actor.clone());
        Ok(())
    };
    let mut state = fixture.runtime.zone_state.lock().unwrap();
    assert_eq!(state.zone_sessions.len(), 100);
    let captured =
        capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase).unwrap();
    drop(state);
    assert!(captured.is_some());
    assert_eq!(
        calls.borrow().len(),
        3,
        "one owner and its two current partners"
    );
}

#[test]
fn personal_experience_admission_selects_new_store_partners_instead_of_stale_zone_partners() {
    let mut fixture = fixture(6);
    set_partners(&mut fixture, 1, 2);
    let calls = RefCell::new(Vec::new());
    let rebase = |actor: &Stage5FriendIdentity,
                  profile: &mut Option<ZoneExperienceProfile>,
                  chat: &mut mir2_simulation::ZoneChatProfile| {
        calls.borrow_mut().push(actor.clone());
        let profile = profile.as_mut().unwrap();
        if actor.account_id == fixture.keys[0].account_id {
            profile.lover_partner = Some(partner(&fixture.keys[3]));
            profile.mentee_partner = Some(partner(&fixture.keys[4]));
            profile.lover_eligible = true;
        } else if actor.account_id == fixture.keys[3].account_id {
            profile.lover_partner = Some(partner(&fixture.keys[0]));
        }
        chat.guild_name = Some("fresh personal projection".into());
        Ok(())
    };
    let (captured, old_chats) = {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        let zone = state
            .zone_manager
            .zone_key_for_session(&fixture.sessions[0])
            .unwrap();
        let captured =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        let old_chats = [1, 2, 5].map(|index| {
            state
                .zone_manager
                .zone(&zone)
                .unwrap()
                .player_chat_profile(&fixture.sessions[index])
                .unwrap()
                .guild_name
        });
        (captured, old_chats)
    };
    let profile = captured.unwrap().unwrap().0;
    assert_eq!(profile.lover_partner, Some(partner(&fixture.keys[3])));
    assert!(profile.lover_eligible);
    assert_eq!(profile.mentee_partner, Some(partner(&fixture.keys[4])));
    assert_eq!(
        calls.into_inner(),
        [0, 3, 4].map(|index| super::super::shared_mentor::identity(&fixture.keys[index]))
    );
    assert_eq!(old_chats, [None, None, None]);
}

#[test]
fn personal_experience_admission_validates_all_required_partners_before_publishing_any() {
    let mut fixture = fixture(4);
    set_partners(&mut fixture, 1, 2);
    let rebase = |actor: &Stage5FriendIdentity,
                  _: &mut Option<ZoneExperienceProfile>,
                  chat: &mut mir2_simulation::ZoneChatProfile| {
        if actor.account_id == fixture.keys[2].account_id {
            return Err("required partner authority unavailable".into());
        }
        chat.guild_name = Some("must not publish".into());
        Ok(())
    };
    let (result, chats) = {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        let result =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        let zone = state
            .zone_manager
            .zone_key_for_session(&fixture.sessions[0])
            .unwrap();
        let chats = fixture
            .sessions
            .iter()
            .map(|session| {
                state
                    .zone_manager
                    .zone(&zone)
                    .unwrap()
                    .player_chat_profile(session)
                    .unwrap()
                    .guild_name
            })
            .collect::<Vec<_>>();
        (result, chats)
    };
    assert!(result
        .unwrap_err()
        .contains("required partner authority unavailable"));
    assert_eq!(chats, vec![None; 4]);
}

#[test]
fn personal_experience_admission_invalid_required_peer_rate_source_cannot_partly_publish() {
    let mut fixture = fixture(3);
    set_partners(&mut fixture, 1, 2);
    let rebase = |actor: &Stage5FriendIdentity,
                  profile: &mut Option<ZoneExperienceProfile>,
                  chat: &mut mir2_simulation::ZoneChatProfile| {
        chat.guild_name = Some("must remain staged".into());
        if actor.account_id == fixture.keys[2].account_id {
            profile
                .as_mut()
                .unwrap()
                .rate_source
                .as_mut()
                .unwrap()
                .guild_stats
                .push(mir2_simulation::ZoneExperienceRateStat {
                    stat: 255,
                    value: 1,
                });
        }
        Ok(())
    };
    let (result, before, after) = {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        let zone = state
            .zone_manager
            .zone_key_for_session(&fixture.sessions[0])
            .unwrap();
        let snapshots = |state: &SharedInProcessZoneState| {
            fixture
                .sessions
                .iter()
                .map(|session| {
                    (
                        state.zone_manager.player_experience_profile(session),
                        state
                            .zone_manager
                            .zone(&zone)
                            .unwrap()
                            .player_chat_profile(session),
                    )
                })
                .collect::<Vec<_>>()
        };
        let before = snapshots(&state);
        let result =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        let after = snapshots(&state);
        (result, before, after)
    };
    assert!(result.is_err());
    assert_eq!(
        after, before,
        "no staged owner or partner can publish before full rate validation"
    );
}

#[test]
fn personal_experience_admission_fenced_owner_is_a_copy_and_fenced_peer_is_not_changed() {
    let mut fixture = fixture(3);
    set_partners(&mut fixture, 1, 2);
    let rebase = |_: &Stage5FriendIdentity,
                  profile: &mut Option<ZoneExperienceProfile>,
                  chat: &mut mir2_simulation::ZoneChatProfile| {
        profile.as_mut().unwrap().guild = Some(mir2_simulation::ZoneGuildExperienceMembership {
            guild_id: "fresh-copy".into(),
            membership_epoch: 8,
        });
        chat.guild_name = Some("current authority".into());
        Ok(())
    };
    let (result, chats) = {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        state.teardown_fences.insert(fixture.keys[0].clone());
        state.teardown_fences.insert(fixture.keys[1].clone());
        let result =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        let zone = state
            .zone_manager
            .zone_key_for_session(&fixture.sessions[0])
            .unwrap();
        let chats = fixture
            .sessions
            .iter()
            .map(|session| {
                state
                    .zone_manager
                    .zone(&zone)
                    .unwrap()
                    .player_chat_profile(session)
                    .unwrap()
                    .guild_name
            })
            .collect::<Vec<_>>();
        state.teardown_fences.clear();
        (result, chats)
    };
    let profile = result.unwrap().unwrap().0;
    assert_eq!(profile.guild.as_ref().unwrap().guild_id, "fresh-copy");
    assert!(!profile.lover_eligible);
    assert_eq!(chats, vec![None, None, Some("current authority".into())]);
}

#[test]
fn personal_experience_admission_deduplicates_partners_and_keeps_offline_gates() {
    let mut fixture = fixture(4);
    set_partners(&mut fixture, 1, 1);
    let calls = RefCell::new(Vec::new());
    let rebase = |actor: &Stage5FriendIdentity,
                  _: &mut Option<ZoneExperienceProfile>,
                  _: &mut mir2_simulation::ZoneChatProfile| {
        calls.borrow_mut().push(actor.clone());
        Ok(())
    };
    let (result, offline_result) = {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        let result =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        state.zone_sessions.remove(&fixture.keys[1]);
        state.zone_manager.handle(ZoneCommand::Leave {
            session_id: fixture.sessions[1].clone(),
        });
        let offline_result =
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], &rebase);
        (result, offline_result)
    };
    assert!(result.unwrap().is_some());
    let profile = offline_result.unwrap().unwrap().0;
    assert!(!profile.lover_eligible && !profile.mentee_eligible);
    assert_eq!(
        calls.borrow().len(),
        3,
        "two unique sources then only the owner"
    );
}

fn edit_systems(
    fixture: &Fixture,
    index: usize,
    edit: impl FnOnce(&mut mir2_simulation::Stage5SystemsState),
) {
    let config = fixture.runtime.inner.shared_mentor_config().unwrap();
    let mut store = config.account_store.lock().unwrap();
    let save = store
        .accounts
        .get_mut(&fixture.keys[index].account_id)
        .unwrap()
        .saves
        .get_mut(&fixture.keys[index].character_index)
        .unwrap();
    let mut systems = serde_json::from_str(save.stage5_systems_json.as_deref().unwrap()).unwrap();
    edit(&mut systems);
    save.stage5_systems_json = Some(serde_json::to_string(&systems).unwrap());
}

fn reciprocal_store_lovers(fixture: &Fixture, owner: usize, peer: usize) {
    for (actor, partner_index) in [(owner, peer), (peer, owner)] {
        edit_systems(fixture, actor, |systems| {
            systems.relationship.partner_identity =
                Some(partner(&fixture.keys[partner_index]).identity);
            systems.relationship.authority_revision = 7;
        });
    }
}

#[test]
fn personal_experience_admission_uses_actual_locked_store_partner_change_without_generation_hint() {
    let mut fixture = fixture(5);
    set_partners(&mut fixture, 1, 2);
    reciprocal_store_lovers(&fixture, 0, 3);
    let config = fixture.runtime.inner.shared_mentor_config().unwrap();
    // An unrelated corrupt checkpoint is not a required personal GainExp source.
    config
        .account_store
        .lock()
        .unwrap()
        .accounts
        .get_mut(&fixture.keys[4].account_id)
        .unwrap()
        .saves
        .get_mut(&fixture.keys[4].character_index)
        .unwrap()
        .stage5_systems_json = Some("{".into());
    let generation = fixture
        .runtime
        .shared_social_generation
        .load(Ordering::Acquire);
    let result = config.with_shared_zone_experience_admission(|rebase| {
        let mut state = fixture.runtime.zone_state.lock().unwrap();
        capture_locked_personal_experience_context(&mut state, &fixture.keys[0], rebase)
    });
    let profile = result.unwrap().unwrap().0;
    assert_eq!(profile.lover_partner, Some(partner(&fixture.keys[3])));
    assert!(profile.mentee_partner.is_none());
    assert_eq!(
        fixture
            .runtime
            .shared_social_generation
            .load(Ordering::Acquire),
        generation
    );
}

#[test]
fn personal_experience_admission_missing_or_malformed_required_authority_is_fail_closed() {
    for malformed in [false, true] {
        let fixture = fixture(4);
        reciprocal_store_lovers(&fixture, 0, 1);
        // The owner rebase can validate its reciprocal lover, but rebasing that
        // required peer must also validate the peer's current mentor authority.
        edit_systems(&fixture, 1, |systems| {
            systems.mentor.is_mentor = false;
            systems.mentor.partner_identity = Some(partner(&fixture.keys[2]).identity);
            systems.mentor.ledger.relationship_epoch = 7;
        });
        let config = fixture.runtime.inner.shared_mentor_config().unwrap();
        {
            let mut store = config.account_store.lock().unwrap();
            if malformed {
                store
                    .accounts
                    .get_mut(&fixture.keys[2].account_id)
                    .unwrap()
                    .saves
                    .get_mut(&fixture.keys[2].character_index)
                    .unwrap()
                    .stage5_systems_json = Some("{".into());
            } else {
                store.accounts.remove(&fixture.keys[2].account_id);
            }
        }
        let before = {
            let state = fixture.runtime.zone_state.lock().unwrap();
            fixture
                .sessions
                .iter()
                .map(|session| state.zone_manager.player_experience_profile(session))
                .collect::<Vec<_>>()
        };
        let result = config.with_shared_zone_experience_admission(|rebase| {
            let mut state = fixture.runtime.zone_state.lock().unwrap();
            capture_locked_personal_experience_context(&mut state, &fixture.keys[0], rebase)
        });
        let after = {
            let state = fixture.runtime.zone_state.lock().unwrap();
            fixture
                .sessions
                .iter()
                .map(|session| state.zone_manager.player_experience_profile(session))
                .collect::<Vec<_>>()
        };
        assert!(result.is_err(), "malformed={malformed}");
        assert_eq!(after, before, "malformed={malformed}");
    }
}
