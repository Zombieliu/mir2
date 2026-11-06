//! Offline account pre-seeder for the WebSocket load harness.
//!
//! This binary deliberately does not open a socket or start a Gateway. It
//! drives the same public simulation packet boundary used by the Gateway and
//! persists the resulting account store to an explicitly supplied file.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use mir2_protocol::{ClientPacket, MirClass, MirGender, SelectInfo, ServerPacket};
use mir2_simulation::{
    AccountStore, CharacterRecord, CharacterSaveRecord, SimulationConfig, SimulationSession,
};

const DEFAULT_COUNT: usize = 64;
const MAX_COUNT: usize = 256;
const ACCOUNT_STORE_PATH_ENV: &str = "MIR2_WS_LOAD_SEED_ACCOUNT_STORE_PATH";
const PREFIX_ENV: &str = "MIR2_WS_LOAD_SEED_PREFIX";
const COUNT_ENV: &str = "MIR2_WS_LOAD_SEED_COUNT";
const PASSWORD_ENV: &str = "MIR2_WS_LOAD_PASSWORD";
const ALLOW_EXISTING_ENV: &str = "MIR2_WS_LOAD_SEED_ALLOW_EXISTING";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    account_store_path: PathBuf,
    prefix: String,
    count: usize,
    password: String,
    allow_existing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountOutcome {
    Created,
    Existing,
    Failed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Counts {
    created: usize,
    existing: usize,
    failed: usize,
}

impl Counts {
    fn record(&mut self, outcome: AccountOutcome) {
        match outcome {
            AccountOutcome::Created => self.created += 1,
            AccountOutcome::Existing => self.existing += 1,
            AccountOutcome::Failed => self.failed += 1,
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }

    let options = parse_options(&args)?;
    validate_options(&options)?;
    validate_store_path(&options.account_store_path, options.allow_existing)?;

    let existing_account_ids = if options.account_store_path.is_file() {
        let contents = fs::read_to_string(&options.account_store_path)
            .map_err(|error| format!("failed to read account store: {error}"))?;
        let store = serde_json::from_str::<AccountStore>(&contents)
            .map_err(|error| format!("existing account store is not valid JSON: {error}"))?;
        store.accounts.keys().cloned().collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };

    let config =
        SimulationConfig::default().with_account_store_path(options.account_store_path.clone());
    // SimulationConfig::default() contains the demo fixture. A fresh seed
    // file must contain only the requested load accounts; preserve demo when
    // merging a store that already had it.
    if !existing_account_ids.contains("demo") {
        config
            .account_store
            .lock()
            .map_err(|_| "account store mutex poisoned".to_string())?
            .accounts
            .remove("demo");
    }

    let mut counts = Counts::default();
    for index in 0..options.count {
        let account_id = account_id(&options.prefix, index);
        let outcome = match seed_account(&config, &account_id, &options.password) {
            Ok(outcome) => outcome,
            Err(_) => AccountOutcome::Failed,
        };
        counts.record(outcome);
    }

    config.save_account_store()?;
    println!(
        "offline WebSocket load account seed complete: {}",
        summary_line(counts, options.count, &options.password)
    );

    if counts.failed > 0 {
        return Err(format!("{} account(s) failed to seed", counts.failed));
    }
    Ok(())
}

fn usage() -> &'static str {
    "Usage: seed_ws_load_accounts --account-store-path PATH --prefix PREFIX [--count N] [--allow-existing]\n\nEnvironment equivalents:\n  MIR2_WS_LOAD_SEED_ACCOUNT_STORE_PATH\n  MIR2_WS_LOAD_SEED_PREFIX\n  MIR2_WS_LOAD_SEED_COUNT (default 64, maximum 256)\n  MIR2_WS_LOAD_PASSWORD\n  MIR2_WS_LOAD_SEED_ALLOW_EXISTING (true/1/yes)"
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut path = env::var(ACCOUNT_STORE_PATH_ENV).ok();
    let mut prefix = env::var(PREFIX_ENV).ok();
    let mut count = env::var(COUNT_ENV)
        .ok()
        .map(|value| parse_count(&value))
        .transpose()?;
    let password = env::var(PASSWORD_ENV).ok();
    let mut allow_existing = env_flag(ALLOW_EXISTING_ENV)?;

    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "--allow-existing" => allow_existing = true,
            "--account-store-path" | "--path" => {
                path = Some(next_value(args, &mut index, argument)?);
            }
            "--prefix" => prefix = Some(next_value(args, &mut index, argument)?),
            "--count" => count = Some(parse_count(&next_value(args, &mut index, argument)?)?),
            "--password" => {
                return Err(format!(
                    "--password is not accepted; provide the password via {PASSWORD_ENV}"
                ));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }

    Ok(Options {
        account_store_path: path
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                format!(
                    "account store path is required (--account-store-path or {ACCOUNT_STORE_PATH_ENV})"
                )
            })?,
        prefix: prefix
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("prefix is required (--prefix or {PREFIX_ENV})"))?,
        count: count.unwrap_or(DEFAULT_COUNT),
        password: password
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("password is required via {PASSWORD_ENV}"))?,
        allow_existing,
    })
}

