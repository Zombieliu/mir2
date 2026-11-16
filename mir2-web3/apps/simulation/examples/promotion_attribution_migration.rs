//! Explicit operator preview/staging only. The member must confirm current
//! terms through the authenticated UI; this CLI never confirms attribution.
use mir2_simulation::promotion::{PromotionMigrationScope, PromotionStore};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE_ENV: &str = "MIR2_PROMOTION_MIGRATION_SOURCE_DATABASE_URL";
const TARGET_ENV: &str = "MIR2_PROMOTION_MIGRATION_TARGET_DATABASE_URL";
const MAX_SCOPE_BYTES: u64 = 1024;

#[derive(Debug, PartialEq)]
enum Mode {
    Preview,
    Stage,
}

#[derive(Debug)]
struct Stage {
    expected_snapshot_sha256: String,
    actor: String,
    reviewer: String,
    evidence_ref: String,
    idempotency_key: String,
}

#[derive(Debug)]
struct Options {
    mode: Mode,
    scope_file: PathBuf,
    output: PathBuf,
    stage: Option<Stage>,
}

fn parse(args: &[String]) -> Result<Options, &'static str> {
    let mut flags = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !matches!(
                pair[0].as_str(),
                "--mode"
                    | "--scope-file"
                    | "--output"
                    | "--expected-snapshot-sha256"
                    | "--actor"
                    | "--reviewer"
                    | "--evidence-ref"
                    | "--idempotency-key"
            )
            || pair[1].is_empty()
            || pair[1].starts_with("--")
            || flags.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("promotionMigrationArgumentsInvalid");
        }
    }
    let mode = match flags.remove("--mode") {
        Some("preview") => Mode::Preview,
        Some("stage") => Mode::Stage,
        _ => return Err("promotionMigrationModeRequired"),
    };
    let scope_file = flags
        .remove("--scope-file")
        .ok_or("promotionMigrationScopeFileRequired")?
        .into();
    let output = flags
        .remove("--output")
        .ok_or("promotionMigrationOutputRequired")?
        .into();
    let stage = if mode == Mode::Stage {
        let expected = flags
            .remove("--expected-snapshot-sha256")
            .ok_or("promotionMigrationSnapshotRequired")?;
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("promotionMigrationSnapshotInvalid");
        }
        let actor = flags
            .remove("--actor")
            .ok_or("promotionMigrationActorsRequired")?;
        let reviewer = flags
            .remove("--reviewer")
            .ok_or("promotionMigrationActorsRequired")?;
        if actor == reviewer || !valid_id(actor) || !valid_id(reviewer) {
            return Err("promotionMigrationActorsInvalid");
        }
        let evidence_ref = flags
            .remove("--evidence-ref")
            .ok_or("promotionMigrationEvidenceRequired")?;
        let idempotency_key = flags
            .remove("--idempotency-key")
            .ok_or("promotionMigrationIdempotencyRequired")?;
        if !valid_reference(evidence_ref) || !valid_id(idempotency_key) {
            return Err("promotionMigrationEvidenceInvalid");
        }
        Some(Stage {
            expected_snapshot_sha256: expected.into(),
            actor: actor.into(),
            reviewer: reviewer.into(),
            evidence_ref: evidence_ref.into(),
            idempotency_key: idempotency_key.into(),
        })
    } else {
        None
    };
    if !flags.is_empty() {
        return Err("promotionMigrationArgumentsInvalid");
    }
    Ok(Options {
        mode,
        scope_file,
        output,
        stage,
    })
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:@".contains(&c))
}

fn valid_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:@/".contains(&c))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeInput {
    member_wallet: String,
    leader_wallet: String,
}

fn parse_scope(bytes: &[u8]) -> Result<PromotionMigrationScope, &'static str> {
    if bytes.len() as u64 > MAX_SCOPE_BYTES {
        return Err("promotionMigrationScopeInvalid");
    }
    let input: ScopeInput =
        serde_json::from_slice(bytes).map_err(|_| "promotionMigrationScopeInvalid")?;
    let valid_wallet = |s: &str| {
        s.len() == 66
            && s.starts_with("0x")
            && s.as_bytes()[2..]
                .iter()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
    };
    if !valid_wallet(&input.member_wallet)
        || !valid_wallet(&input.leader_wallet)
        || input.member_wallet == input.leader_wallet
    {
        return Err("promotionMigrationScopeInvalid");
    }
    Ok(PromotionMigrationScope {
        member_wallet: input.member_wallet,
        leader_wallet: input.leader_wallet,
    })
}

fn read_scope(path: &Path) -> Result<PromotionMigrationScope, &'static str> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| "promotionMigrationScopeFileUnavailable")?
        .take(MAX_SCOPE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "promotionMigrationScopeFileUnavailable")?;
    parse_scope(&bytes)
}

fn reserve_output(path: &Path) -> Result<File, &'static str> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| "promotionMigrationOutputUnavailable")
}

fn execute(
    options: &Options,
    source: &str,
    target: &str,
    scope: &PromotionMigrationScope,
    now: u64,
) -> Result<Value, &'static str> {
    let store = PromotionStore::new(target.to_owned(), "test")
        .map_err(|_| "promotionMigrationConfigurationInvalid")?;
    match &options.stage {
        None => store
            .preview_legacy_invitation_migration(source, scope, now)
            .map_err(|_| "promotionMigrationPreviewFailed"),
        Some(stage) => store
            .stage_legacy_invitation_migration(
                source,
                scope,
                &stage.expected_snapshot_sha256,
                &stage.actor,
                &stage.reviewer,
                &stage.evidence_ref,
                &stage.idempotency_key,
                now,
            )
            .map_err(|_| "promotionMigrationStageFailed"),
    }
}

