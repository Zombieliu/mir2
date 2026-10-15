//! Trusted, insert-only provisioning for a bounded invited-playtest load cohort.
//! Never exposed by the Gateway. No characters, login, StartGame or gameplay
//! grants are created. Credentials use the ordinary NewAccount hashing path.
//!
//! Private input: {"batch":"8randomx","provisioning":"admin-provisioned",
//! "accounts":[{"accountId":"lt8randomx01","password":"..."}]}. Generate the
//! eight-character batch with a CSPRNG; this tool checks its shape and freshness.
//! URL is accepted only through MIR2_PLAYTEST_SEED_DATABASE_URL and must name
//! mir2_playtest/mir2_playtest over a literal loopback SSH-forward port, not 5432.
//! Default dry-run uses a READ ONLY transaction. Only --apply inserts rows.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    validate_commercial_identity_credentials, AccountRecord, SimulationConfig, SimulationSession,
};
use postgres::config::{Host, SslMode};
use postgres::{Client, Config, IsolationLevel, NoTls, Transaction};
use serde::{Deserialize, Serialize};

const URL_ENV: &str = "MIR2_PLAYTEST_SEED_DATABASE_URL";
const REALM: &str = "mir2_playtest";
const MAX_ACCOUNTS: usize = 14;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024;
type Result<T> = std::result::Result<T, &'static str>;

// Same columns/projection as config.rs::upsert_account_record, deliberately
// without its ON CONFLICT/UPDATE path. PostgreSQL uniqueness + one transaction
// fail closed even if ordinary registration races the preflight SELECT.
const INSERT_ACCOUNT: &str = "INSERT INTO public.accounts (
    account_id, password_snapshot, storage_size, has_expanded_storage,
    expanded_storage_expiry_time_binary_datetime, storage_password_snapshot,
    storage_password_last_set_binary_datetime, is_banned, ban_reason,
    ban_until_ms, banned_at_ms, store_version, raw_json, updated_at
) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,1,$12,now())";

struct Options {
    manifest: PathBuf,
    apply: bool,
}

// Do not derive Debug: the private input contains plaintext credentials.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    batch: String,
    provisioning: String,
    accounts: Vec<Credentials>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Credentials {
    account_id: String,
    password: String,
}

struct PreparedAccount {
    account_id: String,
    record: AccountRecord,
    raw_json: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    provisioning: &'static str,
    mode: &'static str,
    requested: usize,
    inserted: usize,
    created_count: usize,
    account_ids: Vec<String>,
    database: String,
    user: String,
    database_identity_verified: bool,
    transaction_committed: bool,
    existing_collision: bool,
    characters_created: usize,
    passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

fn main() {
    // Never emit a packet, input record, URL, password or PHC hash on panic.
    std::panic::set_hook(Box::new(|_| {
        eprintln!("account provisioning failed; panic details suppressed");
    }));
    match run() {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{}", serde_json::json!({"passed": false, "error": error}));
            std::process::exit(1);
        }
    }
}

fn run() -> Result<bool> {
    let args = env::args_os()
        .skip(1)
        .map(|value| value.into_string().map_err(|_| "arguments must be Unicode"))
        .collect::<Result<Vec<_>>>()?;
    if args == ["--help"] || args == ["-h"] {
        println!("Usage: seed_playtest_pg_accounts --manifest ABSOLUTE_PRIVATE_JSON [--dry-run | --apply]\nDefault: read-only database preflight, no inserts.\nURL env: {URL_ENV}; literal 127.0.0.1 or [::1], explicit SSH-forward port other than 5432, database/user mir2_playtest, no query/options.\nInput: {{\"batch\":\"8randomx\",\"provisioning\":\"admin-provisioned\",\"accounts\":[{{\"accountId\":\"lt8randomx01\",\"password\":\"<private>\"}}]}}\nUse a fresh CSPRNG batch; suffixes 01..14; at most 14 new accounts. No characters or gameplay grants.");
        return Ok(true);
    }
    let options = parse_options(&args)?;
    let manifest = read_manifest(&options.manifest)?;
    let database_url = env::var(URL_ENV).map_err(|_| "database URL env is required")?;
    let mut connection = parse_database_url(&database_url)?
        .connect(NoTls)
        .map_err(|_| "loopback database connection failed; details suppressed")?;
    let report = provision(&mut connection, &manifest, options.apply)?;
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|_| "report serialization failed")?
    );
    Ok(report.passed)
}

