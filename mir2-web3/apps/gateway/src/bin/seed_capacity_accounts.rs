//! Offline, account-only provisioning for an isolated File capacity realm.
//! No socket, Login, NewCharacter, StartGame, save editing or gameplay grants.
//! The private input is a capacity pool with a fresh six-character CSPRNG batch.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use mir2_protocol::{ClientPacket, ServerPacket};
use mir2_simulation::{
    validate_commercial_identity_credentials, AccountStore, SimulationConfig, SimulationSession,
};
use serde::Deserialize;

type Result<T> = std::result::Result<T, &'static str>;
const MAX_BYTES: u64 = 64 * 1024;
const SOURCE: &str = "admin-provisioned-file-account-only";

// Do not derive Debug: this is private credential material.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pool {
    schema_version: u8,
    purpose: String,
    realm: String,
    endpoint: String,
    batch: String,
    accounts: Vec<Credentials>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Credentials {
    account_id: String,
    name: String,
    password: String,
    state: String,
    provisioning: String,
}

fn main() {
    std::panic::set_hook(Box::new(|_| {
        eprintln!("capacity account provisioning panic; private details suppressed");
    }));
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{}", serde_json::json!({"passed": false, "error": error}));
            std::process::exit(1);
        }
    }
}

fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--help"] || args == ["-h"] {
        println!("seed_capacity_accounts --manifest ABSOLUTE_PRIVATE_POOL_JSON --account-store ABSOLUTE_FRESH_PRIVATE_JSON\nPool: schemaVersion=1, purpose=isolated-load-actors, realm=isolated, endpoint=literal loopback ws://host:port/ws, batch=six lowercase random letters/digits; 1..100 unique accounts. Account id=lt+batch+two base36 digits, name=L+batch+same suffix, state=registered, provisioning={SOURCE}, private individual password (12..20 chars). Creates only password-hashed accounts; ordinary network Login/NewCharacter/StartGame remain required. Existing output and repository paths are rejected.");
        return Ok(());
    }
    let (manifest, output) = parse_options(&args)?;
    let manifest = private_path(&manifest, true)?;
    let output = private_path(&output, false)?;
    if manifest == output {
        return Err("manifest and account-store paths must differ");
    }
    secure_directory(output.parent().ok_or("account store parent is required")?)?;
    secure_directory(manifest.parent().ok_or("manifest parent is required")?)?;
    secure_directory(&manifest)?;
    let pool = read_pool(&manifest)?;
    let store = prepare_accounts(&pool)?;
    let bytes = serde_json::to_vec_pretty(&store).map_err(|_| "account serialization failed")?;
    write_fresh(&output, &bytes)?;
    println!(
        "{}",
        serde_json::json!({
            "schemaVersion": 1, "passed": true, "provisioning": SOURCE,
            "requested": pool.accounts.len(), "created": store.accounts.len(),
            "charactersCreated": 0, "gameplayGrants": 0, "networkRequests": 0,
            "registrationLoadTested": false, "loginTested": false,
            "existingStoreModified": false, "accountStoreSchema": store.schema_version
        })
    );
    Ok(())
}

fn parse_options(args: &[String]) -> Result<(PathBuf, PathBuf)> {
    let mut manifest = None;
    let mut output = None;
    if args.len() != 4 {
        return Err("require exactly --manifest PATH --account-store PATH; passwords belong in private JSON");
    }
    for pair in args.chunks_exact(2) {
        if pair[1].starts_with('-') || pair[1].is_empty() {
            return Err("absolute JSON path is required");
        }
        match pair[0].as_str() {
            "--manifest" if manifest.is_none() => manifest = Some(PathBuf::from(&pair[1])),
            "--account-store" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
            _ => return Err("unknown or repeated option"),
        }
    }
    Ok((
        manifest.ok_or("manifest is required")?,
        output.ok_or("account store is required")?,
    ))
}

