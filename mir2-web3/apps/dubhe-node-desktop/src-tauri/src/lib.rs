use std::env;
use std::time::Duration;

use mir2_gateway::{HomeAgentKeyring, HomeAgentManagementKeyring, HomeAgentWorkMode};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

const DEFAULT_KEYRING_ACCOUNT: &str = "default";
const DEFAULT_SUPERVISOR_URL: &str = "http://127.0.0.1:17990";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopIdentity {
    node_id: String,
    public_key: String,
    created: bool,
    key_store: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopBootstrap {
    identity: DesktopIdentity,
    management_token_created: bool,
    supervisor_reachable: bool,
    status: Option<SupervisorStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupervisorStatus {
    version: String,
    mode: HomeAgentWorkMode,
    accept_new_sessions: bool,
    reason: String,
    cpu_usage_percent: f32,
    available_memory_bytes: u64,
    active_sessions: usize,
    zone_reachable: bool,
    last_observed_at_ms: u64,
    node_id: String,
    public_key: String,
    key_store: String,
    managed_processes: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeActionReceipt {
    accepted: bool,
    serving: bool,
    status: SupervisorStatus,
}

fn keyring_account() -> String {
    env::var("MIR2_HOME_AGENT_KEYRING_ACCOUNT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_KEYRING_ACCOUNT.to_string())
}

fn supervisor_url() -> Result<Url, String> {
    let value = env::var("MIR2_HOME_SUPERVISOR_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SUPERVISOR_URL.to_string());
    validate_supervisor_url(&value)
}

fn validate_supervisor_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("invalid Supervisor URL: {error}"))?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"))
    {
        return Err(
            "Dubhe Node desktop only accepts a credential-free loopback Supervisor URL".to_string(),
        );
    }
    Ok(url)
}

fn supervisor_client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| format!("build Supervisor client: {error}"))
}

async fn fetch_status() -> Result<SupervisorStatus, String> {
    let endpoint = supervisor_url()?
        .join("/v1/status")
        .map_err(|error| format!("build Supervisor status URL: {error}"))?;
    supervisor_client()?
        .get(endpoint)
        .send()
        .await
        .map_err(|error| format!("connect to local Dubhe Node service: {error}"))?
        .error_for_status()
        .map_err(|error| format!("local Dubhe Node service rejected status: {error}"))?
        .json()
        .await
        .map_err(|error| format!("decode local Dubhe Node status: {error}"))
}

#[tauri::command]
async fn bootstrap_node() -> Result<DesktopBootstrap, String> {
    let account = keyring_account();
    let (identity, created) = HomeAgentKeyring::new(&account)?.load_or_create_identity()?;
    let (_, management_token_created) =
        HomeAgentManagementKeyring::new(&account)?.load_or_create_token()?;
    let status = fetch_status().await.ok();
    Ok(DesktopBootstrap {
        identity: DesktopIdentity {
            node_id: identity.node_id().to_string(),
            public_key: identity.public_key().to_string(),
            created,
            key_store: "operating-system-keyring",
        },
        supervisor_reachable: status.is_some(),
        status,
        management_token_created,
    })
}

#[tauri::command]
async fn node_status() -> Result<SupervisorStatus, String> {
    fetch_status().await
}

#[tauri::command]
async fn set_node_serving(serving: bool) -> Result<NodeActionReceipt, String> {
    let account = keyring_account();
    let token = HomeAgentManagementKeyring::new(account)?.load_token()?;
    let action = if serving { "resume" } else { "drain" };
    let endpoint = supervisor_url()?
        .join(&format!("/v1/{action}"))
        .map_err(|error| format!("build Supervisor action URL: {error}"))?;
    let response = supervisor_client()?
        .post(endpoint)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("request local Dubhe Node {action}: {error}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("local Dubhe Node {action} rejected with {status}"));
    }
    let status = fetch_status().await?;
    Ok(NodeActionReceipt {
        accepted: true,
        serving,
        status,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            bootstrap_node,
            node_status,
            set_node_serving
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dubhe Node desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_url_is_strictly_loopback() {
        assert!(validate_supervisor_url("http://127.0.0.1:17990").is_ok());
        assert!(validate_supervisor_url("http://localhost:17990").is_ok());
        assert!(validate_supervisor_url("https://127.0.0.1:17990").is_err());
        assert!(validate_supervisor_url("http://192.168.1.10:17990").is_err());
        assert!(validate_supervisor_url("http://user:secret@localhost:17990").is_err());
        assert!(validate_supervisor_url("http://localhost:17990?token=secret").is_err());
    }
}