fn parse_options(args: &[String]) -> Result<Options> {
    let mut manifest = None;
    let mut mode = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" if manifest.is_none() => {
                index += 1;
                let value = args.get(index).ok_or("manifest path is required")?;
                if value.starts_with('-') || value.is_empty() {
                    return Err("manifest path is required");
                }
                manifest = Some(PathBuf::from(value));
            }
            "--apply" if mode.is_none() => mode = Some(true),
            "--dry-run" if mode.is_none() => mode = Some(false),
            _ => return Err("unsupported or duplicate option; credentials belong in private JSON"),
        }
        index += 1;
    }
    let manifest = manifest.ok_or("--manifest is required")?;
    if !manifest.is_absolute()
        || manifest.extension().and_then(|value| value.to_str()) != Some("json")
    {
        return Err("manifest must be an absolute JSON path");
    }
    Ok(Options {
        manifest,
        apply: mode.unwrap_or(false),
    })
}

fn parse_database_url(value: &str) -> Result<Config> {
    const BAD_URL: &str = "database URL must target only the isolated loopback SSH-forward realm";
    if !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(BAD_URL);
    }
    let url = reqwest::Url::parse(value).map_err(|_| BAD_URL)?;
    let port = url
        .port()
        .filter(|port| *port != 0 && *port != 5432)
        .ok_or(BAD_URL)?;
    if !matches!(url.scheme(), "postgres" | "postgresql")
        || url.username() != REALM
        || url.password().is_none_or(str::is_empty)
        || url.path() != "/mir2_playtest"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(BAD_URL);
    }
    // Exact authority syntax rules out host lists, DNS, hostaddr, percent-encoded
    // hosts, shortened IPv4 forms and parser disagreement about the target.
    let authority = value
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/').map(|(authority, _)| authority))
        .ok_or(BAD_URL)?;
    if authority.bytes().filter(|byte| *byte == b'@').count() != 1 {
        return Err(BAD_URL);
    }
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .ok_or(BAD_URL)?;
    if host_port != format!("127.0.0.1:{port}") && host_port != format!("[::1]:{port}") {
        return Err(BAD_URL);
    }
    let mut config = value.parse::<Config>().map_err(|_| BAD_URL)?;
    if config.get_user() != Some(REALM)
        || config.get_dbname() != Some(REALM)
        || config.get_ports() != [port]
        || config.get_hosts().len() != 1
        || !matches!(&config.get_hosts()[0], Host::Tcp(host) if host == "127.0.0.1" || host == "::1")
        || !config.get_hostaddrs().is_empty()
        || config.get_options().is_some()
        || config
            .get_password()
            .is_none_or(|password| password.is_empty() || password.iter().any(u8::is_ascii_control))
    {
        return Err(BAD_URL);
    }
    config
        .ssl_mode(SslMode::Disable)
        .connect_timeout(Duration::from_secs(5))
        .application_name("mir2-admin-provisioned-load-cohort");
    Ok(config)
}

