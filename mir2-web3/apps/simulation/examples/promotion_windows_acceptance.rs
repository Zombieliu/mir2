//! Clone-only account compatibility audit. Only schema migrations write data;
//! account, character and financial rows are never updated or seeded here.
use mir2_simulation::{
    apply_migrations, AccountRecord, CharacterRecord, CharacterSaveRecord, MIGRATIONS,
};
use postgres::{Client, Config, NoTls};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn validate_target(url: &str) -> Result<(), String> {
    let parsed: Config = url.parse().map_err(|_| "windowsAcceptanceTargetInvalid")?;
    let suffix = parsed
        .get_dbname()
        .unwrap_or_default()
        .strip_prefix("mir2_promotion_windows_acceptance_")
        .ok_or("windowsAcceptanceCloneRequired")?;
    if suffix.len() != 12
        || !suffix.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || parsed.get_hostaddrs().iter().any(|ip| !ip.is_loopback())
        || parsed.get_hosts().is_empty()
        || parsed.get_hosts().iter().any(|h| {
            !matches!(h, postgres::config::Host::Tcp(n) if n == "127.0.0.1" || n == "localhost")
        })
    {
        return Err("windowsAcceptanceCloneRequired".into());
    }
    Ok(())
}

fn digest(value: &Value) -> Result<String, String> {
    Ok(
        Sha256::digest(serde_json::to_vec(value).map_err(|_| "windowsAcceptanceEncodeFailed")?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

fn snapshot(db: &mut Client) -> Result<Value, String> {
    let mut tables = serde_json::Map::new();
    for (table, query) in [
        (
            "accounts",
            "SELECT to_jsonb(t) FROM accounts t ORDER BY account_id",
        ),
        (
            "characters",
            "SELECT to_jsonb(t) FROM characters t ORDER BY account_id,character_index",
        ),
        (
            "characterSaves",
            "SELECT to_jsonb(t) FROM character_saves t ORDER BY account_id,character_index",
        ),
    ] {
        let rows = db
            .query(query, &[])
            .map_err(|_| "windowsAcceptanceReadFailed")?;
        tables.insert(
            table.into(),
            Value::Array(rows.iter().map(|r| r.get(0)).collect()),
        );
    }
    Ok(Value::Object(tables))
}

// Serde may omit an explicitly empty/default optional field. Permit only that
// representational change in memory; nonempty persisted keys and scalar values
// must survive recursively. Database JSON itself must remain byte-equivalent.
fn preserved(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => a.iter().all(|(k, v)| match b.get(k) {
            Some(next) => preserved(v, next),
            None => {
                v.is_null()
                    || v.as_object().is_some_and(|v| v.is_empty())
                    || v.as_array().is_some_and(|v| v.is_empty())
            }
        }),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(old, new)| preserved(old, new))
        }
        _ => old == new,
    }
}

fn audit(url: &str) -> Result<Value, String> {
    validate_target(url)?;
    let mut db = Client::connect(url, NoTls).map_err(|_| "windowsAcceptanceDatabaseUnavailable")?;
    let before = snapshot(&mut db)?;
    let mut normalized = 0u64;
    for (table, column) in [
        ("accounts", "raw_json"),
        ("characters", "raw_json"),
        ("character_saves", "snapshot_json"),
    ] {
        // Table and column are fixed internal literals, never command input.
        for row in db
            .query(&format!("SELECT {column} FROM {table}"), &[])
            .map_err(|_| "windowsAcceptanceReadFailed")?
        {
            let old: Value = row.get(0);
            let new = match table {
                "accounts" => {
                    let account: AccountRecord = serde_json::from_value(old.clone())
                        .map_err(|_| "windowsAcceptanceAccountShapeIncompatible")?;
                    if let Some(v) = &account.billing {
                        v.validate()
                            .map_err(|_| "windowsAcceptanceBillingInvalid")?;
                    }
                    if let Some(v) = &account.procurement {
                        v.validate()
                            .map_err(|_| "windowsAcceptanceProcurementInvalid")?;
                    }
                    serde_json::to_value(account)
                }
                "characters" => serde_json::from_value::<CharacterRecord>(old.clone())
                    .and_then(serde_json::to_value),
                _ => serde_json::from_value::<CharacterSaveRecord>(old.clone())
                    .and_then(serde_json::to_value),
            }
            .map_err(|_| "windowsAcceptanceTypedRoundtripFailed")?;
            if !preserved(&old, &new) {
                return Err("windowsAcceptancePersistedFieldWouldBeLost".into());
            }
            normalized += u64::from(old != new);
        }
    }
    for _ in 0..2 {
        apply_migrations(&mut db).map_err(|_| "windowsAcceptanceMigrationFailed")?;
    }
    let after = snapshot(&mut db)?;
    if before != after {
        return Err("windowsAcceptanceOriginalDataChanged".into());
    }
    for (version, _) in MIGRATIONS {
        if db
            .query_opt(
                "SELECT 1 FROM schema_migrations WHERE version=$1",
                &[version],
            )
            .map_err(|_| "windowsAcceptanceReadFailed")?
            .is_none()
        {
            return Err("windowsAcceptanceMigrationMissing".into());
        }
    }
    Ok(
        json!({"cloneOnly":true,"migrationsAppliedTwice":true,"expectedMigrations":MIGRATIONS.len(),
        "accounts":before["accounts"].as_array().unwrap().len(),
        "characters":before["characters"].as_array().unwrap().len(),
        "characterSaves":before["characterSaves"].as_array().unwrap().len(),
        "originalRowsSha256":digest(&before)?,"originalRowsPreserved":true,
        "allPersistedFieldsRetained":true,"typedRowsWithEmptyDefaultNormalization":normalized,
        "accountsSeeded":0,"financialRowsWritten":0,"walletProofHttpReplayRequired":true}),
    )
}

