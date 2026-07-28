use std::env;
use std::io;
use std::path::PathBuf;

use mir2_gateway::tcp::chat_broadcast::ChatBroadcastHub;
use mir2_gateway::tcp::run_tcp_gateway;
use mir2_gateway::web::run_web_gateway;
use mir2_gateway::GatewayConfig;

const DEFAULT_TCP_ADDR: &str = "127.0.0.1:7000";
const DEFAULT_WEB_ADDR: &str = "127.0.0.1:7010";
const DEFAULT_ACCOUNT_STORE_PATH: &str = ".mir2-data/accounts.json";

fn main() -> std::io::Result<()> {
    let worker_threads = tokio_worker_threads_from_env();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .build()?
        .block_on(async_main())
}

async fn async_main() -> std::io::Result<()> {
    mir2_gateway::gate15::initialize_from_env()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
    let tcp_addr =
        env::var("MIR2_GATEWAY_TCP_ADDR").unwrap_or_else(|_| DEFAULT_TCP_ADDR.to_string());
    let web_addr =
        env::var("MIR2_GATEWAY_WEB_ADDR").unwrap_or_else(|_| DEFAULT_WEB_ADDR.to_string());
    let account_store_path =
        env::var("MIR2_ACCOUNT_STORE_PATH").unwrap_or_else(|_| DEFAULT_ACCOUNT_STORE_PATH.into());
    let config = GatewayConfig::default()
        .with_crystal_world_runtime()
        .with_account_store_environment(PathBuf::from(account_store_path))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let chat_hub = ChatBroadcastHub::from_env()?;
    let _chat_broadcast_task = chat_hub.spawn();

    tokio::try_join!(
        run_tcp_gateway(&tcp_addr, config.clone(), chat_hub.clone()),
        run_web_gateway(&web_addr, config, chat_hub),
    )?;

    Ok(())
}

fn tokio_worker_threads_from_env() -> usize {
    env::var("MIR2_GATEWAY_TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(4)
        })
        .clamp(1, 64)
}
