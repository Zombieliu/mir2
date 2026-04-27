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
    let database_backend = env::var("MIR2_ACCOUNT_STORE_BACKEND").unwrap_or_default();
    let config = if database_backend.eq_ignore_ascii_case("postgres") {
        let database_url = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL")
            .expect("MIR2_ACCOUNT_STORE_DATABASE_URL is required for postgres account store");
        GatewayConfig::default().with_postgres_account_store(database_url)
    } else {
        let account_store_path = env::var("MIR2_ACCOUNT_STORE_PATH")
            .unwrap_or_else(|_| DEFAULT_ACCOUNT_STORE_PATH.into());
        let mut config =
            GatewayConfig::default().with_account_store_path(PathBuf::from(account_store_path));
        if let Ok(database_url) = env::var("MIR2_ACCOUNT_STORE_DATABASE_URL") {
            config = config.with_account_store_database_url(database_url);
        }
        config
    };

    tokio::try_join!(
        run_tcp_gateway(&tcp_addr, config.clone()),
        run_web_gateway(&web_addr, config),
    )?;

    Ok(())
}
