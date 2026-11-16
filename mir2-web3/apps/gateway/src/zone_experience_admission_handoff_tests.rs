use super::*;
use mir2_simulation::{AccountRecord, CharacterSaveRecord, Stage5FriendIdentity, ZoneChatProfile};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn fixture() -> (
    GatewayConfig,
    Arc<Mutex<SharedInProcessZoneState>>,
    Stage5FriendIdentity,
) {
    let config = GatewayConfig::default();
    let mut character = config.default_character.clone();
    character.index += 1;
    character.name = "AdmissionOwned".into();
    let identity = Stage5FriendIdentity {
        account_id: "admission-handoff-owned".into(),
        character_index: character.index,
    };
    let mut account = AccountRecord::new(character.clone());
    account
        .saves
        .insert(character.index, CharacterSaveRecord::new(character));
    {
        let mut store = config.account_store.lock().unwrap();
        store.next_character_index = store.next_character_index.max(identity.character_index + 1);
        store.accounts.insert(identity.account_id.clone(), account);
    }
    (
        config,
        Arc::new(Mutex::new(SharedInProcessZoneState::new())),
        identity,
    )
}

/// The Zone is definitely held before the test starts admission. No sleeps or
/// scheduling-speed assertion: the hook explicitly releases this owned holder.
fn held_zone(
    zone: &Arc<Mutex<SharedInProcessZoneState>>,
) -> (mpsc::Sender<bool>, thread::JoinHandle<()>) {
    let zone = Arc::clone(zone);
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let holder = thread::spawn(move || {
        let _guard = zone.lock().unwrap();
        ready_tx.send(()).unwrap();
        if release_rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            panic!("owned Zone poison premise");
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    (release_tx, holder)
}

#[test]
fn busy_handoff_releases_store_and_rechecks_fresh_checkpoint() {
    let (config, zone, actor) = fixture();
    config
        .with_shared_zone_experience_admission(|rebase| {
            rebase(&actor, &mut None, &mut ZoneChatProfile::default())
        })
        .unwrap();
    let (release, holder) = held_zone(&zone);
    let changed_config = config.clone();
    let changed_actor = actor.clone();
    let calls = AtomicUsize::new(0);
    let result = with_zone_experience_admission_impl(
        &config,
        &zone,
        |locked, rebase| {
            let _guard = locked.map_err(|_| "original Zone error")?;
            calls.fetch_add(1, Ordering::SeqCst);
            rebase(&actor, &mut None, &mut ZoneChatProfile::default())
        },
        || {
            // A different thread must acquire Store while Zone is still held.
            thread::spawn(move || {
                let mut store = changed_config.account_store.try_lock().unwrap();
                assert!(store
                    .accounts
                    .get_mut(&changed_actor.account_id)
                    .unwrap()
                    .saves
                    .remove(&changed_actor.character_index)
                    .is_some());
            })
            .join()
            .unwrap();
            release.send(false).unwrap();
        },
    );
    holder.join().unwrap();
    assert_eq!(
        result.unwrap_err(),
        "experience profile character checkpoint missing"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(config.account_store.try_lock().is_ok());
    assert!(zone.try_lock().is_ok());
}

#[test]
fn busy_handoff_consumes_nonclone_body_once_with_both_locks_held() {
    struct NonCloneToken;
    let token = NonCloneToken;
    let (config, zone, _) = fixture();
    let (release, holder) = held_zone(&zone);
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let store = Arc::clone(&config.account_store);
    let observed_zone = Arc::clone(&zone);
    let result = with_zone_experience_admission_impl(
        &config,
        &zone,
        move |locked, _| {
            let _guard = locked.map_err(|_| "original Zone error")?;
            drop(token); // This closure is FnOnce, not reusable FnMut.
            assert!(matches!(
                store.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            assert!(matches!(
                observed_zone.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            count.fetch_add(1, Ordering::SeqCst);
            Ok(7)
        },
        || release.send(false).unwrap(),
    );
    holder.join().unwrap();
    assert_eq!(result.unwrap(), 7);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(config.account_store.try_lock().is_ok());
    assert!(zone.try_lock().is_ok());
}

#[test]
fn busy_handoff_fresh_store_poison_precedes_zone_poison_without_running_body() {
    let (config, zone, _) = fixture();
    let (release, holder) = held_zone(&zone);
    let changed = config.clone();
    let calls = AtomicUsize::new(0);
    let result: Result<(), String> = with_zone_experience_admission_impl(
        &config,
        &zone,
        |locked, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            let _guard = locked.expect("original Zone panic policy");
            Ok(())
        },
        || {
            assert!(thread::spawn(move || {
                let _guard = changed.account_store.try_lock().unwrap();
                panic!("owned Store poison premise");
            })
            .join()
            .is_err());
            release.send(true).unwrap();
            assert!(holder.join().is_err());
        },
    );
    assert_eq!(
        result.unwrap_err(),
        "experience profile authority mutex poisoned"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(config.account_store.is_poisoned());
    assert!(zone.is_poisoned());
}

#[test]
fn body_error_does_not_retry_and_releases_both_locks() {
    let (config, zone, _) = fixture();
    let calls = AtomicUsize::new(0);
    let result: Result<(), String> = with_zone_experience_admission_impl(
        &config,
        &zone,
        |locked, _| {
            let _guard = locked.map_err(|_| "original Zone error")?;
            calls.fetch_add(1, Ordering::SeqCst);
            Err("owned business failure".into())
        },
        || panic!("a business error is never Busy"),
    );
    assert_eq!(result.unwrap_err(), "owned business failure");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(config.account_store.try_lock().is_ok());
    assert!(zone.try_lock().is_ok());
}

#[test]
fn poisoned_zone_preserves_result_error_and_expect_store_poison_policies() {
    let (config, zone, _) = fixture();
    let (release, holder) = held_zone(&zone);
    release.send(true).unwrap();
    assert!(holder.join().is_err());
    let result: Result<(), String> = with_zone_experience_admission(&config, &zone, |locked, _| {
        let _guard = locked.map_err(|_| "original Zone error")?;
        Ok(())
    });
    assert_eq!(result.unwrap_err(), "original Zone error");
    assert!(!config.account_store.is_poisoned());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), String> = with_zone_experience_admission(&config, &zone, |locked, _| {
            let _guard = locked.expect("original Zone panic policy");
            Ok(())
        });
    }))
    .is_err());
    assert!(config.account_store.is_poisoned());
}