fn private_path(path: &Path, must_exist: bool) -> Result<PathBuf> {
    if !path.is_absolute() || path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err("private files require absolute JSON paths");
    }
    let parent = path.parent().ok_or("private parent is required")?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| "private parent must already exist")?;
    let name = canonical_parent
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("private parent is invalid")?;
    if name != "private" && !name.starts_with("load-private-") {
        return Err("use a dedicated private or load-private-* directory outside the repository");
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .map_err(|_| "repository boundary unavailable")?;
    if canonical_parent.starts_with(repository) {
        return Err("private provisioning files must be outside the repository");
    }
    let resolved = canonical_parent.join(path.file_name().ok_or("private file name is required")?);
    match fs::symlink_metadata(&resolved) {
        Ok(metadata) if !must_exist => {
            let _ = metadata;
            Err("refusing to overwrite an existing account store")
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("private input must be a regular non-symlink file")
        }
        Ok(_) => Ok(resolved),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !must_exist => Ok(resolved),
        Err(_) => Err("private input is missing or unavailable"),
    }
}

fn secure_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if path.is_dir() { 0o700 } else { 0o600 }),
        )
        .map_err(|_| "could not restrict private directory permissions")?;
    }
    #[cfg(windows)]
    {
        use base64::Engine;
        use std::os::windows::process::CommandExt;
        // Build the ACL from the current SID, with no inherited or other explicit
        // rules. The path is an environment value, never PowerShell source.
        let script = "$ErrorActionPreference='Stop';$p=$env:MIR2_CAPACITY_PRIVATE_DIRECTORY;$sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User;if((Get-Item -LiteralPath $p).PSIsContainer){$acl=New-Object System.Security.AccessControl.DirectorySecurity;$rule=New-Object System.Security.AccessControl.FileSystemAccessRule($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')}else{$acl=New-Object System.Security.AccessControl.FileSecurity;$rule=New-Object System.Security.AccessControl.FileSystemAccessRule($sid,'FullControl','Allow')};$acl.SetOwner($sid);$acl.SetAccessRuleProtection($true,$false);$acl.AddAccessRule($rule);Set-Acl -LiteralPath $p -AclObject $acl;$check=Get-Acl -LiteralPath $p;if(-not $check.AreAccessRulesProtected){throw 'unprotected'};foreach($r in $check.Access){if($r.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]).Value -ne $sid.Value -or $r.AccessControlType -ne 'Allow'){throw 'unexpected access'}}";
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            script
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
            .creation_flags(0x08000000)
            .env("MIR2_CAPACITY_PRIVATE_DIRECTORY", path)
            // Cargo/VS shells may carry another PowerShell edition's module
            // path; the native Windows helper must discover its own modules.
            .env_remove("PSModulePath")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(|_| "private Windows ACL helper unavailable")?;
        if !output.status.success() {
            return Err("could not enforce owner-only private Windows ACL");
        }
    }
    Ok(())
}

fn read_pool(path: &Path) -> Result<Pool> {
    let file = fs::File::open(path).map_err(|_| "private manifest open failed")?;
    let metadata = file
        .metadata()
        .map_err(|_| "private manifest metadata unavailable")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("private manifest must be a regular file of at most 64 KiB");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("manifest requires owner-only permissions");
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private manifest read failed")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("private manifest exceeds 64 KiB");
    }
    let pool = serde_json::from_slice::<Pool>(&bytes)
        .map_err(|_| "private manifest JSON/schema invalid")?;
    validate_pool(&pool)?;
    Ok(pool)
}

