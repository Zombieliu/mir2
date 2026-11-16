//! Opt-in wallet provisioning proof in a dedicated PostgreSQL database/schema.
//! No wallet signing, shared schema, character creation, or Docker is involved.
use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{AccountStoreDatabaseMode, SimulationConfig, SimulationSession};
use postgres::{Client, Config, NoTls};
use rand_core::{OsRng, RngCore};
use serde_json::{json, Value};

const ACCOUNT: &str = "obl_00112233445566778899aabbccddeeff";
const CLASSIC_SHAPED_ACCOUNT: &str = "wallet_pg_password_guard";
const LOCKED_PASSWORD: &str = "$argon2id$mir2-wallet-locked:v1!";
const LEGACY_LOCKED_PASSWORD: &str = "\u{0}wallet-locked";

fn open(scoped: &str) -> SimulationConfig {
    let config = SimulationConfig::default()
        .with_postgres_account_store(scoped.to_owned())
        .unwrap_or_else(|_| panic!("isolated wallet account load failed; credentials suppressed"));
    assert_eq!(
        config.account_store_database_mode,
        AccountStoreDatabaseMode::SourceOfTruth
    );
    assert!(config.account_store_path.is_none());
    config
}

#[test]
#[ignore = "requires MIR2_WALLET_TEST_ISOLATED=1 and a dedicated mir2_wallet_qa_ PostgreSQL database"]
fn wallet_postgres_provision_survives_restart_without_unlocking_password_login() {
    assert_eq!(
        std::env::var("MIR2_WALLET_TEST_ISOLATED").ok().as_deref(),
        Some("1"),
        "explicit isolated wallet opt-in required"
    );
    let url = std::env::var("MIR2_WALLET_TEST_DATABASE_URL")
        .expect("explicit private wallet QA URI required");
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "isolated wallet QA requires a PostgreSQL URI"
    );
    let parsed: Config = url
        .parse()
        .unwrap_or_else(|_| panic!("invalid wallet QA URI; credentials suppressed"));
    let database_name = parsed.get_dbname().unwrap_or_default();
    assert!(
        database_name.starts_with("mir2_wallet_qa_")
            && (28..=63).contains(&database_name.len())
            && database_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "wallet QA database must have a dedicated generated name"
    );
    assert!(
        parsed.get_options().is_none(),
        "QA base URI cannot override schema options"
    );

    let mut base = Client::connect(&url, NoTls)
        .unwrap_or_else(|_| panic!("isolated wallet connection failed; credentials suppressed"));
    assert_eq!(
        base.query_one("SELECT current_database()", &[])
            .unwrap_or_else(|_| panic!("isolated wallet database identity query failed"))
            .get::<_, String>(0),
        database_name
    );
    let mut random = [0_u8; 16];
    OsRng.fill_bytes(&mut random);
    let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let schema = format!("wallet_qa_{}_{suffix}", std::process::id());
    assert!(schema.len() <= 63);
    base.batch_execute(&format!("CREATE SCHEMA {schema}"))
        .unwrap_or_else(|_| panic!("isolated wallet generated schema creation failed"));
    let scoped = format!(
        "{url}{}options=-csearch_path%3D{schema}%20-clock_timeout%3D5000%20-cstatement_timeout%3D15000",
        if url.contains('?') { "&" } else { "?" }
    );
    let mut observer = Client::connect(&scoped, NoTls)
        .unwrap_or_else(|_| panic!("isolated wallet observer connection failed"));
    assert_eq!(
        observer
            .query_one("SELECT current_schema()", &[])
            .unwrap_or_else(|_| panic!("isolated wallet schema identity query failed"))
            .get::<_, String>(0),
        schema
    );

    // Opening the source applies the real application migrations to this fresh
    // schema. Reproduce the old parameter-$2 failure against its real TEXT field.
    let provisioner = open(&scoped);
    let old_insert = observer.execute(
        "INSERT INTO accounts(account_id,password_snapshot,raw_json) VALUES($1,$2,$3)",
        &[&ACCOUNT, &LEGACY_LOCKED_PASSWORD, &json!({})],
    );
    let error = old_insert.expect_err("PostgreSQL must reject the historical NUL marker");
    assert_eq!(
        error.as_db_error().map(|error| error.code().code()),
        Some("22021"),
        "historical TEXT marker must fail with invalid UTF-8 SQLSTATE"
    );
    assert_eq!(
        observer
            .query_one("SELECT count(*) FROM accounts WHERE account_id=$1", &[&ACCOUNT])
            .unwrap_or_else(|_| panic!("isolated wallet failed-insert observer failed"))
            .get::<_, i64>(0),
        0
    );

    SimulationSession::provision_passkey_account(&provisioner, ACCOUNT)
        .unwrap_or_else(|_| panic!("isolated wallet provisioning failed; details suppressed"));
    let row = observer
        .query_one(
            "SELECT password_snapshot,raw_json FROM accounts WHERE account_id=$1",
            &[&ACCOUNT],
        )
        .unwrap_or_else(|_| panic!("isolated wallet durable account observer failed"));
    assert_eq!(row.get::<_, String>(0), LOCKED_PASSWORD);
    let raw: Value = row.get(1);
    assert_eq!(raw["password"], LOCKED_PASSWORD);
    assert_eq!(raw["characters"], json!([]));
    drop(provisioner);

    let reopened = open(&scoped);
    let mut verified_wallet_session = SimulationSession::new(reopened.clone());
    // This is the trusted simulation boundary used after Gateway proof
    // verification; it does not claim to exercise a phone signature or QR flow.
    assert!(matches!(
        verified_wallet_session.passkey_login(ACCOUNT).as_slice(),
        [ServerPacket::LoginSuccess { characters }] if characters.is_empty()
    ));
    for password in [LOCKED_PASSWORD, LEGACY_LOCKED_PASSWORD, "demo", ""] {
        let mut classic_session = SimulationSession::new(reopened.clone());
        let packets = classic_session.handle_packet(ClientPacket::Login {
            account_id: ACCOUNT.into(),
            password: password.into(),
        });
        assert!(matches!(
            packets.as_slice(),
            [ServerPacket::Login { result: 4 }]
        ));
    }
    // This id fits classic account rules and is outside the `sui:` namespace,
    // so neither namespace nor length checks can mask a password-marker bypass.
    SimulationSession::provision_passkey_account(&reopened, CLASSIC_SHAPED_ACCOUNT)
        .unwrap_or_else(|_| panic!("isolated wallet password-guard provisioning failed"));
    let mut classic_guard = SimulationSession::new(reopened.clone());
    for password in [LOCKED_PASSWORD, LEGACY_LOCKED_PASSWORD] {
        let login = classic_guard.handle_packet(ClientPacket::Login {
            account_id: CLASSIC_SHAPED_ACCOUNT.into(),
            password: password.into(),
        });
        assert!(matches!(login.as_slice(), [ServerPacket::Login { result: 4 }]));
        let changed = classic_guard.handle_packet(ClientPacket::ChangePassword {
            account_id: CLASSIC_SHAPED_ACCOUNT.into(),
            current_password: password.into(),
            new_password: "WalletQAChange123".into(),
        });
        assert!(matches!(
            changed.as_slice(),
            [ServerPacket::ChangePassword { result: 5 }]
        ));
    }
    assert_eq!(
        observer
            .query_one(
                "SELECT password_snapshot FROM accounts WHERE account_id=$1",
                &[&CLASSIC_SHAPED_ACCOUNT],
            )
            .unwrap_or_else(|_| panic!("isolated wallet password-guard observer failed"))
            .get::<_, String>(0),
        LOCKED_PASSWORD
    );
    assert_eq!(
        observer
            .query_one("SELECT count(*) FROM characters WHERE account_id=$1", &[&ACCOUNT])
            .unwrap_or_else(|_| panic!("isolated wallet character-count observer failed"))
            .get::<_, i64>(0),
        0
    );
    // Retain the generated schema as failure evidence. The runner owns disposal
    // of the explicitly isolated database; this test performs no DROP/cleanup.
}
