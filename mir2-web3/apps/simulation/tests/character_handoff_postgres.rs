//! Opt-in proof against an owned, loopback-only PostgreSQL Source authority.
//! Uses the activities CI database opt-in, creates its own random schema, and
//! retains that schema as evidence. It never reads .env or drops existing data.
use mir2_protocol::{ClientPacket, MirClass, MirDirection, MirGender, ServerPacket};
use mir2_simulation::{
    deliver_stage5_system_mail, new_stage5_mail_delivery_nonce, AccountRecord,
    AccountStoreDatabaseMode, CharacterHandoffCheckpoint, CharacterHandoffCommitment,
    CharacterSaveRecord, InProcessWorldRuntime, SimulationConfig, Stage5MailDelivery,
    Stage5MailMessage, Stage5MailTargetKind, Stage5SystemsState, WorldCommand, WorldRuntime,
};
use postgres::config::Host;
use postgres::{Client, Config, NoTls};
use serde_json::Value;
use std::collections::BTreeSet;
use std::net::IpAddr;

const PASSWORD: &str = "IsolatedHandoffFixture42!";
const GOLD: u32 = 321;
const CREDIT: u32 = 87;

fn checked<T>(result: Result<T, String>, operation: &str) -> T {
    result.unwrap_or_else(|error| {
        let class = if error.contains("OUTCOME_UNKNOWN") {
            "outcome-unknown"
        } else if error.contains("stale postgres") {
            "stale-source-CAS"
        } else if error.contains("stale full character") {
            "stale-character-revision"
        } else {
            "rejected"
        };
        panic!("isolated handoff {operation}: {class}; credentials suppressed")
    })
}

fn owned_database() -> (String, Client) {
    assert_eq!(
        std::env::var("MIR2_ACTIVITY_TEST_ISOLATED").ok().as_deref(),
        Some("1"),
        "explicit isolated activities database opt-in required"
    );
    let url = std::env::var("MIR2_ACTIVITY_TEST_DATABASE_URL")
        .expect("explicit private activities QA URI required");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "handoff QA requires a PostgreSQL URI"
    );
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid handoff QA URI; credentials suppressed"));
    let name = parsed.get_dbname().unwrap_or_default();
    assert!(
        name.starts_with("mir2_activities_qa_")
            && (28..=63).contains(&name.len())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "handoff QA database must have a dedicated generated name"
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );
    // Validate both host and hostaddr: a loopback hostname must not hide an
    // override that actually connects to a remote database. No socket/default
    // host or environment-derived connection target is accepted.
    assert!(
        !parsed.get_hosts().is_empty()
            && parsed.get_hosts().iter().all(|host| match host {
                Host::Tcp(host) => host
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback()),
                _ => false,
            })
            && parsed.get_hostaddrs().iter().all(IpAddr::is_loopback),
        "handoff QA requires explicit loopback IP hosts and hostaddrs"
    );
    let schema = format!(
        "handoff_qa_{}_{}",
        std::process::id(),
        new_stage5_mail_delivery_nonce()
    );
    assert!(
        schema.len() <= 63
            && schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    );
    let mut base = Client::connect(&url, NoTls)
        .unwrap_or_else(|_| panic!("isolated handoff connection failed; credentials suppressed"));
    assert_eq!(
        base.query_one("SELECT current_database()", &[])
            .unwrap_or_else(|_| panic!("isolated handoff database identity query failed"))
            .get::<_, String>(0),
        name
    );
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap_or_else(|_| panic!("isolated handoff schema creation failed"));
    let scoped = format!(
        "{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" }
    );
    let mut observer = Client::connect(&scoped, NoTls)
        .unwrap_or_else(|_| panic!("isolated handoff observer connection failed"));
    assert_eq!(
        observer
            .query_one("SELECT current_schema()", &[])
            .unwrap_or_else(|_| panic!("isolated handoff schema identity query failed"))
            .get::<_, String>(0),
        schema
    );
    (scoped, observer)
}

fn open(scoped: &str) -> SimulationConfig {
    let mut config = SimulationConfig::default()
        .with_platinum_176_profile()
        .with_postgres_account_store(scoped.to_owned())
        .unwrap_or_else(|_| panic!("isolated handoff Source load failed; credentials suppressed"));
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::SourceOfTruth
    );
    assert!(config.account_store_path.is_none());
    config.visible_monsters.clear();
    config.map_hazards.clear();
    config
}

struct Actor {
    account_id: String,
    character_name: String,
    character_index: i32,
}