fn run() -> Result<(), &'static str> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--help"] {
        println!("promotion_attribution_migration --mode preview|stage --scope-file PATH --output NEW_PATH");
        println!("Stage also requires --expected-snapshot-sha256 HASH --actor OPERATIONS --reviewer FINANCE --evidence-ref REF --idempotency-key KEY. Scope JSON contains only member_wallet and leader_wallet. Database URLs come only from the two MIR2_PROMOTION_MIGRATION_*_DATABASE_URL environment variables. The member must confirm current terms in the authenticated UI.");
        return Ok(());
    }
    let options = parse(&args)?;
    let scope = read_scope(&options.scope_file)?;
    let source =
        std::env::var(SOURCE_ENV).map_err(|_| "promotionMigrationConfigurationRequired")?;
    let target =
        std::env::var(TARGET_ENV).map_err(|_| "promotionMigrationConfigurationRequired")?;
    if source.is_empty() || target.is_empty() {
        return Err("promotionMigrationConfigurationRequired");
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "promotionMigrationClockInvalid")?
        .as_millis()
        .try_into()
        .map_err(|_| "promotionMigrationClockInvalid")?;
    // Reserve before the only possible database mutation. Existing receipts are
    // never replaced; a failed write must be investigated with the same key.
    let mut output = reserve_output(&options.output)?;
    let result = execute(&options, &source, &target, &scope, now)?;
    let mut bytes =
        serde_json::to_vec_pretty(&result).map_err(|_| "promotionMigrationOutputEncodingFailed")?;
    bytes.push(b'\n');
    output
        .write_all(&bytes)
        .and_then(|_| output.sync_all())
        .map_err(|_| "promotionMigrationOutputWriteFailed")?;
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|c| format!("{c:02x}"))
        .collect();
    println!(
        "{}",
        json!({"ok":true,"mode":if options.mode==Mode::Preview {"preview"} else {"stage"},
        "status":if options.mode==Mode::Preview {"preview_complete"} else {"pending_member_confirmation"},"manifestSha256":hash})
    );
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
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }
    fn preview() -> Vec<String> {
        args(&[
            "--mode",
            "preview",
            "--scope-file",
            "scope.json",
            "--output",
            "new.json",
        ])
    }
    fn stage() -> Vec<String> {
        let mut a = args(&[
            "--mode",
            "stage",
            "--scope-file",
            "scope.json",
            "--output",
            "new.json",
            "--actor",
            "operations_actor",
            "--reviewer",
            "finance_actor",
            "--evidence-ref",
            "review:20261031",
            "--idempotency-key",
            "migration:pair:v1",
            "--expected-snapshot-sha256",
        ]);
        a.push("a".repeat(64));
        a
    }
    fn wallets() -> Value {
        json!({"member_wallet":format!("0x{}","a".repeat(64)),"leader_wallet":format!("0x{}","b".repeat(64))})
    }
    #[test]
    fn explicit_modes_and_required_stage_approvals() {
        assert_eq!(parse(&preview()).unwrap().mode, Mode::Preview);
        assert!(parse(&stage()).unwrap().stage.is_some());
        for flag in [
            "--expected-snapshot-sha256",
            "--actor",
            "--reviewer",
            "--evidence-ref",
            "--idempotency-key",
        ] {
            let mut a = stage();
            let at = a.iter().position(|v| v == flag).unwrap();
            a.drain(at..at + 2);
            assert!(parse(&a).is_err());
        }
        assert!(parse(&[]).is_err());
    }
    #[test]
    fn unknown_duplicate_clock_database_and_preview_stage_fields_are_rejected() {
        for flag in [
            "--mode",
            "--output",
            "--scope-file",
            "--now",
            "--source-database-url",
            "--proof",
            "--actor",
        ] {
            let mut a = preview();
            a.extend(args(&[flag, "injected"]));
            assert!(parse(&a).is_err());
        }
        let mut a = preview();
        a.push("--actor".into());
        assert!(parse(&a).is_err());
    }
    #[test]
    fn distinct_actors_and_exact_lowercase_snapshot_hash_are_required() {
        for (flag, value) in [
            ("--reviewer", "operations_actor"),
            ("--expected-snapshot-sha256", "ABC"),
            ("--evidence-ref", "https://host/?secret=x"),
            ("--actor", "x\ny"),
        ] {
            let mut a = stage();
            let i = a.iter().position(|v| v == flag).unwrap();
            a[i + 1] = value.into();
            assert!(parse(&a).is_err());
        }
    }
    #[test]
    fn scope_accepts_only_two_distinct_canonical_public_wallets() {
        let valid = wallets();
        assert!(parse_scope(&serde_json::to_vec(&valid).unwrap()).is_ok());
        for key in [
            "verifiedProof",
            "sourceAccountId",
            "confirmedAtMs",
            "databaseUrl",
        ] {
            let mut wrong = valid.clone();
            wrong[key] = json!("injected");
            assert!(parse_scope(&serde_json::to_vec(&wrong).unwrap()).is_err());
        }
        let mut same = valid.clone();
        same["leader_wallet"] = same["member_wallet"].clone();
        assert!(parse_scope(&serde_json::to_vec(&same).unwrap()).is_err());
        assert!(parse_scope(&vec![b' '; 1025]).is_err());
        assert!(parse_scope(br#"{"member_wallet":"0xa","leader_wallet":"0xb"}"#).is_err());
    }
    #[test]
    fn output_is_exclusive_and_does_not_overwrite_evidence() {
        let path = std::env::temp_dir().join(format!(
            "promotion-migration-cli-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = reserve_output(&path).unwrap();
        file.write_all(b"retained").unwrap();
        drop(file);
        assert!(reserve_output(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"retained");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}