fn next_value(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_count(value: &str) -> Result<usize, String> {
    let count = value
        .parse::<usize>()
        .map_err(|_| "count must be a positive integer".to_string())?;
    if count == 0 || count > MAX_COUNT {
        return Err(format!("count must be between 1 and {MAX_COUNT}"));
    }
    Ok(count)
}

fn env_flag(name: &str) -> Result<bool, String> {
    match env::var(name) {
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            ) =>
        {
            Ok(true)
        }
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no"
            ) =>
        {
            Ok(false)
        }
        Ok(_) => Err(format!("{name} must be true/false, yes/no, or 1/0")),
        Err(env::VarError::NotPresent) => Ok(false),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid UTF-8")),
    }
}

fn validate_options(options: &Options) -> Result<(), String> {
    if options.count == 0 || options.count > MAX_COUNT {
        return Err(format!("count must be between 1 and {MAX_COUNT}"));
    }
    if options.prefix.is_empty() || options.prefix.len() > 29 {
        return Err(
            "prefix must be 1..=29 bytes so generated IDs fit the 32-byte limit".to_string(),
        );
    }
    if !options
        .prefix
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err("prefix may contain only ASCII letters, digits, '_', '-', or '.'".to_string());
    }
    if options.password.is_empty() || options.password.chars().any(char::is_control) {
        return Err("password must be non-empty and contain no control characters".to_string());
    }
    Ok(())
}

fn validate_store_path(path: &Path, allow_existing: bool) -> Result<(), String> {
    if path.exists() {
        if !allow_existing {
            return Err(format!(
                "refusing to modify existing account store {}; pass --allow-existing to merge",
                path.display()
            ));
        }
        if !path.is_file() {
            return Err(format!(
                "account store path is not a regular file: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn account_id(prefix: &str, index: usize) -> String {
    format!("{prefix}{index:03}")
}

fn character_name_from_account_id(account_id: &str) -> String {
    let suffix = account_id
        .rsplit_once('-')
        .map(|(_, suffix)| suffix)
        .unwrap_or(account_id);
    format!("Load{suffix}")
}

fn seed_account(
    config: &SimulationConfig,
    account_id: &str,
    password: &str,
) -> Result<AccountOutcome, String> {
    let already_exists = config
        .account_store
        .lock()
        .map_err(|_| "account store mutex poisoned".to_string())?
        .accounts
        .contains_key(account_id);
    let mut session = SimulationSession::new(config.clone());

    if !already_exists {
        expect_new_account_success(session.handle_packet(ClientPacket::NewAccount {
            account_id: account_id.to_string(),
            password: password.to_string(),
            birth_date_binary: 0,
            user_name: account_id.to_string(),
            secret_question: String::new(),
            secret_answer: String::new(),
            email_address: String::new(),
        }))?;
    }

    let characters = expect_login_success(session.handle_packet(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: password.to_string(),
    }))?;
    if characters.iter().any(|character| character.index == 0) {
        expect_start_game_success(
            session.handle_packet(ClientPacket::StartGame { character_index: 0 }),
        )?;
        return Ok(if already_exists {
            AccountOutcome::Existing
        } else {
            AccountOutcome::Failed
        });
    }

    let character_name = character_name_from_account_id(account_id);
    let created =
        expect_new_character_success(session.handle_packet(ClientPacket::NewCharacter {
            name: character_name.clone(),
            gender: MirGender::Male,
            class: MirClass::Warrior,
        }))?;
    normalize_character_to_zero(config, account_id, &created, &character_name)?;

    let mut verification = SimulationSession::new(config.clone());
    expect_login_success(verification.handle_packet(ClientPacket::Login {
        account_id: account_id.to_string(),
        password: password.to_string(),
    }))?;
    expect_start_game_success(
        verification.handle_packet(ClientPacket::StartGame { character_index: 0 }),
    )?;
    Ok(if already_exists {
        AccountOutcome::Existing
    } else {
        AccountOutcome::Created
    })
}

fn expect_new_account_success(packets: Vec<ServerPacket>) -> Result<(), String> {
    if packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::NewAccount { result: 8 }))
    {
        Ok(())
    } else {
        Err("NewAccount was rejected".to_string())
    }
}

fn expect_login_success(packets: Vec<ServerPacket>) -> Result<Vec<SelectInfo>, String> {
    packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::LoginSuccess { characters } => Some(Ok(characters)),
            ServerPacket::Login { result } => {
                Some(Err(format!("Login was rejected (result {result})")))
            }
            _ => None,
        })
        .unwrap_or_else(|| Err("Login response was missing".to_string()))
}