fn validate_pool(pool: &Pool) -> Result<()> {
    let url = reqwest::Url::parse(&pool.endpoint).map_err(|_| "invalid isolated endpoint")?;
    if pool.schema_version != 1
        || pool.purpose != "isolated-load-actors"
        || pool.realm != "isolated"
        || url.scheme() != "ws"
        || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))
        || url.port().is_none_or(|port| port == 0)
        || url.path() != "/ws"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || pool.batch.len() != 6
        || !pool
            .batch
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || pool.accounts.is_empty()
        || pool.accounts.len() > 100
    {
        return Err("require an isolated loopback ws realm and fresh six-character batch of 1..100 accounts");
    }
    let prefix = format!("lt{}", pool.batch);
    let mut ids = BTreeSet::new();
    for account in &pool.accounts {
        let suffix = account
            .account_id
            .strip_prefix(&prefix)
            .ok_or("account is outside the dedicated cohort")?;
        if suffix.len() != 2
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || account.name != format!("L{}{suffix}", pool.batch)
            || !ids.insert(&account.account_id)
            || account.state != "registered"
            || account.provisioning != SOURCE
            || !(12..=20).contains(&account.password.len())
        {
            return Err("invalid, duplicate or nonempty dedicated capacity account");
        }
        validate_commercial_identity_credentials(&account.account_id, &account.password)
            .map_err(|_| "credentials do not meet ordinary commercial policy")?;
    }
    Ok(())
}

