//! Original quest-outcome recovery through real Login/StartGame after its
//! Session is lost. No test starts a database or exposes a client source API.
use super::*;
use mir2_protocol::{ClientPacket, ServerPacket};
use postgres::{Client, NoTls};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

const QUEST: i32 = 7;
const DIGEST: &str = "trusted-login-quest-v1";
const GOLD: u64 = 60;

fn isolated_url() -> String {
    let url = std::env::var("MIR2_ECONOMY_TEST_DATABASE_URL")
        .expect("ignored quest login acceptance requires MIR2_ECONOMY_TEST_DATABASE_URL");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let parsed: postgres::Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid isolated acceptance configuration"));
    assert!(parsed
        .get_dbname()
        .is_some_and(|name| name.starts_with("mir2_hunting_acceptance_")));
    assert!(!parsed.get_hosts().is_empty() && parsed.get_hosts().iter().all(|host|
        matches!(host,postgres::config::Host::Tcp(name) if name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()))));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback));
    url
}

fn state(stage: &str) -> String {
    json!({"quest_id":QUEST,"title":"Trusted quest","summary":"","reward_preview":"",
        "required":1,"current":1,"stage":stage,"task_progress":{}})
    .to_string()
}

struct Fixture {
    client: Client,
    config: SimulationConfig,
    account: String,
    realm: String,
    incarnation: String,
    before: CharacterSaveRecord,
    url: String,
}
impl Fixture {
    fn new() -> Self {
        let base = isolated_url();
        let nonce = format!(
            "{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let schema = format!("hunting_quest_login_{nonce}");
        let mut bootstrap = Client::connect(&base, NoTls)
            .unwrap_or_else(|_| panic!("isolated acceptance connection failed"));
        bootstrap
            .batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let separator = if base.contains('?') { '&' } else { '?' };
        let url = format!("{base}{separator}options=-csearch_path%3D{schema}");
        let mut client = Client::connect(&url, NoTls)
            .unwrap_or_else(|_| panic!("isolated schema connection failed"));
        assert_eq!(
            client
                .query_one("SELECT current_schema()", &[])
                .unwrap()
                .get::<_, String>(0),
            schema
        );
        crate::apply_migrations(&mut client).unwrap();

        // Valid ordinary account, complete character checkpoint, offline Ready
        // setup only. The bridge never accepts this data from a player packet.
        let account = "questfixture".to_owned();
        let mut store = AccountStore::new(SimulationConfig::default().default_character);
        let mut record = store.accounts.remove("demo").unwrap();
        let before = record.saves.get_mut(&0).unwrap();
        before.gold = 123;
        before.credit = 17;
        before.quest_states_json = vec![state("readyToTurnIn")];
        let before = before.clone();
        store.accounts.insert(account.clone(), record);
        PostgresAccountStoreRepository::new(&url, AccountStoreDatabaseMode::SourceOfTruth)
            .save(&store)
            .unwrap();
        let realm = format!("quest-login-{nonce}");
        let policy = serde_json::from_value(Self::policy_json("quest-login-v1", true)).unwrap();
        economy::initialize_realm(&mut client, &realm, &policy).unwrap();
        let config = SimulationConfig::default()
            .with_postgres_account_store(&url)
            .unwrap()
            .with_hunting_economy_realm(&realm)
            .unwrap();
        let incarnation = config
            .bind_hunting_character_incarnation(&account, &before)
            .unwrap()
            .unwrap();
        Self {
            client,
            config,
            account,
            realm,
            incarnation,
            before,
            url,
        }
    }

    fn policy_json(version: &str, enabled: bool) -> Value {
        json!({"schema_version":1,"policy_version":version,"enabled":enabled,
            "k":{"numerator":1,"denominator":10},"conversion_fee":{"numerator":1,"denominator":50},
            "daily_decay":{"numerator":1,"denominator":150},"baseline_daily_mint_atoms":720000,
            "daily_mint_cap_multiplier":{"numerator":3,"denominator":2},"business_timezone":"Asia/Shanghai","settlement_hour":4})
    }

    fn prepared(&self) -> PreparedHuntingQuestSource {
        let account = self.account.clone();
        let mut after = self.before.clone();
        after.revision += 1;
        after.quest_states_json = vec![state("completed")];
        self.config
            .prepare_hunting_quest_source(
                &self.account,
                &self.incarnation,
                QUEST,
                DIGEST,
                GOLD,
                self.before.clone(),
                move |store| {
                    store
                        .accounts
                        .get_mut(&account)
                        .unwrap()
                        .saves
                        .insert(0, after);
                    Ok(())
                },
            )
            .unwrap()
    }

    fn unknown(&self, fault: u8) {
        assert!(matches!(
            self.config
                .publish_hunting_quest_source_inner(&self.prepared(), fault),
            Err(HuntingSourceError::OutcomeUnknown(_))
        ));
        assert!(self.config.ensure_account_store_writable().is_err());
    }

    fn cold_config(&self) -> SimulationConfig {
        SimulationConfig::default()
            .with_postgres_account_store(&self.url)
            .unwrap()
            .with_hunting_economy_realm(&self.realm)
            .unwrap()
    }

    fn login(&self, config: SimulationConfig, password: &str) -> crate::SimulationSession {
        let mut session = crate::SimulationSession::new(config);
        let packets = session.handle_packet(ClientPacket::Login {
            account_id: self.account.clone(),
            password: password.into(),
        });
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })),
            "ordinary Login must succeed after its own outcome is resolved"
        );
        session
    }

    fn start(&self, session: &mut crate::SimulationSession) -> CharacterSaveRecord {
        let packets = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(
            packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })),
            "ordinary StartGame must load the current canonical role"
        );
        session.active_character_checkpoint().unwrap()
    }

    fn durable(&mut self) -> CharacterSaveRecord {
        serde_json::from_value(self.client.query_one(
            "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",
            &[&self.account]).unwrap().get(0)).unwrap()
    }

    fn counts(&mut self) -> (i64, i64, i64) {
        let row = self
            .client
            .query_one(
                "SELECT
            (SELECT count(*) FROM hunting_economy_wallets WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_economy_operations WHERE realm_id=$1),
            (SELECT count(*) FROM hunting_quest_source_checkpoints WHERE realm_id=$1)",
                &[&self.realm],
            )
            .unwrap();
        (row.get(0), row.get(1), row.get(2))
    }

    fn bound(&mut self, incarnation: &str) -> Option<u64> {
        economy::read_wallet(
            &mut self.client,
            &self.realm,
            &WalletOwner {
                account_id: self.account.clone(),
                character_index: 0,
                incarnation: incarnation.into(),
            },
        )
        .unwrap()
        .map(|wallet| wallet.bound_gold)
    }

    fn assert_resolved(&self) {
        assert!(self.config.ensure_account_store_writable().is_ok());
        assert!(self.config.hunting_source_fence.lock().unwrap().is_none());
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_recovers_lost_session_commit_and_rollback() {
    for fault in [2, 4] {
        let mut db = Fixture::new();
        // Lose the original real Session, not merely its UI. The new Session
        // retains only a config clone with the server-private original fence.
        let original_session = db.login(db.config.clone(), &default_account_password());
        db.unknown(fault);
        drop(original_session);
        if fault == 2 {
            let policy =
                serde_json::from_value(Fixture::policy_json("quest-login-disabled", false))
                    .unwrap();
            economy::register_policy(&mut db.client, &db.realm, &policy).unwrap();
            economy::activate_policy(&mut db.client, &db.realm, &policy.policy_version, false)
                .unwrap();
        }
        let mut fresh = db.login(db.config.clone(), &default_account_password());
        db.assert_resolved();
        let active = db.start(&mut fresh);
        let committed = i64::from(fault == 2);
        assert_eq!(active.gold, db.before.gold);
        assert_eq!(active.revision, db.before.revision + committed as u64);
        assert_eq!(
            quest_stage(&active, QUEST).unwrap(),
            Some(if fault == 2 {
                QuestStage::Completed
            } else {
                QuestStage::ReadyToTurnIn
            })
        );
        assert_eq!(db.counts(), (committed, committed, committed));
        let incarnation = db.incarnation.clone();
        assert_eq!(
            db.bound(&incarnation),
            if fault == 2 { Some(GOLD) } else { None }
        );
        drop(fresh);
        // Separate config models a cold repository load; it has no in-memory
        // fence, and must observe the same once outcome without re-execution.
        let mut cold = db.login(db.cold_config(), &default_account_password());
        let reloaded = db.start(&mut cold);
        assert_eq!(reloaded.revision, active.revision);
        assert_eq!(reloaded.gold, db.before.gold);
        assert_eq!(reloaded.quest_states_json, active.quest_states_json);
        assert_eq!(db.counts(), (committed, committed, committed));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_keeps_later_canonical_checkpoint() {
    let mut db = Fixture::new();
    db.unknown(2);
    let writer = db.cold_config();
    writer
        .commit_account_store_transaction(&[db.account.clone()], |store| {
            let save = store
                .accounts
                .get_mut(&db.account)
                .unwrap()
                .saves
                .get_mut(&0)
                .unwrap();
            save.revision += 1;
            save.credit += 29;
            save.gold += 11;
            Ok(())
        })
        .unwrap();
    let latest = db.durable();
    let mut fresh = db.login(db.config.clone(), &default_account_password());
    db.assert_resolved();
    let active = db.start(&mut fresh);
    assert_eq!(active.revision, latest.revision);
    assert_eq!(active.credit, latest.credit);
    assert_eq!(active.gold, latest.gold);
    assert_eq!(
        db.config.account_store.lock().unwrap().accounts[&db.account].saves[&0].revision,
        latest.revision
    );
    let original: Value = db
        .client
        .query_one(
            "SELECT checkpoint_json FROM hunting_quest_source_checkpoints WHERE realm_id=$1",
            &[&db.realm],
        )
        .unwrap()
        .get(0);
    assert_eq!(original["revision"].as_u64(), Some(db.before.revision + 1));
    assert_eq!(db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_does_not_bypass_password_or_ban() {
    for banned in [false, true] {
        let mut db = Fixture::new();
        db.unknown(2);
        if banned {
            let raw: Value = db
                .client
                .query_one(
                    "SELECT raw_json FROM accounts WHERE account_id=$1",
                    &[&db.account],
                )
                .unwrap()
                .get(0);
            let mut account: AccountRecord = serde_json::from_value(raw).unwrap();
            account.is_banned = true;
            account.ban_reason = "moderation after uncertain quest".into();
            account.ban_until_ms = None;
            db.client.execute("UPDATE accounts SET is_banned=true,ban_until_ms=NULL,raw_json=$2,store_version=store_version+1 WHERE account_id=$1",
                &[&db.account,&to_json(&account).unwrap()]).unwrap();
        }
        let config = db.config.clone();
        let mut fresh = crate::SimulationSession::new(config.clone());
        let packets = fresh.handle_packet(ClientPacket::Login {
            account_id: db.account.clone(),
            password: if banned {
                default_account_password()
            } else {
                "wrong-password".into()
            },
        });
        assert!(!packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
        assert!(fresh.active_character_checkpoint().is_none());
        db.assert_resolved();
        assert_eq!(
            config.account_store.lock().unwrap().accounts[&db.account].is_banned,
            banned
        );
        let entered = fresh.handle_packet(ClientPacket::StartGame { character_index: 0 });
        assert!(!entered
            .iter()
            .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
        assert_eq!(db.counts(), (1, 1, 1));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_retired_or_missing_owner_never_restores_old_role() {
    for lifecycle in ["recreated", "deleted-character", "deleted-account"] {
        let mut db = Fixture::new();
        let old_source = db.prepared();
        db.unknown(2);
        // Administrative fixture mutations exercise historical recovery after
        // lifecycle changes; ordinary nonzero-wallet deletion remains guarded.
        if lifecycle == "recreated" {
            db.client.execute("UPDATE characters SET hunting_incarnation=gen_random_uuid() WHERE account_id=$1 AND character_index=0",
                &[&db.account]).unwrap();
        } else if lifecycle == "deleted-character" {
            let raw: Value = db
                .client
                .query_one(
                    "SELECT raw_json FROM accounts WHERE account_id=$1",
                    &[&db.account],
                )
                .unwrap()
                .get(0);
            let mut current: AccountRecord = serde_json::from_value(raw).unwrap();
            current.characters.clear();
            current.saves.clear();
            let mut tx = db.client.transaction().unwrap();
            tx.execute(
                "UPDATE accounts SET raw_json=$2,store_version=store_version+1 WHERE account_id=$1",
                &[&db.account, &to_json(&current).unwrap()],
            )
            .unwrap();
            tx.execute("DELETE FROM characters WHERE account_id=$1", &[&db.account])
                .unwrap();
            tx.commit().unwrap();
        } else {
            db.client
                .execute("DELETE FROM accounts WHERE account_id=$1", &[&db.account])
                .unwrap();
        }
        let outcome = db
            .config
            .refresh_account_store_account(&db.account)
            .unwrap();
        db.assert_resolved();
        if lifecycle == "deleted-account" {
            assert_eq!(outcome, AccountSourceRefreshOutcome::Missing);
            {
                let mirror = db.config.account_store.lock().unwrap();
                assert!(!mirror.accounts.contains_key(&db.account));
                assert!(!mirror.source_account_versions.contains_key(&db.account));
                assert!(!mirror.source_save_versions.contains_key(&db.account));
            }
            let mut session = crate::SimulationSession::new(db.config.clone());
            let packets = session.handle_packet(ClientPacket::Login {
                account_id: db.account.clone(),
                password: default_account_password(),
            });
            assert!(!packets
                .iter()
                .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
            assert!(session.active_character_checkpoint().is_none());
        } else if lifecycle == "deleted-character" {
            assert_eq!(outcome, AccountSourceRefreshOutcome::Refreshed);
            let mut session = db.login(db.config.clone(), &default_account_password());
            let entered = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
            assert!(!entered
                .iter()
                .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
            let mirror = db.config.account_store.lock().unwrap();
            assert!(mirror.accounts[&db.account].characters.is_empty());
            assert!(mirror.accounts[&db.account].saves.is_empty());
        } else {
            let canonical = db.durable();
            let witness = db
                .config
                .bind_hunting_character_incarnation(&db.account, &canonical)
                .unwrap()
                .unwrap();
            assert_ne!(witness, db.incarnation);
            assert_eq!(db.bound(&witness), None);
            assert!(matches!(
                db.config.publish_hunting_quest_source(&old_source),
                Err(HuntingSourceError::Rejected(_))
            ));
            let mut session = db.login(db.config.clone(), &default_account_password());
            assert_eq!(db.start(&mut session).revision, canonical.revision);
        }
        assert_eq!(db.counts(), (1, 1, 1));
        let incarnation = db.incarnation.clone();
        assert_eq!(db.bound(&incarnation), Some(GOLD));
    }
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_cannot_clear_foreign_rebound_or_other_account_fence() {
    let mut db = Fixture::new();
    db.unknown(2);
    let before = to_json(&*db.config.account_store.lock().unwrap()).unwrap();
    db.config
        .reconcile_hunting_quest_before_account_refresh("other-account")
        .unwrap();
    let mut rebound = db.config.clone();
    rebound.account_store_database_url =
        Some("postgres://fixture@127.0.0.1:1/mir2_hunting_acceptance_wrong_binding".into());
    rebound
        .reconcile_hunting_quest_before_account_refresh(&db.account)
        .unwrap();
    let mut other_realm = db.config.clone();
    other_realm.hunting_economy_realm = Some("other-realm".into());
    other_realm
        .reconcile_hunting_quest_before_account_refresh(&db.account)
        .unwrap();
    assert!(db.config.ensure_account_store_writable().is_err());
    db.config
        .freeze_account_store_writes("foreign source failure".into());
    let epoch = db.config.account_store_freeze_epoch.load(Ordering::Acquire);
    let fence = db.config.hunting_source_fence.lock().unwrap().clone();
    assert!(db
        .config
        .refresh_account_store_account(&db.account)
        .is_err());
    assert_eq!(
        db.config.account_store_freeze_epoch.load(Ordering::Acquire),
        epoch
    );
    assert_eq!(*db.config.hunting_source_fence.lock().unwrap(), fence);
    assert_eq!(
        to_json(&*db.config.account_store.lock().unwrap()).unwrap(),
        before
    );
    assert!(db.config.ensure_account_store_writable().is_err());
    assert_eq!(db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_incomplete_evidence_remains_frozen() {
    let mut db = Fixture::new();
    db.unknown(4);
    // Deliberately create a trusted economic receipt without the quest half.
    // The new login hook must not mistake it for a complete quest outcome.
    economy::credit_bound_with_source(
        &mut db.client,
        &db.realm,
        &WalletOwner {
            account_id: db.account.clone(),
            character_index: 0,
            incarnation: db.incarnation.clone(),
        },
        &source_key(QUEST),
        DIGEST,
        GOLD,
        |_, _| Ok(()),
    )
    .unwrap();
    let before = to_json(&*db.config.account_store.lock().unwrap()).unwrap();
    let epoch = db.config.account_store_freeze_epoch.load(Ordering::Acquire);
    assert!(db
        .config
        .refresh_account_store_account(&db.account)
        .is_err());
    assert!(db.config.ensure_account_store_writable().is_err());
    assert_eq!(
        db.config.account_store_freeze_epoch.load(Ordering::Acquire),
        epoch
    );
    assert_eq!(
        to_json(&*db.config.account_store.lock().unwrap()).unwrap(),
        before
    );
    assert!(db.config.hunting_source_fence.lock().unwrap().is_some());
    assert_eq!(db.counts(), (1, 1, 0));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_unresolved_realm_lock_keeps_exact_fence() {
    let mut db = Fixture::new();
    db.unknown(2);
    let config = db.config.clone();
    let before = to_json(&*config.account_store.lock().unwrap()).unwrap();
    let fence = config.hunting_source_fence.lock().unwrap().clone();
    let epoch = config.account_store_freeze_epoch.load(Ordering::Acquire);
    let mut blocker = db.client.transaction().unwrap();
    economy::lock_realm_and_read_database_clock(&mut blocker, &db.realm).unwrap();
    assert!(config.refresh_account_store_account(&db.account).is_err());
    assert!(config.ensure_account_store_writable().is_err());
    assert_eq!(
        config.account_store_freeze_epoch.load(Ordering::Acquire),
        epoch
    );
    assert_eq!(*config.hunting_source_fence.lock().unwrap(), fence);
    assert_eq!(
        to_json(&*config.account_store.lock().unwrap()).unwrap(),
        before
    );
    blocker.rollback().unwrap();
    assert!(config.refresh_account_store_account(&db.account).is_ok());
    db.assert_resolved();
    assert_eq!(db.counts(), (1, 1, 1));
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_missing_account_keeps_latest_real_guild_source() {
    let mut db = Fixture::new();
    let guild_id = "123456789abcdef0123456789abcdef0".to_owned();
    let leader_account = "guildleader".to_owned();
    let mut leader = db.before.character.clone();
    leader.index = 1;
    leader.name = "GuildLeader".into();
    leader.level = 22;
    let recruiter = Stage5FriendIdentity {
        account_id: leader_account.clone(),
        character_index: leader.index,
    };
    let member = Stage5FriendIdentity {
        account_id: db.account.clone(),
        character_index: db.before.character.index,
    };
    let seed = AccountStore::new(db.config.default_character.clone());
    let mut guild = guild_clock::schema_tests::fixture_guild(&seed);
    guild.id = guild_id.clone();
    guild.name = "QuestRecoveryGuild".into();
    guild.members[0].identity = recruiter.clone();
    guild.members[0].name = leader.name.clone();
    guild.members[0].membership_epoch = 1;
    // Provision a real canonical Guild/leader, then join through the production
    // membership writer so the quest member's epoch and FK row are derived.
    db.config
        .commit_account_store_transaction_with_guilds(
            &[db.account.clone(), leader_account.clone()],
            std::slice::from_ref(&guild_id),
            |store| {
                store
                    .accounts
                    .insert(leader_account.clone(), AccountRecord::new(leader));
                store.shared_guilds.insert(guild_id.clone(), guild);
                Ok(())
            },
        )
        .unwrap();
    let joined = db
        .config
        .commit_shared_guild_join(&recruiter, &member, &guild_id)
        .unwrap();
    let membership_epoch = joined.member(&member).unwrap().membership_epoch;
    assert!(membership_epoch > 1);
    let original_session = db.login(db.config.clone(), &default_account_password());
    let mut after = db.before.clone();
    let sequence = after.guild_experience_journal.next_sequence + 1;
    let event_id =
        serde_json::to_string(&("guild-xp-v1", &db.account, member.character_index, sequence))
            .unwrap();
    // This is the canonical trusted GainExp source journal (no kill permit),
    // consumed together with the complete quest checkpoint and bound credit.
    // The real consumer, not this fixture, derives the Guild mutation plan.
    after.guild_experience_journal.next_sequence = sequence;
    after
        .guild_experience_journal
        .pending
        .push(GuildExperienceEvent {
            sequence,
            source_revision: db.before.revision,
            event_id: event_id.clone(),
            identity: member.clone(),
            guild_id: guild_id.clone(),
            membership_epoch,
            final_amount: 100,
            capture_hash: None,
        });
    after.experience += 100;
    after.revision += 1;
    after.quest_states_json = vec![state("completed")];
    let account = db.account.clone();
    let source = db
        .config
        .prepare_hunting_quest_source(
            &db.account,
            &db.incarnation,
            QUEST,
            DIGEST,
            GOLD,
            db.before.clone(),
            move |store| {
                store
                    .accounts
                    .get_mut(&account)
                    .unwrap()
                    .saves
                    .insert(0, after);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        source.plan.guilds.keys().cloned().collect::<Vec<_>>(),
        vec![guild_id.clone()],
        "the genuine journal consumer must derive a nonempty Guild write set"
    );
    assert!(matches!(
        db.config.publish_hunting_quest_source_inner(&source, 2),
        Err(HuntingSourceError::OutcomeUnknown(_))
    ));
    drop(original_session);
    assert!(db.config.ensure_account_store_writable().is_err());
    let fence = db
        .config
        .hunting_source_fence
        .lock()
        .unwrap()
        .clone()
        .unwrap();
    assert_eq!(fence.guilds, vec![guild_id.clone()]);
    let row = db
        .client
        .query_one(
            "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
            &[&guild_id],
        )
        .unwrap();
    let committed: SharedGuildRecord = serde_json::from_value(row.get("raw_json")).unwrap();
    let committed_version: i64 = row.get("store_version");
    assert_eq!(committed.experience, joined.experience + 1);
    assert!(committed.experience_receipts.contains(&event_id));
    let durable = db.durable();
    assert!(durable.guild_experience_journal.pending.is_empty());
    assert_eq!(
        durable.guild_experience_journal.event_payloads[&event_id],
        committed.experience_receipt_payloads[&event_id]
    );
    assert_eq!(durable.experience, db.before.experience + 100);

    // A different authoritative instance changes this same Guild after the
    // uncertain COMMIT, removes the old member, then moderation deletes the
    // original account. The Guild still has its independently owned leader.
    db.cold_config()
        .commit_account_store_transaction_with_guilds(
            &[db.account.clone(), leader_account.clone()],
            std::slice::from_ref(&guild_id),
            |store| {
                let guild = store.shared_guilds.get_mut(&guild_id).unwrap();
                guild.gold += 29;
                guild.revision += 1;
                guild.notice.push("later canonical Guild state".into());
                guild.members.retain(|entry| entry.identity != member);
                Ok(())
            },
        )
        .unwrap();
    let row = db
        .client
        .query_one(
            "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
            &[&guild_id],
        )
        .unwrap();
    let latest: SharedGuildRecord = serde_json::from_value(row.get("raw_json")).unwrap();
    let latest_version: i64 = row.get("store_version");
    assert!(latest_version > committed_version);
    assert_eq!(latest.gold, committed.gold + 29);
    assert!(latest.member(&member).is_none());
    db.client
        .execute("DELETE FROM accounts WHERE account_id=$1", &[&db.account])
        .unwrap();

    let refresh_config = db.config.clone();
    assert_eq!(
        refresh_config
            .refresh_account_store_account(&db.account)
            .unwrap(),
        AccountSourceRefreshOutcome::Missing
    );
    db.assert_resolved();
    let mirror = refresh_config.account_store.lock().unwrap();
    assert!(!mirror.accounts.contains_key(&db.account));
    assert!(!mirror.source_account_versions.contains_key(&db.account));
    assert!(!mirror.source_save_versions.contains_key(&db.account));
    assert_eq!(mirror.shared_guilds[&guild_id], latest);
    assert_eq!(mirror.source_guild_versions[&guild_id], latest_version);
    drop(mirror);
    // Resolving a missing account never executes its source again or removes
    // the historic wallet/source evidence that survived account retirement.
    assert_eq!(db.counts(), (1, 1, 1));
    let incarnation = db.incarnation.clone();
    assert_eq!(db.bound(&incarnation), Some(GOLD));
    let row = db
        .client
        .query_one(
            "SELECT raw_json,store_version FROM shared_guilds WHERE guild_id=$1",
            &[&guild_id],
        )
        .unwrap();
    assert_eq!(row.get::<_, Value>("raw_json"), to_json(&latest).unwrap());
    assert_eq!(row.get::<_, i64>("store_version"), latest_version);
}

#[test]
#[ignore = "requires explicitly isolated loopback PostgreSQL acceptance database"]
fn hunting_source_postgres_login_refresh_current_checkpoint_mismatch_keeps_own_fence() {
    for mismatch in ["snapshot", "name"] {
        let mut db = Fixture::new();
        db.unknown(2);
        let original_snapshot: Value = db
            .client
            .query_one(
                "SELECT snapshot_json FROM character_saves WHERE account_id=$1 AND character_index=0",
                &[&db.account],
            )
            .unwrap()
            .get(0);
        let original_name: String = db
            .client
            .query_one(
                "SELECT character_name FROM characters WHERE account_id=$1 AND character_index=0",
                &[&db.account],
            )
            .unwrap()
            .get(0);
        if mismatch == "snapshot" {
            let mut inconsistent = original_snapshot.clone();
            inconsistent["credit"] = json!(db.before.credit + 1);
            db.client.execute(
                "UPDATE character_saves SET snapshot_json=$2 WHERE account_id=$1 AND character_index=0",
                &[&db.account,&inconsistent],
            ).unwrap();
        } else {
            db.client.execute(
                "UPDATE characters SET character_name='MismatchedRole' WHERE account_id=$1 AND character_index=0",
                &[&db.account],
            ).unwrap();
        }
        let config = db.config.clone();
        let memory = to_json(&*config.account_store.lock().unwrap()).unwrap();
        let fence = config.hunting_source_fence.lock().unwrap().clone();
        let epoch = config.account_store_freeze_epoch.load(Ordering::Acquire);
        let reason = match &*config.account_store_write_state.lock().unwrap() {
            AccountStoreWriteState::Frozen { reason } => reason.clone(),
            AccountStoreWriteState::Writable => panic!("original source freeze missing"),
        };
        assert!(config.refresh_account_store_account(&db.account).is_err());
        assert!(config.ensure_account_store_writable().is_err());
        assert_eq!(
            config.account_store_freeze_epoch.load(Ordering::Acquire),
            epoch
        );
        assert_eq!(*config.hunting_source_fence.lock().unwrap(), fence);
        assert!(matches!(&*config.account_store_write_state.lock().unwrap(),
            AccountStoreWriteState::Frozen { reason: current } if current == &reason));
        assert_eq!(
            to_json(&*config.account_store.lock().unwrap()).unwrap(),
            memory
        );
        assert_eq!(db.counts(), (1, 1, 1));

        // Only restoring CURRENT canonical consistency permits resolution. The
        // immutable original source evidence itself is never modified here.
        if mismatch == "snapshot" {
            db.client.execute(
                "UPDATE character_saves SET snapshot_json=$2 WHERE account_id=$1 AND character_index=0",
                &[&db.account,&original_snapshot],
            ).unwrap();
        } else {
            db.client.execute(
                "UPDATE characters SET character_name=$2 WHERE account_id=$1 AND character_index=0",
                &[&db.account,&original_name],
            ).unwrap();
        }
        assert!(config.refresh_account_store_account(&db.account).is_ok());
        db.assert_resolved();
        assert_eq!(
            to_json(&config.account_store.lock().unwrap().accounts[&db.account].saves[&0]).unwrap(),
            original_snapshot
        );
        assert_eq!(db.counts(), (1, 1, 1));
    }
}