struct Durable {
    account_raw: Value,
    save_raw: Value,
    save: CharacterSaveRecord,
    account_version: i64,
    save_version: i64,
}

fn durable(db: &mut Client, actor: &Actor) -> Durable {
    let account_row = db
        .query_one(
            "SELECT raw_json,store_version FROM accounts WHERE account_id=$1",
            &[&actor.account_id],
        )
        .unwrap_or_else(|_| panic!("isolated handoff account observer failed"));
    let save_row = db
        .query_one(
            "SELECT snapshot_json,save_version FROM character_saves WHERE account_id=$1 AND character_index=$2",
            &[&actor.account_id, &actor.character_index],
        )
        .unwrap_or_else(|_| panic!("isolated handoff character observer failed"));
    let account_raw: Value = account_row.get(0);
    let save_raw: Value = save_row.get(0);
    let account: AccountRecord = serde_json::from_value(account_raw.clone())
        .expect("owned handoff account JSON must decode");
    let save: CharacterSaveRecord =
        serde_json::from_value(save_raw.clone()).expect("owned handoff character JSON must decode");
    assert_eq!(save.character.index, actor.character_index);
    assert_eq!(save.character.name, actor.character_name);
    assert_eq!(
        serde_json::to_value(&account.saves[&actor.character_index]).unwrap(),
        save_raw,
        "complete account save and authoritative SQL snapshot must agree"
    );
    Durable {
        account_raw,
        save_raw,
        save,
        account_version: account_row.get(1),
        save_version: save_row.get(1),
    }
}

fn unchanged(before: &Durable, after: &Durable) {
    // Avoid printing even the fixture's Argon2 password hash on assertion failure.
    assert!(
        before.account_raw == after.account_raw,
        "account row changed"
    );
    assert_eq!(before.save_raw, after.save_raw, "complete save row changed");
    assert_eq!(before.account_version, after.account_version);
    assert_eq!(before.save_version, after.save_version);
}

fn login(runtime: &mut InProcessWorldRuntime, actor: &Actor) {
    let packets = checked(
        runtime.execute(WorldCommand::ClientPacket(ClientPacket::Login {
            account_id: actor.account_id.clone(),
            password: PASSWORD.into(),
        })),
        "ordinary password login",
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::LoginSuccess { .. })));
    assert!(runtime.active_identity().is_none(), "login stays in Select");
}

fn select(scoped: &str, actor: &Actor) -> InProcessWorldRuntime {
    let mut runtime = InProcessWorldRuntime::new(open(scoped));
    login(&mut runtime, actor);
    runtime
}

fn register_character(scoped: &str) -> Actor {
    let nonce = new_stage5_mail_delivery_nonce();
    let mut actor = Actor {
        account_id: format!("handoff_{}", &nonce[..20]),
        character_name: format!("Hf{}", &nonce[..9]),
        character_index: -1,
    };
    let config = open(scoped);
    let mut provision = InProcessWorldRuntime::new(config.clone());
    let created = checked(
        provision.execute(WorldCommand::ClientPacket(ClientPacket::NewAccount {
            account_id: actor.account_id.clone(),
            password: PASSWORD.into(),
            birth_date_binary: 0,
            user_name: String::new(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        })),
        "real account registration",
    );
    assert!(matches!(
        created.as_slice(),
        [ServerPacket::NewAccount { result: 8 }]
    ));
    login(&mut provision, &actor);
    let created = checked(
        provision.execute(WorldCommand::ClientPacket(ClientPacket::NewCharacter {
            name: actor.character_name.clone(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        })),
        "real character registration",
    );
    actor.character_index = created
        .iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(char_info.index),
            _ => None,
        })
        .expect("normal NewCharacter must return its durable character index");
    // Only this newly registered, still-unadmitted fixture is initialized.
    // Admission and every later save use the real Source transaction path.
    {
        let mut store = config.account_store.lock().unwrap();
        let account = store.accounts.get_mut(&actor.account_id).unwrap();
        assert!(account.password.starts_with("$argon2id$"));
        let save = account.saves.get_mut(&actor.character_index).unwrap();
        assert_eq!(save.default_npc_events.next_sequence, 0);
        save.gold = GOLD;
        save.credit = CREDIT;
    }
    checked(
        config.save_account_store_account(&actor.account_id),
        "owned pre-admission wallet fixture",
    );
    actor
}

