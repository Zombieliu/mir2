use super::*;
use mir2_protocol::{MirClass, MirGender};

const COMMIT: u64 = 1_800_028_800_100;
fn checkpoint(level: u16) -> CharacterSaveRecord {
    CharacterSaveRecord::new(crate::CharacterRecord {
        index: 1,
        name: "source-fixture".into(),
        level,
        class: MirClass::Warrior,
        gender: MirGender::Male,
    })
}
fn insert(save: &mut CharacterSaveRecord, event: CapturedGameplayFact) {
    let key = event.journal_key().unwrap();
    save.guild_experience_journal
        .promotion_game_facts
        .entry(key)
        .or_insert(event);
}
fn facts(
    before: Option<&CharacterSaveRecord>,
    after: &CharacterSaveRecord,
) -> Vec<TrustedGameFact> {
    derive_character_game_facts("account", 1, before, after, 7, COMMIT).unwrap()
}

#[test]
fn historical_snapshots_never_invent_event_times() {
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    after
        .quest_states_json
        .push(json!({"quest_id":41,"stage":"completed"}).to_string());
    after
        .guild_experience_journal
        .applied_kill_receipts
        .insert("old-kill".into(), "a".repeat(64));
    assert!(facts(None, &after).is_empty());
    assert!(facts(Some(&before), &after).is_empty());
}

#[test]
fn movement_online_time_and_inventory_changes_are_not_activity() {
    let before = checkpoint(30);
    let mut after = before.clone();
    after.position.x += 1;
    after.gold += 10;
    after.revision += 1;
    assert!(facts(Some(&before), &after).is_empty());
}

#[test]
fn actual_shared_kill_time_survives_delayed_save_across_utc_midnight() {
    let before = checkpoint(30);
    let mut after = before.clone();
    let occurred = COMMIT / 86_400_000 * 86_400_000 - 100;
    let hash = "a".repeat(64);
    after
        .guild_experience_journal
        .applied_kill_receipts
        .insert("kill".into(), hash.clone());
    insert(
        &mut after,
        CapturedGameplayFact::combat_kill(occurred, "kill", &hash),
    );
    let events = facts(Some(&before), &after);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "combatKill");
    assert_eq!(events[0].occurred_at_ms, occurred);
    assert_ne!(events[0].occurred_at_ms / 86_400_000, COMMIT / 86_400_000);
    assert!(facts(Some(&after), &after).is_empty());
}

#[test]
fn kill_requires_the_exact_committed_source_receipt() {
    let before = checkpoint(30);
    let mut after = before.clone();
    insert(
        &mut after,
        CapturedGameplayFact::combat_kill(COMMIT - 1, "kill", &"a".repeat(64)),
    );
    assert!(derive_character_game_facts("account", 1, Some(&before), &after, 7, COMMIT).is_err());
    after
        .guild_experience_journal
        .applied_kill_receipts
        .insert("kill".into(), "b".repeat(64));
    assert!(derive_character_game_facts("account", 1, Some(&before), &after, 7, COMMIT).is_err());
}

#[test]
fn ordinary_and_periodic_completed_quests_use_explicit_authority_time() {
    let before = checkpoint(30);
    let mut after = before.clone();
    let occurred = COMMIT - 3;
    insert(
        &mut after,
        CapturedGameplayFact::quest_completed(occurred, 41),
    );
    let events = facts(Some(&before), &after);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].source, "quest_commit");
    assert_eq!(events[0].occurred_at_ms, occurred);
    assert!(facts(Some(&after), &after).is_empty());
}

#[test]
fn only_first_meaningful_event_of_each_source_utc_day_is_retained() {
    let mut after = checkpoint(30);
    insert(
        &mut after,
        CapturedGameplayFact::quest_completed(COMMIT - 10, 41),
    );
    insert(
        &mut after,
        CapturedGameplayFact::quest_completed(COMMIT - 1, 42),
    );
    insert(
        &mut after,
        CapturedGameplayFact::quest_completed(COMMIT - 86_400_000, 43),
    );
    assert_eq!(after.guild_experience_journal.promotion_game_facts.len(), 2);
    assert!(after
        .guild_experience_journal
        .promotion_game_facts
        .values()
        .any(|value| value.quest_id == Some(41)));
    assert!(!after
        .guild_experience_journal
        .promotion_game_facts
        .values()
        .any(|value| value.quest_id == Some(42)));
}

#[test]
fn level_capture_uses_event_time_and_does_not_count_as_activity_day() {
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    insert(
        &mut after,
        CapturedGameplayFact::level_reached(COMMIT - 1, 30),
    );
    let events = facts(Some(&before), &after);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].source, "level_commit");
    assert_eq!(events[0].level, Some(30));
    assert_eq!(events[0].occurred_at_ms, COMMIT - 1);
}