fn prepare_accounts(pool: &Pool) -> Result<AccountStore> {
    validate_pool(pool)?;
    let config = SimulationConfig::default();
    if config.account_store_path.is_some() || config.account_store_database_url.is_some() {
        return Err("in-memory registration unexpectedly has persistence configured");
    }
    config
        .account_store
        .lock()
        .map_err(|_| "account lock failed")?
        .accounts
        .clear();
    let mut session = SimulationSession::new(config.clone());
    for account in &pool.accounts {
        let packets = session
            .try_handle_packet(ClientPacket::NewAccount {
                account_id: account.account_id.clone(),
                password: account.password.clone(),
                birth_date_binary: 0,
                user_name: String::new(),
                secret_question: String::new(),
                secret_answer: String::new(),
                email_address: String::new(),
            })
            .map_err(|_| "ordinary in-memory NewAccount failed; private details suppressed")?;
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
        .map_err(|_| "account lock failed")?;
    if store.accounts.len() != pool.accounts.len() || store.accounts.contains_key("demo") {
        return Err("prepared account set differs from the dedicated cohort");
    }
    for account in &pool.accounts {
        let record = store
            .accounts
            .get(&account.account_id)
            .ok_or("prepared account is missing")?;
        if !record.password.starts_with("$argon2id$")
            || record.password == account.password
            || !record.characters.is_empty()
            || !record.saves.is_empty()
            || !record.character_last_access_binary_datetimes.is_empty()
            || record.gm_level != 0
            || record.activities.is_some()
            || record.billing.is_some()
            || record.monthly_card.is_some()
            || record.is_banned
            || !record.ban_reason.is_empty()
            || record.ban_until_ms.is_some()
            || record.banned_at_ms.is_some()
            || !record.storage_password.is_empty()
            || record.has_expanded_storage
            || record.expanded_storage_expiry_time_binary_datetime != 0
        {
            return Err("prepared account contains unexpected characters, grants or privileges");
        }
    }
    Ok(store.clone())
}

fn write_fresh(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "fresh store create failed; existing output is never replaced")?;
    file.write_all(bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| "new account store write failed; partial file retained for inspection")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pool() -> Pool {
        Pool {
            schema_version: 1,
            purpose: "isolated-load-actors".into(),
            realm: "isolated".into(),
            endpoint: "ws://127.0.0.1:17510/ws".into(),
            batch: "a1b2c3".into(),
            accounts: vec![Credentials {
                account_id: "lta1b2c300".into(),
                name: "La1b2c300".into(),
                password: "OnlyFixture12345".into(),
                state: "registered".into(),
                provisioning: SOURCE.into(),
            }],
        }
    }
    #[test]
    fn only_dedicated_fresh_unprivileged_pool_schema_is_accepted() {
        let mut data = pool();
        assert!(validate_pool(&data).is_ok());
        data.accounts[0].state = "ready".into();
        assert!(validate_pool(&data).is_err());
        data.accounts[0].state = "registered".into();
        data.accounts[0].name = "ExistingPlayer".into();
        assert!(validate_pool(&data).is_err());
        data.accounts[0].name = "La1b2c300".into();
        data.endpoint = "ws://example.com:17510/ws".into();
        assert!(validate_pool(&data).is_err());
        data.endpoint = "ws://127.0.0.1:17510/ws?token=private".into();
        assert!(validate_pool(&data).is_err());
    }
    #[test]
    fn duplicate_and_more_than_100_accounts_are_rejected() {
        let mut value = serde_json::json!({"schemaVersion":1,"purpose":"isolated-load-actors","realm":"isolated","endpoint":"ws://127.0.0.1:17510/ws","batch":"a1b2c3","accounts":[{"accountId":"lta1b2c300","name":"La1b2c300","password":"OnlyFixture12345","state":"registered","provisioning":SOURCE}]});
        let account = value["accounts"][0].clone();
        value["accounts"].as_array_mut().unwrap().push(account);
        assert!(validate_pool(&serde_json::from_value(value).unwrap()).is_err());
        let mut bounded = pool();
        bounded.accounts.clear();
        let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
        for index in 0..=100 {
            let suffix = format!(
                "{}{}",
                digits[index / 36] as char,
                digits[index % 36] as char
            );
            bounded.accounts.push(Credentials {
                account_id: format!("lta1b2c3{suffix}"),
                name: format!("La1b2c3{suffix}"),
                password: "OnlyFixture12345".into(),
                state: "registered".into(),
                provisioning: SOURCE.into(),
            });
            if index == 99 {
                assert!(validate_pool(&bounded).is_ok());
            }
        }
        assert!(validate_pool(&bounded).is_err());
        let mut data = pool();
        data.accounts[0].password = "short".into();
        assert!(validate_pool(&data).is_err());
    }
    #[test]
    fn normal_registration_hashes_password_and_creates_no_character_or_grants() {
        let pool = pool();
        let store = prepare_accounts(&pool).expect("ordinary account-only registration");
        assert_eq!(store.accounts.len(), 1);
        let account = &store.accounts["lta1b2c300"];
        assert!(account.password.starts_with("$argon2id$"));
        assert_ne!(account.password, pool.accounts[0].password);
        assert!(account.characters.is_empty());
        assert!(account.saves.is_empty());
        assert_eq!(account.gm_level, 0);
        assert!(account.billing.is_none());
        assert!(account.monthly_card.is_none());
        assert!(account.activities.is_none());
        let json = serde_json::to_string(&store).unwrap();
        assert!(!json.contains(&pool.accounts[0].password));
    }
    #[test]
    fn fresh_writer_never_overwrites_an_existing_store() {
        let directory = std::env::temp_dir().join(format!(
            "mir2-capacity-fresh-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir(&directory).unwrap();
        let file = directory.join("accounts.json");
        write_fresh(&file, b"first").unwrap();
        assert!(write_fresh(&file, b"replacement").is_err());
        assert_eq!(fs::read(&file).unwrap(), b"first\n");
        fs::remove_file(file).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn command_line_rejects_passwords_and_duplicate_inputs() {
        assert!(parse_options(&["--password".into(), "private".into()]).is_err());
        assert!(parse_options(&[
            "--manifest".into(),
            "one".into(),
            "--manifest".into(),
            "two".into()
        ])
        .is_err());
    }

    #[test]
    fn private_permissions_and_fresh_path_validation_cover_real_files() {
        let root = std::env::temp_dir().join(format!(
            "mir2-capacity-private-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let directory = root.join("private");
        fs::create_dir_all(&directory).unwrap();
        let input = directory.join("pool.json");
        fs::write(&input, b"fixture").unwrap();
        let resolved = private_path(&input, true).expect("private existing input path");
        secure_directory(resolved.parent().unwrap())
            .expect("real owner-only directory permissions");
        secure_directory(&resolved).expect("real owner-only input permissions");
        assert!(private_path(&input, false).is_err());
        assert!(private_path(&directory.join("fresh.json"), false).is_ok());
        assert!(private_path(Path::new("relative.json"), false).is_err());
        fs::remove_file(&input).unwrap();
        fs::remove_dir(&directory).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
