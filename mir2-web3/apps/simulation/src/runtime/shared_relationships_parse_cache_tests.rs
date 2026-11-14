//! Memory-only authority reader parity; no game, private files, or environment mutation.
use super::*;
use std::sync::Mutex;
use std::time::Instant;

fn fixture(label: &str) -> (AccountStore, Stage5FriendIdentity) {
    let config = SimulationConfig::default();
    let mut store = config.account_store.lock().unwrap().clone();
    let account = store.accounts.remove("demo").unwrap();
    let actor = Stage5FriendIdentity {
        account_id: format!("mentor-parse-{label}"),
        character_index: config.default_character.index,
    };
    store.accounts.insert(actor.account_id.clone(), account);
    (store, actor)
}

fn set_json(store: &mut AccountStore, actor: &Stage5FriendIdentity, json: Option<String>) {
    store
        .accounts
        .get_mut(&actor.account_id)
        .unwrap()
        .saves
        .get_mut(&actor.character_index)
        .unwrap()
        .stage5_systems_json = json;
}

// Exact pre-change profile algorithm, isolated to tests for a semantic oracle.
fn uncached_profile(
    store: &AccountStore,
    identity: &Stage5FriendIdentity,
) -> Result<SharedMentorProfile, String> {
    let account = store
        .accounts
        .get(&identity.account_id)
        .ok_or("social account no longer exists")?;
    let character = account
        .characters
        .iter()
        .find(|c| c.index == identity.character_index)
        .ok_or("social character no longer exists")?;
    let save = account
        .saves
        .get(&identity.character_index)
        .ok_or("social character save missing")?;
    let systems: Stage5SystemsState = match save.stage5_systems_json.as_deref() {
        Some(value) => {
            serde_json::from_str(value).map_err(|e| format!("invalid social state: {e}"))?
        }
        None => Stage5SystemsState::default(),
    };
    Ok(SharedMentorProfile {
        identity: identity.clone(),
        name: character.name.clone(),
        class: character.class,
        level: character.level,
        mentor: systems.mentor,
    })
}

#[test]
fn mentor_profile_parse_memo_keeps_metadata_fresh() {
    use super::super::shared_experience_profile::full_json_decodes_for_test;

    let (mut store, actor) = fixture("metadata");
    set_json(&mut store, &actor, Some("{}".into()));
    let decodes_before = full_json_decodes_for_test();
    let before = profile(&store, &actor).unwrap();
    let account = store.accounts.get_mut(&actor.account_id).unwrap();
    let original_json = account.saves[&actor.character_index]
        .stage5_systems_json
        .clone();
    let character = account
        .characters
        .iter_mut()
        .find(|c| c.index == actor.character_index)
        .unwrap();
    character.name = "public changed name".into();
    character.class = if character.class == MirClass::Warrior {
        MirClass::Wizard
    } else {
        MirClass::Warrior
    };
    character.level += 1;
    let after = profile(&store, &actor).unwrap();
    assert_eq!(after, uncached_profile(&store, &actor).unwrap());
    assert_ne!(after.name, before.name);
    assert_ne!(after.class, before.class);
    assert_ne!(after.level, before.level);
    assert_eq!(after.mentor, before.mentor);
    assert_eq!(full_json_decodes_for_test() - decodes_before, 1);
    assert_eq!(
        store.accounts[&actor.account_id].saves[&actor.character_index].stage5_systems_json,
        original_json
    );
}

#[test]
fn mentor_profile_parse_memo_uses_current_bytes_even_at_same_revision() {
    let (mut store, actor) = fixture("same-revision");
    let mut systems = Stage5SystemsState::default();
    set_json(
        &mut store,
        &actor,
        Some(serde_json::to_string(&systems).unwrap()),
    );
    let revision = store.accounts[&actor.account_id].saves[&actor.character_index].revision;
    let before = profile(&store, &actor).unwrap();
    systems.mentor.allow_mentor = !systems.mentor.allow_mentor;
    systems.mentor.authority_revision = 17;
    set_json(
        &mut store,
        &actor,
        Some(serde_json::to_string(&systems).unwrap()),
    );
    let after = profile(&store, &actor).unwrap();
    assert_eq!(
        store.accounts[&actor.account_id].saves[&actor.character_index].revision,
        revision
    );
    assert_ne!(after.mentor, before.mentor);
    assert_eq!(after, uncached_profile(&store, &actor).unwrap());
}