fn read_manifest(path: &Path) -> Result<Manifest> {
    for ancestor in path.ancestors() {
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| "private manifest path unavailable")?;
        if metadata.file_type().is_symlink() {
            return Err("private manifest may not traverse links");
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("private manifest may not traverse reparse points");
            }
        }
    }
    let file = fs::File::open(path).map_err(|_| "private manifest could not be opened")?;
    let metadata = file
        .metadata()
        .map_err(|_| "private manifest metadata unavailable")?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err("private manifest must be a regular file of at most 16 KiB");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("private manifest requires owner-only permissions");
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private manifest read failed")?;
    parse_manifest(&bytes)
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("private manifest exceeds 16 KiB");
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| "private manifest JSON/schema invalid")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.provisioning != "admin-provisioned"
        || manifest.batch.len() != 8
        || !manifest
            .batch
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || manifest.accounts.is_empty()
        || manifest.accounts.len() > MAX_ACCOUNTS
    {
        return Err("require an admin-provisioned 8-character cohort of 1..14 accounts");
    }
    let prefix = format!("lt{}", manifest.batch);
    let mut unique = BTreeSet::new();
    for credentials in &manifest.accounts {
        let suffix = credentials
            .account_id
            .strip_prefix(&prefix)
            .ok_or("account id is outside the cohort")?;
        if suffix.len() != 2
            || !suffix.bytes().all(|byte| byte.is_ascii_digit())
            || !matches!(suffix.parse::<usize>(), Ok(1..=MAX_ACCOUNTS))
            || !unique.insert(&credentials.account_id)
        {
            return Err("account ids require unique lt + batch + 01..14 suffixes");
        }
        validate_commercial_identity_credentials(&credentials.account_id, &credentials.password)
            .map_err(|_| "credentials do not meet commercial policy")?;
    }
    Ok(())
}

fn prepare_accounts(manifest: &Manifest) -> Result<Vec<PreparedAccount>> {
    validate_manifest(manifest)?;
    let config = SimulationConfig::default();
    if config.account_store_path.is_some() || config.account_store_database_url.is_some() {
        return Err("in-memory account creation unexpectedly has persistence configured");
    }
    config
        .account_store
        .lock()
        .map_err(|_| "in-memory account lock failed")?
        .accounts
        .clear();
    let mut session = SimulationSession::new(config.clone());
    for credentials in &manifest.accounts {
        let packets = session
            .try_handle_packet(ClientPacket::NewAccount {
                account_id: credentials.account_id.clone(),
                password: credentials.password.clone(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            })
            .map_err(|_| "ordinary in-memory NewAccount failed; details suppressed")?;
        if !packets
            .iter()
            .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 }))
        {
            return Err("ordinary in-memory NewAccount was rejected");
        }
    }
    let store = config
        .account_store
        .lock()
        .map_err(|_| "in-memory account lock failed")?;
    if store.accounts.len() != manifest.accounts.len() {
        return Err("in-memory account set differs from requested cohort");
    }
    manifest
        .accounts
        .iter()
        .map(|credentials| {
            let record = store
                .accounts
                .get(&credentials.account_id)
                .ok_or("new account record missing")?
                .clone();
            if !record.password.starts_with("$argon2id$")
                || record.password == credentials.password
                || !record.characters.is_empty()
                || !record.saves.is_empty()
                || !record.character_last_access_binary_datetimes.is_empty()
                || record.gm_level != 0
                || record.is_banned
                || !record.ban_reason.is_empty()
                || record.ban_until_ms.is_some()
                || record.banned_at_ms.is_some()
                || !record.storage_password.is_empty()
                || record.has_expanded_storage
                || record.expanded_storage_expiry_time_binary_datetime != 0
                || record.storage_password_last_set_binary_datetime != 0
            {
                return Err(
                    "new account contains unexpected credentials, characters or privileges",
                );
            }
            let raw_json =
                serde_json::to_value(&record).map_err(|_| "account serialization failed")?;
            Ok(PreparedAccount {
                account_id: credentials.account_id.clone(),
                record,
                raw_json,
            })
        })
        .collect()
}

fn require_database_identity(database: &str, user: &str) -> Result<()> {
    if database != REALM || user != REALM {
        return Err("connected database and current user must both be mir2_playtest");
    }
    Ok(())
}