fn start(runtime: &mut InProcessWorldRuntime, actor: &Actor) {
    let packets = checked(
        runtime.execute(WorldCommand::ClientPacket(ClientPacket::StartGame {
            character_index: actor.character_index,
        })),
        "ordinary StartGame",
    );
    assert!(packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    let identity = runtime.active_identity().expect("StartGame must admit");
    assert_eq!(identity.account_id, actor.account_id);
    assert_eq!(identity.character_index, actor.character_index);
    assert_eq!(identity.character_name, actor.character_name);
}

fn inventory_uids(save: &CharacterSaveRecord) -> Vec<u64> {
    let ids = save
        .inventory_items_json
        .iter()
        .map(|item| {
            serde_json::from_str::<Value>(item).unwrap()["unique_id"]
                .as_u64()
                .expect("saved starter item must carry its actual UID")
        })
        .collect::<Vec<_>>();
    assert!(!ids.is_empty(), "UID preservation needs a real starter bag");
    assert!(ids.iter().all(|id| *id > 0));
    assert_eq!(
        ids.iter().copied().collect::<BTreeSet<_>>().len(),
        ids.len()
    );
    ids
}

fn assert_resume(
    target: &mut InProcessWorldRuntime,
    actor: &Actor,
    checkpoint: &CharacterHandoffCheckpoint,
) -> CharacterHandoffCommitment {
    assert!(target.active_identity().is_none());
    let committed = checked(
        target.resume_character_handoff(checkpoint),
        "Source continuation into authenticated Select",
    );
    // This compares the entire image, including all journals and histories,
    // allowing only the API's witnessed skill/refine clock representation.
    checked(
        checkpoint.validate_commitment(&committed),
        "complete runtime commitment",
    );
    let identity = target.active_identity().expect("handoff must admit");
    assert_eq!(identity.account_id, actor.account_id);
    assert_eq!(identity.character_index, actor.character_index);
    assert_eq!(identity.character_name, actor.character_name);
    assert_eq!(target.current_tick(), checkpoint.source_tick);
    assert_eq!(committed.runtime.revision, checkpoint.durable.revision);
    assert_eq!(committed.runtime.gold, GOLD);
    assert_eq!(committed.runtime.credit, CREDIT);
    assert_eq!(
        committed.runtime.inventory_items_json,
        checkpoint.durable.inventory_items_json
    );
    assert_eq!(
        inventory_uids(&committed.runtime),
        inventory_uids(&checkpoint.durable)
    );
    assert_eq!(
        committed.runtime.default_npc_events, checkpoint.durable.default_npc_events,
        "handoff must not enqueue another Login event"
    );
    committed
}

fn mail(save: &CharacterSaveRecord) -> Vec<Stage5MailMessage> {
    serde_json::from_str::<Stage5SystemsState>(save.stage5_systems_json.as_deref().unwrap())
        .unwrap()
        .mail
}

#[test]
#[ignore = "requires MIR2_ACTIVITY_TEST_ISOLATED=1 and an owned loopback mir2_activities_qa_ PostgreSQL database"]
fn source_handoff_preserves_one_admission_and_rejects_a_stale_complete_postgres_row() {
    let (scoped, mut observer) = owned_database();
    let actor = register_character(&scoped);
    let registered = durable(&mut observer, &actor);
    let mut source = select(&scoped, &actor);
    unchanged(&registered, &durable(&mut observer, &actor));
    start(&mut source, &actor);
    let admitted = durable(&mut observer, &actor);
    assert_eq!(admitted.save.default_npc_events.next_sequence, 1);
    assert_eq!(admitted.save.default_npc_events.committed_sequence, 1);
    assert!(admitted.save.default_npc_events.pending.is_empty());
    assert!(admitted.save.revision > registered.save.revision);
    assert_eq!(admitted.save.gold, GOLD);
    assert_eq!(admitted.save.credit, CREDIT);
    inventory_uids(&admitted.save);
    checked(source.execute(WorldCommand::Tick), "ordinary source tick");
    assert!(source.current_tick() > 0);

    // A genuinely independent authority cache delivers mail while the source
    // owns an active world. prepare must merge the committed external mail.
    let delivery = checked(
        deliver_stage5_system_mail(
            &open(&scoped),
            Stage5MailDelivery {
                target_kind: Stage5MailTargetKind::Account,
                target_id: actor.account_id.clone(),
                from: "Isolated handoff fixture".into(),
                subject: "Committed external mail".into(),
                body: "Must survive preparation and continuation unchanged".into(),
                gold: 17,
                items: Vec::new(),
            },
        ),
        "independent external mail transaction",
    );
    assert_eq!(delivery.delivered_count, 1);
    assert_eq!(delivery.mail_ids.len(), 1);
    let delivered = durable(&mut observer, &actor);
    let delivered_mail = mail(&delivered.save);
    let external = delivered_mail
        .iter()
        .find(|mail| mail.id == delivery.mail_ids[0])
        .expect("external mail must already exist in the real SQL row");
    assert_eq!(external.subject, "Committed external mail");
    assert_eq!(external.gold, 17);
    assert!(!external.delivery_nonce.is_empty());
    assert!(!external.claimed);

    let checkpoint = checked(
        source.prepare_character_handoff(),
        "complete Source preparation",
    );
    let prepared = durable(&mut observer, &actor);
    assert_eq!(
        serde_json::to_value(&checkpoint.durable).unwrap(),
        prepared.save_raw
    );
    assert!(checkpoint.durable.revision > admitted.save.revision);
    assert_eq!(checkpoint.source_tick, source.current_tick());
    assert_eq!(mail(&checkpoint.durable), delivered_mail);
    let mut target = select(&scoped, &actor);
    unchanged(&prepared, &durable(&mut observer, &actor));
    assert!(target
        .restore_active_character_checkpoint(&checkpoint.runtime)
        .is_err());
    assert!(target.active_identity().is_none());
    unchanged(&prepared, &durable(&mut observer, &actor));
    let committed = assert_resume(&mut target, &actor, &checkpoint);
    assert_eq!(mail(&committed.runtime), delivered_mail);
    unchanged(&prepared, &durable(&mut observer, &actor));

    // A later ordinary admission is intentionally separate from handoff. It
    // still executes exactly one Login hook and publishes its own revision.
    let mut ordinary = select(&scoped, &actor);
    unchanged(&prepared, &durable(&mut observer, &actor));
    start(&mut ordinary, &actor);
    let ordinary_save = durable(&mut observer, &actor);
    assert_eq!(
        ordinary_save.save.default_npc_events.next_sequence,
        checkpoint.durable.default_npc_events.next_sequence + 1
    );
    assert_eq!(
        ordinary_save.save.default_npc_events.committed_sequence,
        checkpoint.durable.default_npc_events.committed_sequence + 1
    );
    assert!(ordinary_save.save.default_npc_events.pending.is_empty());
    assert!(ordinary_save.save.revision > checkpoint.durable.revision);

    // Use another real actor so the previous ordinary admission cannot expire
    // this Source's DTO before the stale-save scenario even begins.
    let actor = register_character(&scoped);
    let mut source = select(&scoped, &actor);
    start(&mut source, &actor);
    let old = checked(
        source.prepare_character_handoff(),
        "initial stale-case preparation",
    );
    let old_row = durable(&mut observer, &actor);
    assert_eq!(
        serde_json::to_value(&old.durable).unwrap(),
        old_row.save_raw
    );
    checked(
        source.execute(WorldCommand::ClientPacket(ClientPacket::Turn {
            direction: MirDirection::Left,
        })),
        "ordinary Source transform change",
    );
    checked(source.save_active_character(), "new complete Source save");
    let advanced = durable(&mut observer, &actor);
    assert!(advanced.save.revision > old.durable.revision);
    assert_eq!(advanced.save.direction, MirDirection::Left);
    assert_eq!(
        advanced.save.default_npc_events,
        old.durable.default_npc_events
    );
    assert_ne!(advanced.save_raw, old_row.save_raw);
    let mut rejected = select(&scoped, &actor);
    unchanged(&advanced, &durable(&mut observer, &actor));
    let rejected_tick = rejected.current_tick();
    assert!(rejected.resume_character_handoff(&old).is_err());
    assert!(rejected.active_identity().is_none());
    assert_eq!(rejected.current_tick(), rejected_tick);
    unchanged(&advanced, &durable(&mut observer, &actor));

    let fresh = checked(
        source.prepare_character_handoff(),
        "fresh Source preparation",
    );
    let fresh_row = durable(&mut observer, &actor);
    assert!(fresh.durable.revision > advanced.save.revision);
    assert_eq!(
        serde_json::to_value(&fresh.durable).unwrap(),
        fresh_row.save_raw
    );
    assert_eq!(fresh.durable.direction, MirDirection::Left);
    assert_eq!(
        fresh.durable.default_npc_events,
        old.durable.default_npc_events
    );
    let mut fresh_target = select(&scoped, &actor);
    unchanged(&fresh_row, &durable(&mut observer, &actor));
    assert_resume(&mut fresh_target, &actor, &fresh);
    unchanged(&fresh_row, &durable(&mut observer, &actor));
}
