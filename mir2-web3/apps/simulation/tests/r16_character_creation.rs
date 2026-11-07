use mir2_protocol::{ClientPacket, MirClass, MirGender, ServerPacket};
use mir2_simulation::{
    new_stage5_mail_delivery_nonce, AccountRecord, AccountStore, AccountStoreDatabaseMode,
    AccountStoreRepository, CharacterRecord, CharacterSaveRecord, PostgresAccountStoreRepository,
    SimulationConfig, SimulationSession,
};
use std::collections::BTreeSet;
use std::sync::Barrier;

const PASSWORD: &str = "R16FixturePassword42!";

fn fixture_store() -> AccountStore {
    let mut store = AccountStore::new(SimulationConfig::default().default_character);
    for (account_id, records) in [
        ("create-left", Vec::new()),
        ("create-right", Vec::new()),
        (
            "create-slots",
            vec![(10, "SlotOne"), (11, "SlotTwo"), (12, "SlotThree")],
        ),
        (
            "create-legacy",
            vec![
                (20, "OldTwin"),
                (21, "OldTwo"),
                (22, "OldThree"),
                (23, "OldFour"),
                (24, "OldFive"),
            ],
        ),
        ("create-peer", vec![(30, "oLDTWIN")]),
    ] {
        let mut account = AccountRecord::empty();
        account.password = PASSWORD.into();
        for (index, name) in records {
            let character = CharacterRecord {
                index,
                name: name.into(),
                level: 7,
                class: MirClass::Warrior,
                gender: MirGender::Male,
            };
            let mut save = CharacterSaveRecord::new(character.clone());
            save.gold = 2300 + index as u32;
            account.characters.push(character);
            account.saves.insert(index, save);
        }
        store.accounts.insert(account_id.into(), account);
    }
    store
}

fn login(config: SimulationConfig, account_id: &str) -> SimulationSession {
    let mut session = SimulationSession::new(config);
    let packets = session.handle_packet(ClientPacket::Login {
        account_id: account_id.into(),
        password: PASSWORD.into(),
    });
    assert!(matches!(
        packets.as_slice(),
        [ServerPacket::LoginSuccess { .. }]
    ));
    session
}

#[derive(Debug, PartialEq, Eq)]
enum Creation {
    Created(i32),
    Rejected(u8),
}

fn create(session: &mut SimulationSession, name: &str) -> Creation {
    let packets = session.handle_packet(ClientPacket::NewCharacter {
        name: name.into(),
        gender: MirGender::Female,
        class: MirClass::Wizard,
    });
    match packets.as_slice() {
        [ServerPacket::NewCharacterSuccess { char_info }] => {
            assert_eq!(char_info.name, name);
            assert_eq!(char_info.level, 0);
            Creation::Created(char_info.index)
        }
        [ServerPacket::NewCharacter { result }] => Creation::Rejected(*result),
        _ => panic!("unexpected creation reply"),
    }
}

fn race(
    left: SimulationConfig,
    right: SimulationConfig,
    actors: [(&str, &str); 2],
) -> [Creation; 2] {
    let barrier = Barrier::new(2);
    let [left_actor, right_actor] = actors;
    std::thread::scope(|scope| {
        let left = scope.spawn(|| {
            barrier.wait();
            create(&mut login(left, left_actor.0), left_actor.1)
        });
        let right = scope.spawn(|| {
            barrier.wait();
            create(&mut login(right, right_actor.0), right_actor.1)
        });
        [left.join().unwrap(), right.join().unwrap()]
    })
}

fn assert_races(open: impl Fn() -> SimulationConfig) {
    let [first, second] = race(
        open(),
        open(),
        [
            ("create-left", "Concurrent"),
            ("create-right", "cONCURRENT"),
        ],
    );
    assert!(matches!(
        (first, second),
        (Creation::Created(_), Creation::Rejected(5))
            | (Creation::Rejected(5), Creation::Created(_))
    ));

    let [first, second] = race(
        open(),
        open(),
        [
            ("create-left", "UniqueLeft"),
            ("create-right", "UniqueRight"),
        ],
    );
    assert!(matches!((first, second), (Creation::Created(a), Creation::Created(b)) if a != b));

    // Warm both sessions once so the concurrent request tests the fourth slot,
    // without competing to upgrade the same legacy password.
    login(open(), "create-slots");
    let [first, second] = race(
        open(),
        open(),
        [
            ("create-slots", "FourthLeft"),
            ("create-slots", "FourthRight"),
        ],
    );
    assert!(matches!(
        (first, second),
        (Creation::Created(_), Creation::Rejected(4))
            | (Creation::Rejected(4), Creation::Created(_))
    ));

    // The R16 hard delete releases a name. A second PG configuration that
    // cached it before deletion must take its decision from the database.
    let left = open();
    let right = open();
    let old_index = left.account_store.lock().unwrap().accounts["create-left"]
        .characters
        .iter()
        .find(|character| character.name == "UniqueLeft")
        .unwrap()
        .index;
    let deleted = login(left, "create-left").handle_packet(ClientPacket::DeleteCharacter {
        character_index: old_index,
    });
    assert!(matches!(
        deleted.as_slice(),
        [ServerPacket::DeleteCharacterSuccess { .. }]
    ));
    let recreated = create(&mut login(right, "create-right"), "UniqueLeft");
    assert!(matches!(recreated, Creation::Created(new_index) if new_index != old_index));
}

