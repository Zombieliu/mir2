//! Explicit operator recovery, never a game client command. Reads one private
//! JSON request on stdin; stdout contains public order status only, no tokens.
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::PathBuf,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    account_id: String,
    order_id: String,
    credential_id: String,
    payer: String,
    digest: String,
    #[serde(default)]
    deliver_inactive: bool,
}

fn run() -> Result<serde_json::Value, String> {
    if std::env::var("MIR2_SUI_USDC_NATIVE_RECOVERY")
        .ok()
        .as_deref()
        != Some("1")
    {
        return Err("usdcRecoveryNotEnabled".into());
    }
    let mut bytes = Vec::new();
    io::stdin()
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| "usdcRequestInvalid")?;
    if bytes.len() > 8192 {
        return Err("usdcRequestInvalid".into());
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(|_| "usdcRequestInvalid")?;
    let path = std::env::var_os("MIR2_ACCOUNT_STORE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".mir2-data/accounts.json"));
    let config =
        mir2_simulation::SimulationConfig::default().with_account_store_environment(path)?;
    let order = mir2_gateway::sui_usdc_billing::reconcile_native_recharge_from_env(
        &config,
        &request.account_id,
        &request.order_id,
        &request.credential_id,
        &request.payer,
        &request.digest,
    )?;
    if request.deliver_inactive {
        mir2_gateway::sui_usdc_billing::deliver_inactive_native_recharge_from_env(
            &config,
            &request.account_id,
            &request.order_id,
        )
    } else {
        Ok(serde_json::json!({"order":order,"delivery":{"status":"source_pending"}}))
    }
}
fn main() {
    match run() {
        Ok(result) => println!("{}", serde_json::json!({"ok":true,"result":result})),
        Err(error) => {
            let safe = if error.starts_with("usdc") || error.starts_with("billing") {
                error
            } else {
                "usdcRecoveryFailed".into()
            };
            eprintln!("{}", serde_json::json!({"ok":false,"error":safe}));
            std::process::exit(1);
        }
    }
}