#[test]
fn mentor_profile_parse_memo_preserves_none_invalid_and_missing_error_order() {
    let (mut store, actor) = fixture("errors");
    for json in [
        None,
        Some("{}"),
        Some(" {}\n"),
        Some(r#"{"mail":false}"#),
        Some("{"),
    ] {
        set_json(&mut store, &actor, json.map(str::to_owned));
        assert_eq!(profile(&store, &actor), uncached_profile(&store, &actor));
    }
    let account = store.accounts[&actor.account_id].clone();
    store.accounts.remove(&actor.account_id);
    assert_eq!(
        profile(&store, &actor).unwrap_err(),
        "social account no longer exists"
    );
    store
        .accounts
        .insert(actor.account_id.clone(), account.clone());
    store
        .accounts
        .get_mut(&actor.account_id)
        .unwrap()
        .characters
        .clear();
    assert_eq!(
        profile(&store, &actor).unwrap_err(),
        "social character no longer exists"
    );
    store.accounts.insert(actor.account_id.clone(), account);
    store
        .accounts
        .get_mut(&actor.account_id)
        .unwrap()
        .saves
        .remove(&actor.character_index);
    assert_eq!(
        profile(&store, &actor).unwrap_err(),
        "social character save missing"
    );
}

#[test]
fn mentor_profile_parse_memo_is_pure_across_stores_and_owned_results() {
    let (mut first, actor) = fixture("two-stores");
    set_json(&mut first, &actor, Some("{}".into()));
    let mut second = first.clone();
    let mut state = Stage5SystemsState::default();
    state.mentor.authority_revision = 29;
    set_json(
        &mut second,
        &actor,
        Some(serde_json::to_string(&state).unwrap()),
    );
    for _ in 0..3 {
        assert_eq!(profile(&first, &actor), uncached_profile(&first, &actor));
        assert_eq!(profile(&second, &actor), uncached_profile(&second, &actor));
    }
    let mut detached = profile(&second, &actor).unwrap();
    detached.mentor.authority_revision += 1;
    assert_ne!(detached.mentor, profile(&second, &actor).unwrap().mentor);
}

#[test]
fn mentor_profile_parse_memo_contention_falls_back_to_original_parse_and_errors() {
    use super::super::shared_experience_profile::{
        full_json_decodes_for_test, with_parse_cache_locked_for_test,
    };
    let (mut store, actor) = fixture("contention");
    set_json(&mut store, &actor, Some("{}".into()));
    let before = full_json_decodes_for_test();
    let result = with_parse_cache_locked_for_test(|| profile(&store, &actor));
    assert_eq!(result, uncached_profile(&store, &actor));
    assert_eq!(full_json_decodes_for_test() - before, 1);
    set_json(&mut store, &actor, Some(r#"{"mail":false}"#.into()));
    let result = with_parse_cache_locked_for_test(|| profile(&store, &actor));
    assert_eq!(result, uncached_profile(&store, &actor));
}

#[test]
fn mentor_profile_parse_memo_observes_write_mentor_and_store_poison() {
    let (mut store, actor) = fixture("mutation");
    set_json(&mut store, &actor, Some("{}".into()));
    let old = profile(&store, &actor).unwrap();
    let mut mentor = old.mentor.clone();
    mentor.authority_revision = 42;
    write_mentor(&mut store, &actor, mentor.clone()).unwrap();
    assert_eq!(profile(&store, &actor).unwrap().mentor, mentor);
    assert_eq!(profile(&store, &actor), uncached_profile(&store, &actor));
    let mut config = SimulationConfig::default();
    config.account_store = std::sync::Arc::new(Mutex::new(store));
    assert_eq!(
        config.shared_mentor_profile_for(&actor).unwrap().mentor,
        mentor
    );
    let shared = config.account_store.clone();
    assert!(std::thread::spawn(move || {
        let _guard = shared.lock().unwrap();
        panic!("test-only Store poison");
    })
    .join()
    .is_err());
    assert_eq!(
        config.shared_mentor_profile_for(&actor).unwrap_err(),
        "social account store unavailable"
    );
}

#[test]
#[ignore = "bounded serial component profile; not a CCU or timing-threshold test"]
fn mentor_profile_parse_memo_component_profile() {
    use super::super::shared_experience_profile::full_json_decodes_for_test;
    for (shape, level) in [("default", None), ("level40", Some(40))] {
        let (mut store, original) = fixture(&format!("profile-{shape}"));
        let template = store.accounts.remove(&original.account_id).unwrap();
        let encoded = level.map(|_| serde_json::to_string(&Stage5SystemsState::default()).unwrap());
        let mut actors = Vec::new();
        for i in 0..100 {
            let actor = Stage5FriendIdentity {
                account_id: format!("public-profile-{shape}-{i}"),
                character_index: original.character_index,
            };
            let mut account = template.clone();
            let character = account
                .characters
                .iter_mut()
                .find(|c| c.index == actor.character_index)
                .unwrap();
            character.name = format!("public-profile-{i}");
            if let Some(level) = level {
                character.level = level;
            }
            let saved = account.saves.get_mut(&actor.character_index).unwrap();
            saved.character = character.clone();
            saved.stage5_systems_json = encoded.clone();
            store.accounts.insert(actor.account_id.clone(), account);
            actors.push(actor);
        }
        let expected = actors
            .iter()
            .map(|actor| uncached_profile(&store, actor).unwrap())
            .collect::<Vec<_>>();
        let store = Mutex::new(store);
        for trial in 0..3 {
            for cached in [false, true] {
                let before = full_json_decodes_for_test();
                let mut measured_ns = Vec::with_capacity(3200);
                for _ in 0..32 {
                    for (actor, expected) in actors.iter().zip(&expected) {
                        let guard = store.lock().unwrap();
                        let began = Instant::now();
                        let actual = if cached {
                            profile(&guard, actor)
                        } else {
                            uncached_profile(&guard, actor)
                        }
                        .unwrap();
                        measured_ns.push(began.elapsed().as_nanos());
                        drop(guard);
                        assert_eq!(&actual, expected);
                    }
                }
                let json_decodes = if cached {
                    full_json_decodes_for_test() - before
                } else if encoded.is_some() {
                    measured_ns.len() as u64
                } else {
                    0
                };
                measured_ns.sort_unstable();
                let total_ns = measured_ns.iter().sum::<u128>();
                println!(
                    "{}",
                    serde_json::json!({
                        "schema":"mir2.mentor-profile-parse-component.v1", "shape":shape, "trial":trial,
                        "reader":if cached { "memo" } else { "pre-change-uncached" },
                        "identities":100, "readsPerIdentity":32, "calls":measured_ns.len(),
                        "fullJsonDecodes":json_decodes, "jsonBytesPerIdentity":encoded.as_ref().map_or(0,String::len),
                        "lockHeldReaderTotalNs":total_ns, "lockHeldReaderP95Ns":measured_ns[(measured_ns.len()*95).div_ceil(100)-1],
                        "lockHeldReaderMaxNs":measured_ns.last().unwrap(), "outputEqualityCheckedEveryCall":true,
                        "cacheScope":"process static; reused across trials; reads only",
                        "excludes":"fixture construction, mutex acquisition and output comparison",
                        "capacityClaimAllowed":false
                    })
                );
            }
        }
    }
}