fn assert_reopened(config: SimulationConfig) {
    let mut legacy = login(config.clone(), "create-legacy");
    // An oversized old roster stays playable. The cap takes precedence over
    // name availability, including a name already duplicated in the old data.
    assert_eq!(create(&mut legacy, "OldTwin"), Creation::Rejected(4));
    let start = legacy.handle_packet(ClientPacket::StartGame {
        character_index: 24,
    });
    assert!(start
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. })));
    assert_eq!(
        create(&mut login(config.clone(), "create-peer"), "OldTwin"),
        Creation::Rejected(5)
    );
    assert_eq!(
        create(&mut login(config.clone(), "create-slots"), "Another"),
        Creation::Rejected(4)
    );

    let store = config.account_store.lock().unwrap();
    let characters: Vec<_> = store
        .accounts
        .values()
        .flat_map(|account| &account.characters)
        .collect();
    assert_eq!(
        characters
            .iter()
            .filter(|character| character.name.eq_ignore_ascii_case("Concurrent"))
            .count(),
        1
    );
    assert_eq!(
        characters
            .iter()
            .filter(|character| character.name.eq_ignore_ascii_case("OldTwin"))
            .count(),
        2
    );
    assert_eq!(
        characters
            .iter()
            .map(|character| character.index)
            .collect::<BTreeSet<_>>()
            .len(),
        characters.len()
    );
    assert_eq!(store.accounts["create-slots"].characters.len(), 4);
    let old = &store.accounts["create-legacy"];
    assert_eq!(
        old.characters
            .iter()
            .map(|character| character.index)
            .collect::<Vec<_>>(),
        [20, 21, 22, 23, 24]
    );
    assert_eq!(old.saves[&24].gold, 2324);
    assert_eq!(old.saves[&24].character.level, 7);
}

#[test]
fn file_creation_serializes_rosters_and_preserves_legacy_data_after_reopen() {
    let root = std::env::temp_dir().join(format!(
        "mir2-r16-create-{}",
        new_stage5_mail_delivery_nonce()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("accounts.json");
    fixture_store().save_to_path(&path).unwrap();
    let config = SimulationConfig::default().with_account_store_path(&path);
    assert_races(|| config.clone());
    drop(config);
    assert_reopened(SimulationConfig::default().with_account_store_path(&path));
}

#[test]
#[ignore = "requires an explicitly isolated native PostgreSQL QA database"]
fn postgres_creation_serializes_across_independent_sources_and_preserves_legacy_data() {
    use postgres::{config::Host, Client, Config, NoTls};

    let url = std::env::var("MIR2_R16_CREATE_TEST_DATABASE_URL")
        .expect("isolated R16 PostgreSQL URI required");
    assert!(url.starts_with("postgresql://"));
    let parsed: Config = url.parse().expect("valid isolated PostgreSQL URI required");
    assert!(matches!(parsed.get_hosts(), [Host::Tcp(host)] if host == "127.0.0.1"));
    assert!(parsed
        .get_hostaddrs()
        .iter()
        .all(|address| address.is_loopback()));
    assert!(parsed.get_options().is_none());
    let database = parsed.get_dbname().unwrap_or_default();
    assert!(database.starts_with("mir2_r16_create_qa_"));
    let schema = format!("create_qa_{}", new_stage5_mail_delivery_nonce());
    assert!(
        schema.len() <= 63
            && schema
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    );
    let mut client = Client::connect(&url, NoTls).expect("isolated PostgreSQL connection failed");
    assert_eq!(
        client
            .query_one("SELECT current_database()", &[])
            .unwrap()
            .get::<_, String>(0),
        database
    );
    client
        .batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap();
    let separator = if url.contains('?') { "&" } else { "?" };
    let scoped = format!("{url}{separator}options=-csearch_path%3D{schema}%20-clock_timeout%3D10000%20-cstatement_timeout%3D20000");
    let mut observer = Client::connect(&scoped, NoTls).expect("isolated schema connection failed");
    assert_eq!(
        observer
            .query_one("SELECT current_schema()", &[])
            .unwrap()
            .get::<_, String>(0),
        schema
    );

    PostgresAccountStoreRepository::new(&scoped, AccountStoreDatabaseMode::Mirror)
        .save(&fixture_store())
        .expect("seed historical roster through PostgreSQL repository");
    let open = || {
        SimulationConfig::default()
            .with_postgres_account_store(scoped.clone())
            .expect("open independent source")
    };
    assert_races(open);
    assert_reopened(open());
    let row = observer.query_one("SELECT count(*) FROM characters WHERE lower(character_name COLLATE \"C\") = 'concurrent'", &[]).unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
}
