use std::env;
use std::net::SocketAddr;

use mir2_admin_api::admin_router;

#[tokio::main]
async fn main() {
    let addr = env::var("ADMIN_API_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:7420".into())
        .parse::<SocketAddr>()
        .expect("ADMIN_API_ADDR must be a socket address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("admin api bind should succeed");

    println!("mir2-admin-api listening on http://{addr}");
    axum::serve(listener, admin_router())
        .await
        .expect("admin api server should run");
}
