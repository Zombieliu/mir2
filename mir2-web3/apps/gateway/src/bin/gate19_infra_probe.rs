use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mir2_gateway::zone_lease::PostgresZoneOwnerLeaseAuthority;
use mir2_gateway::{
    GatewaySessionCache, GatewaySessionCacheKey, GatewaySessionCacheRecord,
    RedisGatewaySessionCache, ZoneId,
};
use postgres::{Client, NoTls};
use serde::Serialize;

const DEFAULT_OUTPUT: &str = "docs/generated/regional/gate19-infra-probe.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatewayProbe {
    endpoint: String,
    healthy: bool,
    latency_ms: f64,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gate19InfraProbeEvidence {
    schema_version: u32,
    generated_at_ms: u64,
    phase: String,
    fault_started_at_ms: Option<u64>,
    recovery_rto_ms: Option<u64>,
    gateways: Vec<GatewayProbe>,
    healthy_gateway_count: usize,
    required_healthy_gateway_count: usize,
    redis_master_round_trip: bool,
    redis_master_address: String,
    redis_route_lease_owner: String,
    postgres_writable_primary: bool,
    postgres_server_address: String,
    postgres_zone_fencing_token: u64,
    assertions: BTreeMap<String, bool>,
    success: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let phase = env::var("MIR2_GATE19_PROBE_PHASE").unwrap_or_else(|_| "preflight".to_string());
    let fault_started_at_ms = env::var("MIR2_GATE19_FAULT_STARTED_AT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    let output = PathBuf::from(
        env::var("MIR2_GATE19_INFRA_PROBE_OUT").unwrap_or_else(|_| DEFAULT_OUTPUT.to_string()),
    );
    let gateway_endpoints = required_env("MIR2_GATE19_GATEWAY_ENDPOINTS")?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let required_healthy_gateway_count = env::var("MIR2_GATE19_REQUIRED_HEALTHY_GATEWAYS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3)
        .min(gateway_endpoints.len());
    let gateways = gateway_endpoints
        .iter()
        .map(|endpoint| probe_gateway(endpoint))
        .collect::<Vec<_>>();
    let healthy_gateway_count = gateways.iter().filter(|probe| probe.healthy).count();

    let sentinel_addrs = required_env("MIR2_GATEWAY_REDIS_SENTINEL_ADDRS")?;
    let sentinel_master = env::var("MIR2_GATEWAY_REDIS_SENTINEL_MASTER")
        .unwrap_or_else(|_| "mir2-primary".to_string());
    let cache = RedisGatewaySessionCache::with_sentinels(
        &sentinel_addrs,
        sentinel_master,
        "mir2:gate19:probe",
        30,
    )?;
    cache.ping()?;
    let redis_master_address = cache.current_master_address()?;
    let probe_key =
        env::var("MIR2_GATE19_PROBE_KEY").unwrap_or_else(|_| format!("{}-{}", phase, now_ms()));
    let route_owner =
        env::var("MIR2_GATE19_ROUTE_OWNER").unwrap_or_else(|_| format!("gate19-probe-{phase}"));
    let key = GatewaySessionCacheKey {
        account_id: probe_key.clone(),
        character_index: 1,
    };
    let route_lease = cache.acquire_route_lease(&key, &route_owner, 1)?;
    let record = GatewaySessionCacheRecord {
        key: key.clone(),
        character_name: format!("Gate19{phase}"),
        zone_id: Some("map:0".to_string()),
        zone_owner_id: Some("gate19-active".to_string()),
        zone_owner_fencing_token: Some(1),
        map_file_name: Some("0".to_string()),
        player_object_id: Some(99_000),
        player_hp: Some(100),
        player_max_hp: Some(100),
        gold: 100,
        tick: 1,
        updated_at_ms: now_ms(),
        route_lease_owner: Some(route_lease.owner.clone()),
        route_lease_expires_at_ms: Some(route_lease.expires_at_ms),
    };
    cache.put(record.clone());
    let redis_master_round_trip = cache.get(&key).as_ref() == Some(&record);

    let database_url = required_env("MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL")?;
    let mut client = Client::connect(&database_url, NoTls)
        .map_err(|error| format!("Gate 19 Postgres multi-host connect failed: {error}"))?;
    mir2_simulation::apply_migrations(&mut client)?;
    let row = client.query_one(
        "SELECT NOT pg_is_in_recovery() AS writable,
                COALESCE(inet_server_addr()::text, 'local') AS server_address",
        &[],
    )?;
    let postgres_writable_primary: bool = row.get("writable");
    let postgres_server_address: String = row.get("server_address");
    let authority =
        PostgresZoneOwnerLeaseAuthority::new(database_url, format!("gate19-probe-{phase}"), 5_000);
    let zone = ZoneId::new(format!("gate19-probe:{phase}:{}", now_ms()));
    let postgres_lease = authority.acquire_at(&zone, now_ms())?;
    let postgres_zone_fencing_token = postgres_lease.fencing_token();

    let generated_at_ms = now_ms();
    let recovery_rto_ms =
        fault_started_at_ms.map(|started_at_ms| generated_at_ms.saturating_sub(started_at_ms));
    let assertions = BTreeMap::from([
        (
            "gatewayQuorumHealthy".to_string(),
            healthy_gateway_count >= required_healthy_gateway_count,
        ),
        (
            "redisMasterRoundTripSucceeded".to_string(),
            redis_master_round_trip,
        ),
        (
            "redisRouteLeaseAcquired".to_string(),
            route_lease.owner == route_owner,
        ),
        (
            "postgresSelectedWritablePrimary".to_string(),
            postgres_writable_primary,
        ),
        (
            "postgresFencedWriteSucceeded".to_string(),
            postgres_zone_fencing_token > 0,
        ),
    ]);
    let success = assertions.values().all(|value| *value);
    let evidence = Gate19InfraProbeEvidence {
        schema_version: 1,
        generated_at_ms,
        phase,
        fault_started_at_ms,
        recovery_rto_ms,
        gateways,
        healthy_gateway_count,
        required_healthy_gateway_count,
        redis_master_round_trip,
        redis_master_address,
        redis_route_lease_owner: route_lease.owner,
        postgres_writable_primary,
        postgres_server_address,
        postgres_zone_fencing_token,
        assertions,
        success,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, serde_json::to_vec_pretty(&evidence)?)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    if success {
        Ok(())
    } else {
        Err("Gate 19 infrastructure probe assertions failed".into())
    }
}

fn probe_gateway(endpoint: &str) -> GatewayProbe {
    let started = Instant::now();
    let result = (|| -> Result<(), String> {
        let mut stream = TcpStream::connect(endpoint)
            .map_err(|error| format!("connect {endpoint} failed: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("read timeout failed: {error}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("write timeout failed: {error}"))?;
        stream
            .write_all(b"GET /health HTTP/1.0\r\nHost: gate19\r\n\r\n")
            .map_err(|error| format!("health request failed: {error}"))?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| format!("health response failed: {error}"))?;
        if response.starts_with("HTTP/1.0 200") || response.starts_with("HTTP/1.1 200") {
            Ok(())
        } else {
            Err(response
                .lines()
                .next()
                .unwrap_or("empty response")
                .to_string())
        }
    })();
    GatewayProbe {
        endpoint: endpoint.to_string(),
        healthy: result.is_ok(),
        latency_ms: started.elapsed().as_secs_f64() * 1_000.0,
        error: result.err(),
    }
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
