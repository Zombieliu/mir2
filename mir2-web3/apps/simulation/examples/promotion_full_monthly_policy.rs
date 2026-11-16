//! Explicit, audited amendment of one unlocked Testnet campaign. No signer or payout.
use mir2_simulation::promotion::{PromotionStore, TARGET_CAMPAIGN_ID};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse(args: &[String]) -> Result<(bool, BTreeMap<String, String>), &'static str> {
    let apply = match args.first().map(String::as_str) {
        Some("preview") => false,
        Some("apply") => true,
        _ => return Err("mode must be preview or apply"),
    };
    let mut flags = BTreeMap::new();
    for pair in args[1..].chunks(2) {
        if pair.len() != 2
            || ![
                "--expected-rules-sha256",
                "--actor",
                "--reviewer",
                "--evidence-ref",
            ]
            .contains(&pair[0].as_str())
            || pair[1].trim().is_empty()
            || pair[1].starts_with("--")
            || flags.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err("invalid policy arguments");
        }
    }
    if (apply && flags.len() != 4) || (!apply && !flags.is_empty()) {
        return Err(
            "apply requires the reviewed hash, two actors and evidence; preview accepts no flags",
        );
    }
    Ok((apply, flags))
}
fn run() -> Result<Value, String> {
    let (apply, flags) =
        parse(&std::env::args().skip(1).collect::<Vec<_>>()).map_err(str::to_owned)?;
    if std::env::var("MIR2_PROMOTION_POLICY_ALLOW_TEST").as_deref() != Ok("1") {
        return Err("explicit test policy opt-in required".into());
    }
    let url = std::env::var("MIR2_PROMOTION_POLICY_DATABASE_URL")
        .map_err(|_| "policy database URL missing")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock invalid")?
        .as_millis()
        .try_into()
        .map_err(|_| "clock overflow")?;
    let store = PromotionStore::new(url, "test")?;
    if apply {
        store.adopt_full_monthly_policy(
            TARGET_CAMPAIGN_ID,
            &flags["--actor"],
            &flags["--reviewer"],
            &flags["--expected-rules-sha256"],
            &flags["--evidence-ref"],
            now,
        )
    } else {
        store.preview_full_monthly_policy(TARGET_CAMPAIGN_ID, now)
    }
}
fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("Test-only monthly policy failed: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }
    #[test]
    fn preview_is_read_only_and_has_no_actor_flags() {
        assert_eq!(parse(&args("preview")).unwrap().0, false);
        assert!(parse(&args("preview --actor someone")).is_err());
    }
    #[test]
    fn apply_requires_reviewed_complete_arguments() {
        assert!(parse(&args("apply")).is_err());
        assert!(parse(&args("apply --actor a --reviewer b --expected-rules-sha256 hash --evidence-ref owner-request")).unwrap().0);
    }
    #[test]
    fn rejects_unknown_duplicate_or_partial_flags() {
        for input in [
            "pay",
            "apply --actor",
            "preview --unknown value",
            "apply --actor a --actor b",
        ] {
            assert!(parse(&args(input)).is_err());
        }
    }
}