fn insert_account(transaction: &mut Transaction<'_>, prepared: &PreparedAccount) -> Result<()> {
    let account = &prepared.record;
    let affected = transaction
        .execute(
            INSERT_ACCOUNT,
            &[
                &prepared.account_id,
                &account.password,
                &(account.storage_size as i32),
                &account.has_expanded_storage,
                &account.expanded_storage_expiry_time_binary_datetime,
                &account.storage_password,
                &account.storage_password_last_set_binary_datetime,
                &account.is_banned,
                &account.ban_reason,
                &Option::<i64>::None,
                &Option::<i64>::None,
                &prepared.raw_json,
            ],
        )
        .map_err(|_| {
            "account INSERT failed; transaction will be rolled back; details suppressed"
        })?;
    if affected != 1 {
        return Err("account INSERT did not create exactly one row");
    }
    Ok(())
}

fn provision(client: &mut Client, manifest: &Manifest, apply: bool) -> Result<Report> {
    validate_manifest(manifest)?;
    let mut transaction = client
        .build_transaction()
        .isolation_level(IsolationLevel::Serializable)
        .read_only(!apply)
        .start()
        .map_err(|_| "could not start bounded account transaction")?;
    let result = (|| {
        transaction.batch_execute("SET LOCAL statement_timeout = '10s'; SET LOCAL lock_timeout = '3s'; SET LOCAL idle_in_transaction_session_timeout = '30s'")
            .map_err(|_| "could not set transaction time limits")?;
        let identity = transaction
            .query_one("SELECT current_database()::text, current_user::text", &[])
            .map_err(|_| "database identity check failed")?;
        let database = identity.get::<_, String>(0);
        let user = identity.get::<_, String>(1);
        require_database_identity(&database, &user)?;
        let prefix = format!("lt{}%", manifest.batch);
        // Check the entire prefix before generating hashes or issuing any INSERT.
        let existing = transaction
            .query(
                "SELECT account_id FROM public.accounts WHERE account_id LIKE $1",
                &[&prefix],
            )
            .map_err(|_| "account cohort existence check failed; details suppressed")?;
        if !existing.is_empty() {
            return Ok((database, user, true));
        }
        if apply {
            for prepared in prepare_accounts(manifest)? {
                insert_account(&mut transaction, &prepared)?;
            }
        }
        Ok((database, user, false))
    })();
    let (database, user, existing_collision) = match result {
        Ok(result) => result,
        Err(error) => {
            transaction.rollback().map_err(|_| {
                "transaction rollback could not be confirmed; inspect cohort before retry"
            })?;
            return Err(error);
        }
    };
    let committed = apply && !existing_collision;
    if committed {
        transaction
            .commit()
            .map_err(|_| "commit outcome unknown; inspect cohort before retry")?;
    } else {
        transaction
            .rollback()
            .map_err(|_| "read-only preflight rollback failed")?;
    }
    Ok(cohort_report(
        manifest,
        apply,
        database,
        user,
        committed,
        existing_collision,
    ))
}

