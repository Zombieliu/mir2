use std::env;
use std::net::SocketAddr;

use mir2_admin_api::{
    admin_router_with_state, spawn_daily_report_scheduler, spawn_world_director_approval_scheduler,
    AdminApiState,
};

fn main() {
    let addr = env::var("ADMIN_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7420".into())
        .parse::<SocketAddr>()
        .expect("ADMIN_API_ADDR must be a socket address");
    let state = AdminApiState::from_env().expect("admin api state should initialize");

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime should build")
        .block_on(run(addr, state));
}

async fn run(addr: SocketAddr, state: AdminApiState) {
    spawn_daily_report_scheduler(state.clone());
    spawn_world_director_approval_scheduler(state.clone());
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("admin api bind should succeed");

    println!("mir2-admin-api listening on http://{addr}");
    axum::serve(listener, admin_router_with_state(state))
        .await
        .expect("admin api server should run");
}