fn expect_new_character_success(packets: Vec<ServerPacket>) -> Result<SelectInfo, String> {
    packets
        .into_iter()
        .find_map(|packet| match packet {
            ServerPacket::NewCharacterSuccess { char_info } => Some(Ok(char_info)),
            ServerPacket::NewCharacter { result } => {
                Some(Err(format!("NewCharacter was rejected (result {result})")))
            }
            _ => None,
        })
        .unwrap_or_else(|| Err("NewCharacter response was missing".to_string()))
}

fn expect_start_game_success(packets: Vec<ServerPacket>) -> Result<(), String> {
    if packets
        .iter()
        .any(|packet| matches!(packet, ServerPacket::StartGame { result: 4, .. }))
    {
        Ok(())
    } else {
        Err("StartGame for character index 0 was rejected".to_string())
    }
}

fn normalize_character_to_zero(
    config: &SimulationConfig,
    account_id: &str,
    created: &SelectInfo,
    name: &str,
) -> Result<(), String> {
    let character = CharacterRecord {
        index: 0,
        name: name.to_string(),
        level: created.level,
        class: created.class,
        gender: created.gender,
    };
    let mut store = config
        .account_store
        .lock()
        .map_err(|_| "account store mutex poisoned".to_string())?;
    let account = store
        .accounts
        .get_mut(account_id)
        .ok_or_else(|| "account disappeared during character creation".to_string())?;
    if account.characters.iter().any(|entry| entry.index == 0) {
        return Err("account already has character index 0".to_string());
    }
    account
        .characters
        .retain(|entry| entry.index != created.index);
    account.characters.push(character.clone());
    account.saves.remove(&created.index);
    account.saves.insert(0, CharacterSaveRecord::new(character));
    drop(store);
    config.save_account_store_account(account_id)
}

fn summary_line(counts: Counts, total: usize, password: &str) -> String {
    let summary = format!(
        "created={} existing={} failed={} total={}",
        counts.created, counts.existing, counts.failed, total
    );
    debug_assert!(!summary.contains(password));
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_boundary_accepts_one_and_maximum() {
        assert_eq!(parse_count("1"), Ok(1));
        assert_eq!(parse_count("256"), Ok(256));
        assert!(parse_count("0").is_err());
        assert!(parse_count("257").is_err());
    }

    #[test]
    fn generated_account_names_are_deterministic_and_bounded() {
        assert_eq!(account_id("load-", 7), "load-007");
        assert_eq!(account_id("load-", 7), account_id("load-", 7));
        assert_ne!(account_id("load-", 7), account_id("load-", 8));
    }

    #[test]
    fn summary_does_not_echo_password() {
        let password = "SensitiveSeedPassword!42";
        let output = summary_line(
            Counts {
                created: 2,
                existing: 1,
                failed: 0,
            },
            3,
            password,
        );
        assert!(!output.contains(password));
        assert_eq!(output, "created=2 existing=1 failed=0 total=3");
    }

    #[test]
    fn password_argument_is_rejected_without_echoing_the_secret() {
        let secret = "SensitiveSeedPassword!42";
        let error = parse_options(&[
            "--account-store-path".to_string(),
            "accounts.json".to_string(),
            "--prefix".to_string(),
            "load-".to_string(),
            "--password".to_string(),
            secret.to_string(),
        ])
        .expect_err("password must not be accepted on the command line");

        assert!(error.contains("MIR2_WS_LOAD_PASSWORD"));
        assert!(!error.contains(secret));
    }

    #[test]
    fn existing_index_zero_is_verified_through_start_game() {
        let config = SimulationConfig::default();

        let outcome = seed_account(&config, "demo", "demo")
            .expect("the default demo character should pass Login and StartGame");

        assert_eq!(outcome, AccountOutcome::Existing);
    }

    #[test]
    fn start_game_rejection_is_not_treated_as_success() {
        let error = expect_start_game_success(vec![ServerPacket::StartGame {
            result: 2,
            resolution: 0,
        }])
        .expect_err("a rejected StartGame response must fail seeding");

        assert_eq!(error, "StartGame for character index 0 was rejected");
    }
}