fn cohort_report(
    manifest: &Manifest,
    apply: bool,
    database: String,
    user: String,
    committed: bool,
    existing_collision: bool,
) -> Report {
    Report {
        provisioning: "admin-provisioned",
        mode: if apply { "apply" } else { "dry-run" },
        requested: manifest.accounts.len(),
        inserted: if committed {
            manifest.accounts.len()
        } else {
            0
        },
        created_count: if committed {
            manifest.accounts.len()
        } else {
            0
        },
        account_ids: manifest
            .accounts
            .iter()
            .map(|credentials| credentials.account_id.clone())
            .collect(),
        database,
        user,
        database_identity_verified: true,
        transaction_committed: committed,
        existing_collision,
        characters_created: 0,
        passed: !existing_collision,
        error: existing_collision.then_some("cohort already exists; no accounts were inserted"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(count: usize) -> Manifest {
        Manifest {
            batch: "b7c3d9a2".into(),
            provisioning: "admin-provisioned".into(),
            accounts: (1..=count)
                .map(|index| Credentials {
                    account_id: format!("ltb7c3d9a2{index:02}"),
                    password: "Offline-test-password-2849".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn url_requires_exact_isolated_loopback_forward_and_never_echoes_input() {
        for host in ["127.0.0.1", "[::1]"] {
            assert!(parse_database_url(&format!(
                "postgresql://mir2_playtest:private%40value@{host}:25432/mir2_playtest"
            ))
            .is_ok());
        }
        for value in [
            "postgres://mir2_playtest:private@127.0.0.1:5432/mir2_playtest",
            "postgres://mir2_playtest:private@127.0.0.1/mir2_playtest",
            "postgres://mir2_playtest:private@127.0.0.1:0/mir2_playtest",
            "postgres://mir2_playtest:private@127.1:25432/mir2_playtest",
            "postgres://mir2_playtest:private@localhost:25432/mir2_playtest",
            "postgres://mir2_playtest:private@192.0.2.3:25432/mir2_playtest",
            "postgres://mir2_playtest:private@127.0.0.1,example.com:25432/mir2_playtest",
            "postgres://mir2:private@127.0.0.1:25432/mir2_playtest",
            "postgres://mir2_playtest:private@127.0.0.1:25432/mir2",
            "postgres://mir2_playtest:private@127.0.0.1:25432/mir2_playtest?options=-csearch_path%3Devil",
            "postgres://mir2_playtest:private@127.0.0.1:25432/mir2_playtest#secret",
            "postgres://mir2_playtest:private@127.0.0.1:25432/mir2_playtest?hostaddr=192.0.2.4",
            "postgres://mir2_playtest:private%0Avalue@127.0.0.1:25432/mir2_playtest",
            "postgres://mir2_playtest@127.0.0.1:25432/mir2_playtest",
            "host=127.0.0.1 port=25432 dbname=mir2_playtest user=mir2_playtest password=private",
        ] {
            let result = parse_database_url(value);
            assert!(result.is_err());
            if let Err(error) = result { assert!(!error.contains("private")); }
        }
    }

    #[test]
    fn mode_defaults_to_dry_run_and_rejects_ambiguous_or_secret_cli_options() {
        let absolute = env::temp_dir()
            .join("private-test-manifest.json")
            .to_string_lossy()
            .into_owned();
        let base = vec!["--manifest".into(), absolute];
        assert!(!parse_options(&base).unwrap().apply);
        let mut apply = base.clone();
        apply.push("--apply".into());
        assert!(parse_options(&apply).unwrap().apply);
        apply.push("--dry-run".into());
        assert!(parse_options(&apply).is_err());
        let mut secret = base;
        secret.extend(["--password".into(), "do-not-log-this".into()]);
        let Err(error) = parse_options(&secret) else {
            panic!("secret CLI input accepted")
        };
        assert!(!error.contains("do-not-log-this"));
    }

    #[test]
    fn cohort_limits_duplicates_namespace_and_ordinary_password_policy_fail_closed() {
        assert!(validate_manifest(&fixture(14)).is_ok());
        assert!(validate_manifest(&fixture(0)).is_err());
        assert!(validate_manifest(&fixture(15)).is_err());
        let mut manifest = fixture(2);
        manifest.accounts[1].account_id = manifest.accounts[0].account_id.clone();
        assert!(validate_manifest(&manifest).is_err());
        for id in [
            "demo",
            "ltotherxxx01",
            "ltb7c3d9a200",
            "ltb7c3d9a215",
            "ltb7c3d9a2x1",
            "ltb7c3d9a21",
        ] {
            let mut manifest = fixture(1);
            manifest.accounts[0].account_id = id.into();
            assert!(validate_manifest(&manifest).is_err());
        }
        for password in [
            "demo",
            "password123",
            "1234567890",
            "ltb7c3d9a201",
            "long\npassword",
        ] {
            let mut manifest = fixture(1);
            manifest.accounts[0].password = password.into();
            assert!(validate_manifest(&manifest).is_err());
        }
        let mut manifest = fixture(1);
        manifest.provisioning = "ordinary-registration".into();
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn manifest_does_not_accept_privileges_unknown_fields_or_unbounded_data() {
        let valid = br#"{"batch":"b7c3d9a2","provisioning":"admin-provisioned","accounts":[{"accountId":"ltb7c3d9a201","password":"Offline-test-password-2849"}]}"#;
        assert!(parse_manifest(valid).is_ok());
        let text = String::from_utf8(valid.to_vec()).unwrap();
        for extra in [
            "\"gmLevel\":1,",
            "\"characters\":[],",
            "\"accountId\":\"demo\",",
        ] {
            assert!(parse_manifest(
                text.replace("\"password\":", &format!("{extra}\"password\":"))
                    .as_bytes()
            )
            .is_err());
        }
        assert!(parse_manifest(&vec![b' '; MAX_MANIFEST_BYTES as usize + 1]).is_err());
    }

    #[test]
    fn connected_database_and_user_both_must_match() {
        assert!(require_database_identity(REALM, REALM).is_ok());
        assert!(require_database_identity("mir2", REALM).is_err());
        assert!(require_database_identity(REALM, "postgres").is_err());
        assert!(require_database_identity("MIR2_PLAYTEST", REALM).is_err());
    }

    #[test]
    fn reports_distinguish_dry_run_collision_and_commit_without_credentials() {
        let manifest = fixture(13);
        for (apply, committed, collision) in [
            (false, false, false),
            (false, false, true),
            (true, false, true),
            (true, true, false),
        ] {
            let report = cohort_report(
                &manifest,
                apply,
                REALM.into(),
                REALM.into(),
                committed,
                collision,
            );
            let text = serde_json::to_string(&report).unwrap();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert!(json["database"] == REALM && json["user"] == REALM);
            assert_eq!(json["createdCount"], if committed { 13 } else { 0 });
            assert_eq!(json["transactionCommitted"], committed);
            assert_eq!(json["existingCollision"], collision);
            assert_eq!(json["passed"], !collision);
            assert_eq!(json["charactersCreated"], 0);
            assert!(!text.contains(&manifest.accounts[0].password));
            assert!(!text.contains("$argon2") && !text.contains("postgres://"));
        }
    }

    #[test]
    fn ordinary_account_creation_uses_random_salts_and_grants_no_characters() {
        let manifest = fixture(2);
        let prepared =
            prepare_accounts(&manifest).expect("ordinary NewAccount should work in memory");
        assert_eq!(prepared.len(), 2);
        assert!(prepared[0].record.password != prepared[1].record.password);
        for account in &prepared {
            assert!(account.record.password.starts_with("$argon2id$"));
            assert!(account.record.characters.is_empty() && account.record.saves.is_empty());
            assert!(account.record.gm_level == 0 && !account.record.is_banned);
            assert!(
                account.raw_json["password"].as_str() == Some(account.record.password.as_str())
            );
            assert!(!serde_json::to_string(&account.raw_json)
                .unwrap()
                .contains(&manifest.accounts[0].password));
        }
        // Verify the real password through ordinary Login, without entering a world.
        let config = SimulationConfig::default();
        {
            let mut store = config.account_store.lock().unwrap();
            store.accounts.clear();
            for account in prepared {
                store.accounts.insert(account.account_id, account.record);
            }
        }
        let mut session = SimulationSession::new(config);
        let replies = session
            .try_handle_packet(ClientPacket::Login {
                account_id: manifest.accounts[0].account_id.clone(),
                password: manifest.accounts[0].password.clone(),
            })
            .unwrap();
        assert!(replies.iter().any(|reply| matches!(reply, ServerPacket::LoginSuccess { characters } if characters.is_empty())));
    }
}