fn run() -> Result<(), String> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--audit-clone"] {
        return Err("usage: promotion_windows_acceptance --audit-clone".into());
    }
    let url = std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
        .map_err(|_| "windowsAcceptanceDatabaseRequired")?;
    let parsed: Config = url.parse().map_err(|_| "invalid database configuration")?;
    if parsed.get_options().is_some() {
        return Err("database options are not accepted by this operator CLI".into());
    }
    println!("{}", audit(&url)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_loopback_clone_name_is_accepted() {
        for url in ["host=127.0.0.1 hostaddr=203.0.113.1 dbname=mir2_promotion_windows_acceptance_123456789abc",
            "postgresql://localhost/mir2_playtest",
            "postgresql://remote/mir2_promotion_windows_acceptance_123456789abc",
            "postgresql://localhost/mir2_promotion_windows_acceptance_123456789ab",
            "postgresql://localhost/mir2_promotion_windows_acceptance_123456789ABC",
        ] {
            assert!(validate_target(url).is_err());
        }
        validate_target("postgresql://127.0.0.1/mir2_promotion_windows_acceptance_123456789abc")
            .unwrap();
    }
    #[test]
    fn normalized_empty_defaults_never_hide_nonempty_financial_fields() {
        assert!(preserved(
            &json!({"empty":{},"none":null}),
            &json!({"newDefault":false})
        ));
        assert!(!preserved(
            &json!({"procurement":{"orders":{"old":1}}}),
            &json!({})
        ));
        assert!(!preserved(
            &json!({"billing":{"suiUsdcOrders":{"old":1}}}),
            &json!({"billing":{}})
        ));
        assert!(!preserved(&json!({"credit":10}), &json!({"credit":9})));
    }

    #[test]
    fn native_pg_clone_audit_reads_real_account_character_and_save_without_mutating_rows() {
        if std::env::var("MIR2_RUN_PROMOTION_PG_TESTS").as_deref() != Ok("1") {
            eprintln!("SKIPPED: native PG opt-in required");
            return;
        }
        let url = std::env::var("MIR2_PROMOTION_WINDOWS_TEST_DATABASE_URL").unwrap();
        validate_target(&url).unwrap();
        let parsed: Config = url.parse().unwrap();
        assert!(parsed.get_options().is_none());
        let schema = format!(
            "windows_audit_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut base = Client::connect(&url, NoTls).unwrap();
        base.batch_execute(&format!("CREATE SCHEMA {schema}"))
            .unwrap();
        let scoped = format!(
            "{url}{}options=-csearch_path%3D{schema}",
            if url.contains('?') { "&" } else { "?" }
        );
        let config = mir2_simulation::SimulationConfig::default()
            .with_postgres_account_store(scoped.clone())
            .unwrap();
        let account = AccountRecord::new(config.default_character.clone());
        config
            .account_store
            .lock()
            .unwrap()
            .accounts
            .insert("audit-fixture".into(), account);
        config.save_account_store_account("audit-fixture").unwrap();
        let first = audit(&scoped).unwrap();
        assert_eq!(first["accounts"], 1);
        assert_eq!(first["characters"], 1);
        assert_eq!(first["characterSaves"], 1);
        assert_eq!(first["financialRowsWritten"], 0);
        assert_eq!(first, audit(&scoped).unwrap());
        drop(config);
        assert!(schema.starts_with("windows_audit_"));
        base.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
            .unwrap();
    }
}