#[test]
fn first_attainment_cannot_be_rewritten_or_dated_after_commit() {
    let mut before = checkpoint(30);
    insert(
        &mut before,
        CapturedGameplayFact::level_reached(COMMIT - 2, 30),
    );
    let mut after = before.clone();
    after
        .guild_experience_journal
        .promotion_game_facts
        .get_mut("level:30")
        .unwrap()
        .occurred_at_ms = COMMIT - 1;
    assert!(derive_character_game_facts("account", 1, Some(&before), &after, 7, COMMIT).is_err());
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    insert(
        &mut after,
        CapturedGameplayFact::level_reached(COMMIT + 1, 30),
    );
    assert!(derive_character_game_facts("account", 1, Some(&before), &after, 7, COMMIT).is_err());
}

#[test]
fn deterministic_source_identity_is_independent_of_worker_and_retry_save() {
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    insert(
        &mut after,
        CapturedGameplayFact::level_reached(COMMIT - 1, 30),
    );
    let a = facts(Some(&before), &after);
    let b =
        derive_character_game_facts("account", 1, Some(&before), &after, 9, COMMIT + 100).unwrap();
    assert_eq!(a[0].source_event_id, b[0].source_event_id);
    let c = derive_character_game_facts("other", 1, Some(&before), &after, 7, COMMIT).unwrap();
    assert_ne!(a[0].source_event_id, c[0].source_event_id);
    assert_eq!(a[0].source_event_id.len(), 79);
}

#[test]
fn checkpoint_restore_rolls_back_private_pending_evidence() {
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    insert(
        &mut after,
        CapturedGameplayFact::level_reached(COMMIT - 1, 30),
    );
    let encoded = serde_json::to_string(&after).unwrap();
    let restored: CharacterSaveRecord = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        restored.guild_experience_journal.promotion_game_facts.len(),
        1
    );
    assert!(facts(Some(&before), &before).is_empty());
    assert_eq!(facts(Some(&before), &restored).len(), 1);
}

