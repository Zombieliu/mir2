//! Offline, development-only import into the dedicated local PvP database.
//! Stop every Gateway/other database writer before running this example. The
//! repository provides per-account CAS, not an exclusive empty-database lease.
use mir2_simulation::{
    AccountStore, AccountStoreDatabaseMode, AccountStoreRepository, PostgresAccountStoreRepository,
};
use postgres::{config::Host, Config};
use serde_json::{json, Value};
use std::{env, fs, net::IpAddr, path::Path, process::ExitCode};

type Result<T> = std::result::Result<T, &'static str>;

fn validate_destination(url: &str) -> Result<()> {
    let config: Config = url.parse().map_err(|_| "invalid database configuration")?;
    let host = match config.get_hosts() {
        [Host::Tcp(host)] => host,
        _ => return Err("exactly one literal loopback IP is required"),
    };
    if !host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || config.get_ports() != [55432]
        || config.get_dbname() != Some("mir2_pvp_capacity_v1")
        || !config.get_hostaddrs().is_empty()
        || config.get_options().is_some()
    {
        return Err("destination must be loopback:55432/mir2_pvp_capacity_v1 without overrides");
    }
    Ok(())
}

// Ignore only fresh-schema bookkeeping; reject any retained business state,
// including sealed stones and a previously advanced/owned guild clock.
fn empty_business_image(store: &AccountStore) -> Result<Value> {
    if !store.accounts.is_empty() || !(0..=1).contains(&store.next_character_index) {
        return Err("destination store is not empty");
    }
    let mut value = serde_json::to_value(store).map_err(|_| "store validation failed")?;
    let object = value.as_object_mut().ok_or("store validation failed")?;
    if let Some(clock) = object.get("guildClock") {
        if !clock.is_null()
            && clock
                != &json!({"generation":0,"ownerToken":null,"leaseExpiresMs":0,"minuteAnchorMs":0})
        {
            return Err("destination clock is not fresh");
        }
    }
    object.remove("guildClock");
    object.remove("schemaVersion");
    object.remove("nextCharacterIndex");
    Ok(value)
}

fn run() -> Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 3 || args[0] != "--development" || args[1] != "--input" {
        return Err(
            "usage: capacity_pvp_fixture_import --development --input PRIVATE_FIXTURE_JSON",
        );
    }
    if env::var("MIR2_RUNTIME_ENV").as_deref() != Ok("development")
        || ["MIR2_ENV", "MIR2_DEPLOYMENT_ENV"]
            .iter()
            .any(|name| env::var_os(name).is_some_and(|value| value != "development"))
    {
        return Err("explicit development environment is required");
    }
    let url = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
        .map_err(|_| "database configuration is required")?;
    validate_destination(&url)?;
    let input_path = Path::new(&args[2]);
    let metadata =
        fs::symlink_metadata(input_path).map_err(|_| "cannot inspect private fixture")?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 256 * 1024 * 1024
    {
        return Err("private fixture must be a regular bounded file");
    }
    let bytes = fs::read(input_path).map_err(|_| "cannot read private fixture")?;
    let source: AccountStore =
        serde_json::from_slice(&bytes).map_err(|_| "invalid private fixture JSON")?;
    let count = source.accounts.len();
    if !(2..=500).contains(&count)
        || source.accounts.values().any(|account| {
            account.characters.len() != 1
                || account.saves.len() != 1
                || account
                    .characters
                    .first()
                    .is_none_or(|character| !account.saves.contains_key(&character.index))
        })
    {
        return Err("fixture requires 2..500 accounts with one saved character each");
    }
    let default_character = source
        .accounts
        .values()
        .next()
        .and_then(|account| account.characters.first())
        .ok_or("fixture has no character")?
        .clone();
    let mut baseline = AccountStore::new(default_character.clone());
    baseline.accounts.clear();
    if source.schema_version != baseline.schema_version {
        return Err("fixture schema must match the current repository");
    }
    let repository =
        PostgresAccountStoreRepository::new(url, AccountStoreDatabaseMode::SourceOfTruth);
    // load() performs the repository's normal migrations and does not insert a
    // demo account. Any load failure is fatal, never treated as an empty store.
    let existing = repository
        .load(default_character.clone())
        .map_err(|_| "destination load failed")?;
    if empty_business_image(&existing)? != empty_business_image(&baseline)? {
        return Err("destination store contains business state");
    }
    repository
        .save(&source)
        .map_err(|_| "fixture import failed")?;
    let confirmed = repository
        .load(default_character)
        .map_err(|_| "import readback failed")?;
    if serde_json::to_value(&confirmed.accounts).map_err(|_| "readback validation failed")?
        != serde_json::to_value(&source.accounts).map_err(|_| "fixture validation failed")?
    {
        return Err("import readback does not match; do not retry over this database");
    }
    println!("imported_accounts={count} imported_characters={count} success=true");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Never print repository errors, DSNs, account IDs or fixture data.
            eprintln!("capacity PvP fixture import: {message}");
            ExitCode::FAILURE
        }
    }
}
