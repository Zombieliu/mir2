use std::env;
use std::path::PathBuf;

use mir2_gateway::tcp::run_tcp_gateway;
use mir2_gateway::web::run_web_gateway;
use mir2_gateway::GatewayConfig;

const DEFAULT_TCP_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_WEB_ADDR: &str = "127.0.0.1:7010";
const DEFAULT_ACCOUNT_STORE_PATH: &str = ".mir2-data/accounts.json";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let tcp_addr =
        env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_TCP_ADDR.to_string());
    let web_addr =
        env::var("MIR2_GATEWAY_WEB_ADDR").unwrap_or_else(|_| DEFAULT_WEB_ADDR.to_string());
    let account_store_path =
        env::var("MIR2_ACCOUNT_STORE_PATH").unwrap_or_else(|_| DEFAULT_ACCOUNT_STORE_PATH.into());
    let config =
        GatewayConfig::default().with_account_store_path(PathBuf::from(account_store_path));

    tokio::try_join!(
        run_tcp_gateway(&tcp_addr, config.clone()),
        run_web_gateway(&web_addr, config),
    )?;

    Ok(())
}