#[test]
fn isolated_postgres_game_sources_commit_rollback_replay_and_preserve_utc_day() {
    if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIPPED native promotion game-source PG test: explicit local opt-in required");
        return;
    }
    let url = std::env::var("MIR2_PROMOTION_TEST_DATABASE_URL")
        .expect("isolated promotion PG URI required");
    let config: postgres::Config = url.parse().unwrap();
    assert!(config
        .get_dbname()
        .is_some_and(|name| name.contains("promotion")));
    assert!(config.get_hosts().iter().all(|host| matches!(host,postgres::config::Host::Tcp(name) if name=="localhost"||name=="127.0.0.1")));
    assert!(config.get_options().is_none());
    let mut base = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let schema = format!("promotion_game_sources_{}_{}", std::process::id(), nonce);
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped = format!("{url}{separator}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000");
    let mut client = postgres::Client::connect(&scoped, postgres::NoTls).unwrap();
    crate::apply_migrations(&mut client).unwrap();
    let before = checkpoint(29);
    let mut after = checkpoint(30);
    after.revision = 1;
    let killed_at = COMMIT / 86_400_000 * 86_400_000 - 100;
    let hash = "a".repeat(64);
    after
        .guild_experience_journal
        .applied_kill_receipts
        .insert("real-zone-key".into(), hash.clone());
    insert(
        &mut after,
        CapturedGameplayFact::combat_kill(killed_at, "real-zone-key", &hash),
    );
    insert(
        &mut after,
        CapturedGameplayFact::level_reached(killed_at, 30),
    );
    let account: crate::AccountRecord =
        serde_json::from_value(json!({"characters":[after.character.clone()],"saves":{}})).unwrap();
    client
        .execute(
            "INSERT INTO accounts(account_id,raw_json) VALUES('account',$1)",
            &[&serde_json::to_value(account).unwrap()],
        )
        .unwrap();
    client.execute("INSERT INTO characters(account_id,character_index,character_name,class,gender,level,raw_json) VALUES('account',1,'source-fixture','Warrior','Male',30,$1)",&[&serde_json::to_value(&after.character).unwrap()]).unwrap();
    client.execute("INSERT INTO character_saves(account_id,character_index,snapshot_json,save_version) VALUES('account',1,$1,6)",&[&serde_json::to_value(&before).unwrap()]).unwrap();
    {
        let mut tx = client.transaction().unwrap();
        let disabled =
            capture_character_source(&mut tx, "account", 1, AccountStoreDatabaseMode::Mirror)
                .unwrap();
        assert!(disabled.is_none());
        tx.execute("UPDATE character_saves SET snapshot_json=$1,save_version=7 WHERE account_id='account' AND character_index=1",&[&serde_json::to_value(&after).unwrap()]).unwrap();
        append_character_source(
            &mut tx,
            Some(GameSourceCapture {
                environment: "test".into(),
                before: Some(before.clone()),
            }),
            "account",
            1,
            &after,
            7,
            COMMIT,
        )
        .unwrap();
        assert_eq!(
            tx.query_one("SELECT count(*) FROM promotion_game_source_events", &[])
                .unwrap()
                .get::<_, i64>(0),
            2
        );
        tx.rollback().unwrap();
    }
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_game_source_events", &[])
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        client
            .query_one(
                "SELECT save_version FROM character_saves WHERE account_id='account'",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
        6
    );
    {
        let mut tx = client.transaction().unwrap();
        tx.execute("UPDATE character_saves SET snapshot_json=$1,save_version=7 WHERE account_id='account' AND character_index=1",&[&serde_json::to_value(&after).unwrap()]).unwrap();
        append_character_source(
            &mut tx,
            Some(GameSourceCapture {
                environment: "test".into(),
                before: Some(before.clone()),
            }),
            "account",
            1,
            &after,
            7,
            COMMIT,
        )
        .unwrap();
        tx.commit().unwrap();
    }
    // Simulate ingestion succeeding and the worker crashing before recording
    // delivery. Restart must replay the same payload without double counting.
    let row = client
        .query_one(
            "SELECT fact_json FROM promotion_game_source_events WHERE source='combat_commit'",
            &[],
        )
        .unwrap();
    let fact: TrustedGameFact = serde_json::from_value(row.get(0)).unwrap();
    let store = PromotionStore::new(&scoped, "test").unwrap();
    store
        .record_committed_game_fact(&fact, COMMIT + 1000)
        .unwrap();
    // Character deletion and a later account ban must not erase previously
    // committed authority evidence. The ordinary unproven API still rejects it.
    client
        .execute(
            "DELETE FROM characters WHERE account_id='account' AND character_index=1",
            &[],
        )
        .unwrap();
    client
        .execute(
            "UPDATE accounts SET is_banned=TRUE WHERE account_id='account'",
            &[],
        )
        .unwrap();
    let level_row = client
        .query_one(
            "SELECT fact_json FROM promotion_game_source_events WHERE source='level_commit'",
            &[],
        )
        .unwrap();
    let level_fact: TrustedGameFact = serde_json::from_value(level_row.get(0)).unwrap();
    assert!(store.record_game_fact(&level_fact, COMMIT + 1000).is_err());
    let mut forged = level_fact.clone();
    forged.source_event_id = "forged-authority-fixture".into();
    assert_eq!(
        store
            .record_committed_game_fact(&forged, COMMIT + 1000)
            .unwrap_err(),
        "promotionCommittedSourceRequired"
    );
    let mut conflicting = level_fact.clone();
    conflicting.level = Some(31);
    assert_eq!(
        store
            .record_committed_game_fact(&conflicting, COMMIT + 1000)
            .unwrap_err(),
        "promotionSourceEventConflict"
    );
    let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let gate = gate.clone();
            let database = scoped.clone();
            std::thread::spawn(move || {
                gate.wait();
                drain_committed_game_events(&database, "test", 100, COMMIT + 1000).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .map(|value| value["delivered"].as_u64().unwrap())
            .sum::<u64>(),
        2
    );
    assert!(results.iter().all(|value| value["failures"] == json!([])));
    assert_eq!(
        drain_committed_game_events(&scoped, "test", 100, COMMIT + 2000).unwrap()["selected"],
        0
    );
    let row=client.query_one("SELECT utc_day,source_occurred_at_ms FROM promotion_game_facts WHERE source='combat_commit'",&[]).unwrap();
    assert_eq!(
        row.get::<_, i64>(0),
        i64::try_from(killed_at / 86_400_000).unwrap()
    );
    assert_eq!(row.get::<_, i64>(1), i64::try_from(killed_at).unwrap());
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_game_facts", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    assert!(client
        .execute(
            "UPDATE promotion_game_source_events SET source_revision=999",
            &[]
        )
        .is_err());
    assert!(client
        .batch_execute("TRUNCATE promotion_game_source_deliveries")
        .is_err());
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM promotion_game_source_deliveries", &[])
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    assert_eq!(
        store
            .record_committed_game_fact(&level_fact, COMMIT + 3000)
            .unwrap()["replayed"],
        true
    );
    eprintln!("native PG gameplay source checks passed: rollback, commit, exact-receipt evidence, crash replay, deleted-character/banned-owner archival, forged evidence rejection, UTC occurrence day, append-only update/truncate");
}
