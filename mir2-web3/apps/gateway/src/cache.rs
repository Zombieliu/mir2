use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{routing::SessionRouteRequest, GatewaySession};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionCacheKey {
    pub account_id: String,
    pub character_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionCacheRecord {
    pub key: GatewaySessionCacheKey,
    pub character_name: String,
    #[serde(default, rename = "zoneId")]
    pub zone_id: Option<String>,
    pub map_file_name: Option<String>,
    pub player_object_id: Option<u32>,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub gold: u32,
    pub tick: u64,
    #[serde(default, rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    #[serde(default, rename = "routeLeaseOwner")]
    pub route_lease_owner: Option<String>,
    #[serde(default, rename = "routeLeaseExpiresAtMs")]
    pub route_lease_expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionRoute {
    pub key: GatewaySessionCacheKey,
    pub character_name: String,
    #[serde(default, rename = "zoneId")]
    pub zone_id: Option<String>,
    pub map_file_name: Option<String>,
    pub tick: u64,
    #[serde(default, rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    #[serde(default, rename = "routeLeaseOwner")]
    pub route_lease_owner: Option<String>,
    #[serde(default, rename = "routeLeaseExpiresAtMs")]
    pub route_lease_expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRouteLease {
    pub key: GatewaySessionCacheKey,
    pub owner: String,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionCacheStatus {
    pub configured: bool,
    pub backend: String,
    pub ttl_seconds: Option<u64>,
    pub record_count: usize,
    pub stale_record_count: usize,
    pub route_lease_count: usize,
    pub healthy: bool,
    pub last_error: Option<String>,
}

impl From<GatewaySessionCacheRecord> for GatewaySessionRoute {
    fn from(record: GatewaySessionCacheRecord) -> Self {
        Self {
            key: record.key,
            character_name: record.character_name,
            zone_id: record.zone_id,
            map_file_name: record.map_file_name,
            tick: record.tick,
            updated_at_ms: record.updated_at_ms,
            route_lease_owner: record.route_lease_owner,
            route_lease_expires_at_ms: record.route_lease_expires_at_ms,
        }
    }
}

pub trait GatewaySessionCache: Send + Sync {
    fn get(&self, key: &GatewaySessionCacheKey) -> Option<GatewaySessionCacheRecord>;
    fn list(&self) -> Vec<GatewaySessionCacheRecord>;
    fn put(&self, record: GatewaySessionCacheRecord);
    fn remove(&self, key: &GatewaySessionCacheKey);
    fn remove_owned(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
    ) -> Option<GatewaySessionCacheRecord> {
        let record = self.get(key)?;
        match record.route_lease_owner.as_deref() {
            Some(record_owner) if record_owner != owner => return None,
            _ => {}
        }
        self.remove(key);
        Some(record)
    }
    fn remove_character(&self, character_name: &str) -> Option<GatewaySessionCacheRecord>;
    fn acquire_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String>;
    fn renew_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String>;
    fn release_route_lease(&self, key: &GatewaySessionCacheKey, owner: &str) -> Result<(), String>;
    fn route_lease_count(&self) -> usize {
        0
    }
    fn status(&self) -> GatewaySessionCacheStatus {
        let records = self.list();
        GatewaySessionCacheStatus {
            configured: true,
            backend: "custom".to_string(),
            ttl_seconds: None,
            record_count: records.len(),
            stale_record_count: stale_record_count(&records, 30_000),
            route_lease_count: self.route_lease_count(),
            healthy: true,
            last_error: None,
        }
    }
    fn route_character(&self, character_name: &str) -> Option<GatewaySessionRoute> {
        self.list()
            .into_iter()
            .find(|record| record.character_name.eq_ignore_ascii_case(character_name))
            .map(GatewaySessionRoute::from)
    }
}

pub type SharedGatewaySessionCache = Arc<dyn GatewaySessionCache>;

#[derive(Debug, Default)]
pub struct InMemoryGatewaySessionCache {
    records: Mutex<BTreeMap<GatewaySessionCacheKey, GatewaySessionCacheRecord>>,
    route_leases: Mutex<BTreeMap<GatewaySessionCacheKey, GatewayRouteLease>>,
}

impl GatewaySessionCache for InMemoryGatewaySessionCache {
    fn get(&self, key: &GatewaySessionCacheKey) -> Option<GatewaySessionCacheRecord> {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .get(key)
            .cloned()
    }

    fn list(&self) -> Vec<GatewaySessionCacheRecord> {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .values()
            .cloned()
            .collect()
    }

    fn put(&self, record: GatewaySessionCacheRecord) {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .insert(record.key.clone(), record);
    }

    fn remove(&self, key: &GatewaySessionCacheKey) {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .remove(key);
        self.route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned")
            .remove(key);
    }

    fn remove_owned(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
    ) -> Option<GatewaySessionCacheRecord> {
        let mut records = self
            .records
            .lock()
            .expect("gateway session cache mutex should not be poisoned");
        let record = records.get(key)?;
        match record.route_lease_owner.as_deref() {
            Some(record_owner) if record_owner != owner => return None,
            _ => {}
        }
        let record = records.remove(key);
        self.route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned")
            .remove(key);
        record
    }

    fn remove_character(&self, character_name: &str) -> Option<GatewaySessionCacheRecord> {
        let mut records = self
            .records
            .lock()
            .expect("gateway session cache mutex should not be poisoned");
        let key = records
            .iter()
            .find(|(_, record)| record.character_name.eq_ignore_ascii_case(character_name))
            .map(|(key, _)| key.clone())?;
        let record = records.remove(&key);
        self.route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned")
            .remove(&key);
        record
    }

    fn acquire_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String> {
        let mut leases = self
            .route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned");
        let now_ms = current_unix_ms();
        if let Some(existing) = leases.get(key) {
            if existing.expires_at_ms > now_ms && existing.owner != owner {
                return Err(format!(
                    "route lease for {}/{} is held by {} until {}",
                    key.account_id, key.character_index, existing.owner, existing.expires_at_ms
                ));
            }
        }
        let lease = GatewayRouteLease {
            key: key.clone(),
            owner: owner.to_string(),
            expires_at_ms: now_ms.saturating_add(ttl_seconds.max(1).saturating_mul(1_000)),
        };
        leases.insert(key.clone(), lease.clone());
        Ok(lease)
    }

    fn renew_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String> {
        self.acquire_route_lease(key, owner, ttl_seconds)
    }

    fn release_route_lease(&self, key: &GatewaySessionCacheKey, owner: &str) -> Result<(), String> {
        let mut leases = self
            .route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned");
        if leases.get(key).is_some_and(|lease| lease.owner == owner) {
            leases.remove(key);
        }
        Ok(())
    }

    fn route_lease_count(&self) -> usize {
        self.route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned")
            .values()
            .filter(|lease| lease.expires_at_ms > current_unix_ms())
            .count()
    }

    fn status(&self) -> GatewaySessionCacheStatus {
        let records = self.list();
        GatewaySessionCacheStatus {
            configured: true,
            backend: "in_memory".to_string(),
            ttl_seconds: None,
            record_count: records.len(),
            stale_record_count: stale_record_count(&records, 30_000),
            route_lease_count: self.route_lease_count(),
            healthy: true,
            last_error: None,
        }
    }

    fn route_character(&self, character_name: &str) -> Option<GatewaySessionRoute> {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .values()
            .find(|record| record.character_name.eq_ignore_ascii_case(character_name))
            .cloned()
            .map(GatewaySessionRoute::from)
    }
}

#[derive(Debug, Clone)]
pub struct RedisGatewaySessionCache {
    addr: String,
    namespace: String,
    ttl_seconds: u64,
    timeout: Duration,
}

impl RedisGatewaySessionCache {
    pub fn new(redis_url: &str, namespace: impl Into<String>, ttl_seconds: u64) -> Self {
        Self {
            addr: redis_addr_from_url(redis_url),
            namespace: namespace.into(),
            ttl_seconds: ttl_seconds.max(1),
            timeout: Duration::from_millis(500),
        }
    }

    fn redis_key(&self, key: &GatewaySessionCacheKey) -> String {
        format!(
            "{}:{}:{}",
            self.namespace,
            sanitize_cache_key_part(&key.account_id),
            key.character_index
        )
    }

    fn character_index_key(&self, character_name: &str) -> String {
        format!(
            "{}:character:{}",
            self.namespace,
            sanitize_cache_key_part(&character_name.to_ascii_lowercase())
        )
    }

    fn route_lease_key(&self, key: &GatewaySessionCacheKey) -> String {
        format!(
            "{}:lease:{}:{}",
            self.namespace,
            sanitize_cache_key_part(&key.account_id),
            key.character_index
        )
    }

    fn execute(&self, args: &[String]) -> Result<RedisValue, String> {
        let mut stream = TcpStream::connect(&self.addr)
            .map_err(|error| format!("redis connect {} failed: {error}", self.addr))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| format!("redis read timeout setup failed: {error}"))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| format!("redis write timeout setup failed: {error}"))?;
        write_resp_command(&mut stream, args)?;
        read_resp_value(&mut stream)
    }

    pub fn ping(&self) -> Result<(), String> {
        match self.execute(&["PING".to_string()])? {
            RedisValue::Simple(value) if value == "PONG" => Ok(()),
            other => Err(format!("unexpected redis PING response: {other:?}")),
        }
    }

    fn list_keys(&self) -> Result<Vec<String>, String> {
        let mut cursor = "0".to_string();
        let mut keys = Vec::new();
        loop {
            let response = self.execute(&[
                "SCAN".to_string(),
                cursor.clone(),
                "MATCH".to_string(),
                format!("{}:*", self.namespace),
                "COUNT".to_string(),
                "100".to_string(),
            ])?;
            let RedisValue::Array(values) = response else {
                return Err(format!("unexpected redis SCAN response: {response:?}"));
            };
            if values.len() != 2 {
                return Err(format!("unexpected redis SCAN arity: {}", values.len()));
            }
            cursor = redis_string_value(&values[0])
                .ok_or_else(|| format!("unexpected redis SCAN cursor: {:?}", values[0]))?;
            let RedisValue::Array(batch) = &values[1] else {
                return Err(format!("unexpected redis SCAN key batch: {:?}", values[1]));
            };
            for key in batch {
                if let Some(key) = redis_string_value(key) {
                    keys.push(key);
                }
            }
            if cursor == "0" {
                return Ok(keys);
            }
        }
    }

    fn get_record_by_redis_key(&self, redis_key: &str) -> Option<GatewaySessionCacheRecord> {
        let value = match self.execute(&["GET".to_string(), redis_key.to_string()]) {
            Ok(RedisValue::Bulk(Some(value))) => value,
            Ok(RedisValue::Bulk(None)) => return None,
            Ok(other) => {
                eprintln!("unexpected redis session-cache list get response: {other:?}");
                return None;
            }
            Err(error) => {
                eprintln!("redis session-cache list get failed: {error}");
                return None;
            }
        };
        serde_json::from_str(&value).ok()
    }
}

impl GatewaySessionCache for RedisGatewaySessionCache {
    fn get(&self, key: &GatewaySessionCacheKey) -> Option<GatewaySessionCacheRecord> {
        let redis_key = self.redis_key(key);
        let value = match self.execute(&["GET".to_string(), redis_key]) {
            Ok(RedisValue::Bulk(Some(value))) => value,
            Ok(RedisValue::Bulk(None)) => return None,
            Ok(other) => {
                eprintln!("unexpected redis session-cache get response: {other:?}");
                return None;
            }
            Err(error) => {
                eprintln!("redis session-cache get failed: {error}");
                return None;
            }
        };
        serde_json::from_str(&value).ok()
    }

    fn list(&self) -> Vec<GatewaySessionCacheRecord> {
        let keys = match self.list_keys() {
            Ok(keys) => keys,
            Err(error) => {
                eprintln!("redis session-cache list failed: {error}");
                return Vec::new();
            }
        };
        let mut records = keys
            .iter()
            .filter_map(|key| self.get_record_by_redis_key(key))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.character_name
                .cmp(&right.character_name)
                .then_with(|| left.key.account_id.cmp(&right.key.account_id))
                .then(left.key.character_index.cmp(&right.key.character_index))
        });
        records.dedup_by(|left, right| left.key == right.key);
        records
    }

    fn put(&self, record: GatewaySessionCacheRecord) {
        let redis_key = self.redis_key(&record.key);
        let character_index_key = self.character_index_key(&record.character_name);
        let Ok(value) = serde_json::to_string(&record) else {
            eprintln!("redis session-cache record serialization failed");
            return;
        };
        let args = [
            "SETEX".to_string(),
            redis_key.clone(),
            self.ttl_seconds.to_string(),
            value,
        ];
        if let Err(error) = self.execute(&args) {
            eprintln!("redis session-cache put failed: {error}");
        }
        let index_args = [
            "SETEX".to_string(),
            character_index_key,
            self.ttl_seconds.to_string(),
            redis_key,
        ];
        if let Err(error) = self.execute(&index_args) {
            eprintln!("redis session-cache character index put failed: {error}");
        }
    }

    fn remove(&self, key: &GatewaySessionCacheKey) {
        let redis_key = self.redis_key(key);
        let lease_key = self.route_lease_key(key);
        let record = self.get(key);
        if let Err(error) = self.execute(&["DEL".to_string(), redis_key, lease_key]) {
            eprintln!("redis session-cache remove failed: {error}");
        }
        if let Some(record) = record {
            let character_index_key = self.character_index_key(&record.character_name);
            if let Err(error) = self.execute(&["DEL".to_string(), character_index_key]) {
                eprintln!("redis session-cache character index remove failed: {error}");
            }
        }
    }

    fn remove_owned(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
    ) -> Option<GatewaySessionCacheRecord> {
        let record = self.get(key)?;
        match record.route_lease_owner.as_deref() {
            Some(record_owner) if record_owner != owner => return None,
            _ => {}
        }
        self.remove(key);
        Some(record)
    }

    fn remove_character(&self, character_name: &str) -> Option<GatewaySessionCacheRecord> {
        let character_index_key = self.character_index_key(character_name);
        let redis_key = match self.execute(&["GET".to_string(), character_index_key.clone()]) {
            Ok(RedisValue::Bulk(Some(value))) => value,
            Ok(RedisValue::Bulk(None)) => return None,
            Ok(other) => {
                eprintln!("unexpected redis session-cache character index response: {other:?}");
                return None;
            }
            Err(error) => {
                eprintln!("redis session-cache character index get failed: {error}");
                return None;
            }
        };
        let record: Option<GatewaySessionCacheRecord> =
            match self.execute(&["GET".to_string(), redis_key.clone()]) {
                Ok(RedisValue::Bulk(Some(value))) => serde_json::from_str(&value).ok(),
                Ok(RedisValue::Bulk(None)) => None,
                Ok(other) => {
                    eprintln!("unexpected redis session-cache character get response: {other:?}");
                    None
                }
                Err(error) => {
                    eprintln!("redis session-cache character get failed: {error}");
                    None
                }
            };
        if let Err(error) = self.execute(&["DEL".to_string(), redis_key, character_index_key]) {
            eprintln!("redis session-cache character remove failed: {error}");
        }
        if let Some(record) = record.as_ref() {
            if let Err(error) =
                self.execute(&["DEL".to_string(), self.route_lease_key(&record.key)])
            {
                eprintln!("redis session-cache character lease remove failed: {error}");
            }
        }
        record
    }

    fn acquire_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String> {
        let ttl_seconds = ttl_seconds.max(1);
        let lease_key = self.route_lease_key(key);
        let response = self.execute(&[
            "SET".to_string(),
            lease_key.clone(),
            owner.to_string(),
            "EX".to_string(),
            ttl_seconds.to_string(),
            "NX".to_string(),
        ])?;
        match response {
            RedisValue::Simple(value) if value == "OK" => Ok(GatewayRouteLease {
                key: key.clone(),
                owner: owner.to_string(),
                expires_at_ms: current_unix_ms().saturating_add(ttl_seconds.saturating_mul(1_000)),
            }),
            RedisValue::Bulk(None) => {
                let current_owner = match self.execute(&["GET".to_string(), lease_key.clone()]) {
                    Ok(RedisValue::Bulk(Some(value))) | Ok(RedisValue::Simple(value)) => value,
                    Ok(other) => {
                        return Err(format!("unexpected redis route lease get: {other:?}"))
                    }
                    Err(error) => return Err(error),
                };
                if current_owner == owner {
                    return self.renew_route_lease(key, owner, ttl_seconds);
                }
                Err(format!(
                    "route lease for {}/{} is held by {current_owner}",
                    key.account_id, key.character_index
                ))
            }
            other => Err(format!(
                "unexpected redis route lease set response: {other:?}"
            )),
        }
    }

    fn renew_route_lease(
        &self,
        key: &GatewaySessionCacheKey,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<GatewayRouteLease, String> {
        let ttl_seconds = ttl_seconds.max(1);
        let lease_key = self.route_lease_key(key);
        let current_owner = match self.execute(&["GET".to_string(), lease_key.clone()]) {
            Ok(RedisValue::Bulk(Some(value))) | Ok(RedisValue::Simple(value)) => value,
            Ok(RedisValue::Bulk(None)) => return self.acquire_route_lease(key, owner, ttl_seconds),
            Ok(other) => return Err(format!("unexpected redis route lease get: {other:?}")),
            Err(error) => return Err(error),
        };
        if current_owner != owner {
            return Err(format!(
                "route lease for {}/{} is held by {current_owner}",
                key.account_id, key.character_index
            ));
        }
        match self.execute(&["EXPIRE".to_string(), lease_key, ttl_seconds.to_string()])? {
            RedisValue::Integer(1) => Ok(GatewayRouteLease {
                key: key.clone(),
                owner: owner.to_string(),
                expires_at_ms: current_unix_ms().saturating_add(ttl_seconds.saturating_mul(1_000)),
            }),
            other => Err(format!(
                "unexpected redis route lease expire response: {other:?}"
            )),
        }
    }

    fn release_route_lease(&self, key: &GatewaySessionCacheKey, owner: &str) -> Result<(), String> {
        let lease_key = self.route_lease_key(key);
        let current_owner = match self.execute(&["GET".to_string(), lease_key.clone()]) {
            Ok(RedisValue::Bulk(Some(value))) | Ok(RedisValue::Simple(value)) => value,
            Ok(RedisValue::Bulk(None)) => return Ok(()),
            Ok(other) => return Err(format!("unexpected redis route lease get: {other:?}")),
            Err(error) => return Err(error),
        };
        if current_owner == owner {
            let _ = self.execute(&["DEL".to_string(), lease_key])?;
        }
        Ok(())
    }

    fn status(&self) -> GatewaySessionCacheStatus {
        match self.ping() {
            Ok(()) => {
                let records = self.list();
                GatewaySessionCacheStatus {
                    configured: true,
                    backend: "redis".to_string(),
                    ttl_seconds: Some(self.ttl_seconds),
                    record_count: records.len(),
                    stale_record_count: stale_record_count(
                        &records,
                        self.ttl_seconds.saturating_mul(1_000),
                    ),
                    route_lease_count: records
                        .iter()
                        .filter(|record| record.route_lease_owner.is_some())
                        .count(),
                    healthy: true,
                    last_error: None,
                }
            }
            Err(error) => GatewaySessionCacheStatus {
                configured: true,
                backend: "redis".to_string(),
                ttl_seconds: Some(self.ttl_seconds),
                record_count: 0,
                stale_record_count: 0,
                route_lease_count: 0,
                healthy: false,
                last_error: Some(error),
            },
        }
    }

    fn route_character(&self, character_name: &str) -> Option<GatewaySessionRoute> {
        let character_index_key = self.character_index_key(character_name);
        let redis_key = match self.execute(&["GET".to_string(), character_index_key]) {
            Ok(RedisValue::Bulk(Some(value))) => value,
            Ok(RedisValue::Bulk(None)) => return None,
            Ok(other) => {
                eprintln!("unexpected redis session-cache route index response: {other:?}");
                return None;
            }
            Err(error) => {
                eprintln!("redis session-cache route index get failed: {error}");
                return None;
            }
        };
        let record: GatewaySessionCacheRecord = match self.execute(&["GET".to_string(), redis_key])
        {
            Ok(RedisValue::Bulk(Some(value))) => serde_json::from_str(&value).ok(),
            Ok(RedisValue::Bulk(None)) => None,
            Ok(other) => {
                eprintln!("unexpected redis session-cache route record response: {other:?}");
                None
            }
            Err(error) => {
                eprintln!("redis session-cache route record get failed: {error}");
                None
            }
        }?;
        Some(GatewaySessionRoute::from(record))
    }
}

pub fn default_gateway_session_cache_from_env() -> SharedGatewaySessionCache {
    match std::env::var("MIR2_GATEWAY_REDIS_CACHE_URL") {
        Ok(redis_url) if !redis_url.trim().is_empty() => {
            let ttl_seconds = std::env::var("MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(30);
            Arc::new(RedisGatewaySessionCache::new(
                &redis_url,
                "mir2:gateway:session",
                ttl_seconds,
            ))
        }
        _ => Arc::new(InMemoryGatewaySessionCache::default()),
    }
}

fn redis_addr_from_url(redis_url: &str) -> String {
    let trimmed = redis_url.trim();
    let without_scheme = trimmed.strip_prefix("redis://").unwrap_or(trimmed);
    without_scheme
        .split('/')
        .next()
        .filter(|addr| !addr.is_empty())
        .unwrap_or("127.0.0.1:6379")
        .to_string()
}

fn sanitize_cache_key_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
enum RedisValue {
    Simple(String),
    Bulk(Option<String>),
    Integer(i64),
    Array(Vec<RedisValue>),
}

fn write_resp_command(stream: &mut TcpStream, args: &[String]) -> Result<(), String> {
    let mut command = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        command.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        command.extend_from_slice(arg.as_bytes());
        command.extend_from_slice(b"\r\n");
    }
    stream
        .write_all(&command)
        .map_err(|error| format!("redis write failed: {error}"))
}

fn read_resp_value(stream: &mut TcpStream) -> Result<RedisValue, String> {
    let prefix = read_byte(stream)?;
    match prefix {
        b'+' => Ok(RedisValue::Simple(read_line(stream)?)),
        b':' => {
            let line = read_line(stream)?;
            let value = line
                .parse::<i64>()
                .map_err(|error| format!("redis integer parse failed: {error}"))?;
            Ok(RedisValue::Integer(value))
        }
        b'$' => {
            let line = read_line(stream)?;
            let len = line
                .parse::<isize>()
                .map_err(|error| format!("redis bulk length parse failed: {error}"))?;
            if len < 0 {
                return Ok(RedisValue::Bulk(None));
            }
            let mut data = vec![0_u8; len as usize];
            stream
                .read_exact(&mut data)
                .map_err(|error| format!("redis bulk read failed: {error}"))?;
            let mut crlf = [0_u8; 2];
            stream
                .read_exact(&mut crlf)
                .map_err(|error| format!("redis bulk terminator read failed: {error}"))?;
            if crlf != *b"\r\n" {
                return Err("redis bulk response missing CRLF".to_string());
            }
            String::from_utf8(data)
                .map(|value| RedisValue::Bulk(Some(value)))
                .map_err(|error| format!("redis bulk utf8 decode failed: {error}"))
        }
        b'*' => {
            let line = read_line(stream)?;
            let len = line
                .parse::<isize>()
                .map_err(|error| format!("redis array length parse failed: {error}"))?;
            if len < 0 {
                return Ok(RedisValue::Array(Vec::new()));
            }
            let mut values = Vec::with_capacity(len as usize);
            for _ in 0..len {
                values.push(read_resp_value(stream)?);
            }
            Ok(RedisValue::Array(values))
        }
        b'-' => Err(format!("redis error response: {}", read_line(stream)?)),
        other => Err(format!("unsupported redis response prefix: {other}")),
    }
}

fn redis_string_value(value: &RedisValue) -> Option<String> {
    match value {
        RedisValue::Simple(value) => Some(value.clone()),
        RedisValue::Bulk(Some(value)) => Some(value.clone()),
        RedisValue::Integer(value) => Some(value.to_string()),
        RedisValue::Bulk(None) | RedisValue::Array(_) => None,
    }
}

fn read_byte(stream: &mut TcpStream) -> Result<u8, String> {
    let mut buf = [0_u8; 1];
    stream
        .read_exact(&mut buf)
        .map_err(|error| format!("redis response read failed: {error}"))?;
    Ok(buf[0])
}

fn read_line(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::new();
    loop {
        let byte = read_byte(stream)?;
        buf.push(byte);
        if buf.ends_with(b"\r\n") {
            buf.truncate(buf.len() - 2);
            return String::from_utf8(buf)
                .map_err(|error| format!("redis line utf8 decode failed: {error}"));
        }
    }
}

pub fn session_cache_key(session: &GatewaySession) -> Option<GatewaySessionCacheKey> {
    let identity = session.active_identity()?;
    Some(GatewaySessionCacheKey {
        account_id: identity.account_id,
        character_index: identity.character_index,
    })
}

pub fn session_cache_record(session: &GatewaySession) -> Option<GatewaySessionCacheRecord> {
    let identity = session.active_identity()?;
    let snapshot = session.world_snapshot();
    Some(GatewaySessionCacheRecord {
        key: GatewaySessionCacheKey {
            account_id: identity.account_id,
            character_index: identity.character_index,
        },
        character_name: identity.character_name,
        zone_id: Some(session.zone_id().as_str().to_string()),
        map_file_name: snapshot.map_file_name,
        player_object_id: snapshot.player_object_id,
        player_hp: snapshot.player_hp,
        player_max_hp: snapshot.player_max_hp,
        gold: snapshot.gold,
        tick: snapshot.tick,
        updated_at_ms: current_unix_ms(),
        route_lease_owner: None,
        route_lease_expires_at_ms: None,
    })
}

pub fn refresh_session_cache(
    cache: &dyn GatewaySessionCache,
    session: &GatewaySession,
) -> Option<GatewaySessionCacheRecord> {
    let record = session_cache_record(session)?;
    cache.put(record.clone());
    Some(record)
}

pub fn refresh_session_cache_with_route_lease(
    cache: &dyn GatewaySessionCache,
    session: &GatewaySession,
    ttl_seconds: u64,
) -> Result<Option<GatewaySessionCacheRecord>, String> {
    let mut record = match session_cache_record(session) {
        Some(record) => record,
        None => return Ok(None),
    };
    let lease = cache.renew_route_lease(&record.key, session.session_id(), ttl_seconds)?;
    record.route_lease_owner = Some(lease.owner);
    record.route_lease_expires_at_ms = Some(lease.expires_at_ms);
    cache.put(record.clone());
    Ok(Some(record))
}

pub fn cached_session_record(
    cache: &dyn GatewaySessionCache,
    session: &GatewaySession,
) -> Option<GatewaySessionCacheRecord> {
    let key = session_cache_key(session)?;
    cache.get(&key)
}

pub fn remove_session_cache(
    cache: &dyn GatewaySessionCache,
    session: &GatewaySession,
) -> Option<GatewaySessionCacheKey> {
    let key = session_cache_key(session)?;
    cache.remove(&key);
    Some(key)
}

pub fn remove_owned_session_cache(
    cache: &dyn GatewaySessionCache,
    session: &GatewaySession,
) -> Option<GatewaySessionCacheKey> {
    let key = session_cache_key(session)?;
    let _ = cache.release_route_lease(&key, session.session_id());
    cache.remove_owned(&key, session.session_id())?;
    Some(key)
}

pub fn route_request_for_character(
    cache: &dyn GatewaySessionCache,
    character_name: &str,
) -> Option<SessionRouteRequest> {
    let route = cache.route_character(character_name)?;
    Some(route_request_from_route(route))
}

pub fn fresh_route_request_for_character(
    cache: &dyn GatewaySessionCache,
    character_name: &str,
    max_age_ms: u64,
) -> Option<SessionRouteRequest> {
    let route = cache.route_character(character_name)?;
    route_is_fresh(&route, current_unix_ms(), max_age_ms).then(|| route_request_from_route(route))
}

pub fn remove_stale_session_routes(cache: &dyn GatewaySessionCache, max_age_ms: u64) -> usize {
    let now_ms = current_unix_ms();
    cache
        .list()
        .into_iter()
        .filter(|record| record.updated_at_ms != 0)
        .filter(|record| now_ms.saturating_sub(record.updated_at_ms) > max_age_ms)
        .map(|record| {
            cache.remove(&record.key);
            1_usize
        })
        .sum()
}

pub fn gateway_session_cache_status(cache: &dyn GatewaySessionCache) -> GatewaySessionCacheStatus {
    cache.status()
}

fn route_request_from_route(route: GatewaySessionRoute) -> SessionRouteRequest {
    SessionRouteRequest {
        account_id: Some(route.key.account_id),
        character_index: Some(route.key.character_index),
        map_file_name: route.map_file_name,
    }
}

fn route_is_fresh(route: &GatewaySessionRoute, now_ms: u64, max_age_ms: u64) -> bool {
    let record_fresh =
        route.updated_at_ms == 0 || now_ms.saturating_sub(route.updated_at_ms) <= max_age_ms;
    let lease_fresh = route
        .route_lease_expires_at_ms
        .is_none_or(|expires_at_ms| expires_at_ms >= now_ms);
    record_fresh && lease_fresh
}

fn stale_record_count(records: &[GatewaySessionCacheRecord], max_age_ms: u64) -> usize {
    let now_ms = current_unix_ms();
    records
        .iter()
        .filter(|record| record.updated_at_ms != 0)
        .filter(|record| now_ms.saturating_sub(record.updated_at_ms) > max_age_ms)
        .count()
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        cached_session_record, refresh_session_cache, remove_session_cache,
        route_request_for_character, GatewaySessionCache, GatewaySessionCacheKey,
        InMemoryGatewaySessionCache, RedisGatewaySessionCache,
    };
    use crate::{
        GatewayConfig, GatewaySession, MapZoneSessionRouter, SharedSessionRouter,
        SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
    };
    use mir2_protocol::{ClientPacket, MirDirection};
    use std::{sync::Arc, time::Duration};

    fn login_and_start(session: &mut GatewaySession) {
        let _ = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    }

    #[test]
    fn session_cache_is_empty_until_character_enters_world() {
        let session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();

        assert!(refresh_session_cache(&cache, &session).is_none());
        assert!(cached_session_record(&cache, &session).is_none());
    }

    #[test]
    fn session_cache_hit_matches_authoritative_world_snapshot_after_refresh() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);

        let refreshed =
            refresh_session_cache(&cache, &session).expect("active session should cache");
        let cached =
            cached_session_record(&cache, &session).expect("active session should read cache");
        let listed = cache.list();

        assert_eq!(cached, refreshed);
        assert_eq!(listed, vec![refreshed]);
        assert_eq!(cached.key.account_id, "demo");
        assert_eq!(cached.key.character_index, 0);
        assert_eq!(cached.character_name, "Scout");
        assert_eq!(cached.zone_id.as_deref(), Some("primary"));
        assert_eq!(cached.map_file_name.as_deref(), Some("0"));
        assert_eq!(cached.gold, session.world_snapshot().gold);
    }

    #[test]
    fn session_cache_refresh_replaces_stale_position_state() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let first = refresh_session_cache(&cache, &session).expect("first cache record");

        let _ = session.handle_packet(ClientPacket::Walk {
            direction: MirDirection::Right,
        });
        let second = refresh_session_cache(&cache, &session).expect("second cache record");
        let cached = cached_session_record(&cache, &session).expect("cached record");

        assert_ne!(first.tick, second.tick);
        assert_eq!(cached, second);
    }

    #[test]
    fn session_cache_remove_clears_online_presence_record() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        refresh_session_cache(&cache, &session).expect("active session should cache");

        let removed = remove_session_cache(&cache, &session).expect("active session key");

        assert_eq!(removed.account_id, "demo");
        assert!(cached_session_record(&cache, &session).is_none());
        assert!(cache.list().is_empty());
    }

    #[test]
    fn session_cache_remove_character_clears_matching_presence_record() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        refresh_session_cache(&cache, &session).expect("active session should cache");

        let removed = cache
            .remove_character("Scout")
            .expect("character record should be removed");

        assert_eq!(removed.key.account_id, "demo");
        assert!(cached_session_record(&cache, &session).is_none());
        assert!(cache.list().is_empty());
    }

    #[test]
    fn session_cache_routes_online_character_to_zone() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let refreshed =
            refresh_session_cache(&cache, &session).expect("active session should cache");

        let route = cache
            .route_character("sCoUt")
            .expect("online character should have a route");

        assert_eq!(route.key, refreshed.key);
        assert_eq!(route.character_name, "Scout");
        assert_eq!(route.zone_id.as_deref(), Some("primary"));
        assert_eq!(route.map_file_name.as_deref(), Some("0"));
        assert_eq!(route.tick, refreshed.tick);
        assert!(cached_session_record(&cache, &session).is_some());
    }

    #[test]
    fn session_cache_route_request_can_drive_zone_registry() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        refresh_session_cache(&cache, &session).expect("active session should cache");
        let registry = ZoneRegistry::with_router(
            ZoneId::primary(),
            Arc::new(crate::SharedInProcessZoneRuntimeFactory::new()) as SharedZoneRuntimeFactory,
            Arc::new(MapZoneSessionRouter::new().with_route("0", ZoneId::new("bichon-0")))
                as SharedSessionRouter,
        );

        let route_request = route_request_for_character(&cache, "Scout")
            .expect("online character should produce route request");
        let routed = registry.open_session_for(GatewayConfig::default(), route_request);

        assert_eq!(routed.zone_id, ZoneId::new("bichon-0"));
    }

    #[test]
    fn session_cache_fresh_route_rejects_stale_presence_records() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let mut stale =
            refresh_session_cache(&cache, &session).expect("active session should cache");
        stale.updated_at_ms = super::current_unix_ms().saturating_sub(60_000);
        cache.put(stale);

        assert!(route_request_for_character(&cache, "Scout").is_some());
        assert!(super::fresh_route_request_for_character(&cache, "Scout", 1_000).is_none());
    }

    #[test]
    fn session_cache_stale_route_cleanup_keeps_fresh_records() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let fresh = refresh_session_cache(&cache, &session).expect("active session should cache");
        let mut stale = fresh.clone();
        stale.key = GatewaySessionCacheKey {
            account_id: "stale".to_string(),
            character_index: 7,
        };
        stale.character_name = "StaleScout".to_string();
        stale.updated_at_ms = super::current_unix_ms().saturating_sub(60_000);
        cache.put(stale.clone());

        let removed = super::remove_stale_session_routes(&cache, 1_000);

        assert_eq!(removed, 1);
        assert!(cache.get(&fresh.key).is_some());
        assert!(cache.get(&stale.key).is_none());
    }

    #[test]
    fn session_cache_status_reports_backend_and_stale_records() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let fresh = refresh_session_cache(&cache, &session).expect("active session should cache");
        let mut stale = fresh.clone();
        stale.key = GatewaySessionCacheKey {
            account_id: "stale-status".to_string(),
            character_index: 8,
        };
        stale.character_name = "StaleStatusScout".to_string();
        stale.updated_at_ms = super::current_unix_ms().saturating_sub(60_000);
        cache.put(stale);

        let status = super::gateway_session_cache_status(&cache);

        assert!(status.configured);
        assert_eq!(status.backend, "in_memory");
        assert_eq!(status.ttl_seconds, None);
        assert_eq!(status.record_count, 2);
        assert_eq!(status.stale_record_count, 1);
        assert!(status.healthy);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn session_cache_route_lease_blocks_stale_owner_overwrite_and_owned_remove() {
        let mut first = GatewaySession::new(GatewayConfig::default());
        let mut second = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut first);
        login_and_start(&mut second);

        let first_record = super::refresh_session_cache_with_route_lease(&cache, &first, 30)
            .expect("first lease should succeed")
            .expect("first session should cache");
        let second_error = super::refresh_session_cache_with_route_lease(&cache, &second, 30)
            .expect_err("second owner should be blocked while first lease is fresh");
        let route = cache
            .route_character("Scout")
            .expect("first route should remain visible");

        assert!(second_error.contains("route lease"));
        assert_eq!(
            first_record.route_lease_owner.as_deref(),
            Some(first.session_id())
        );
        assert_eq!(route.route_lease_owner.as_deref(), Some(first.session_id()));
        assert_eq!(super::remove_owned_session_cache(&cache, &second), None);
        assert!(cache.route_character("Scout").is_some());
        assert_eq!(
            super::remove_owned_session_cache(&cache, &first),
            Some(first_record.key)
        );
        assert!(cache.route_character("Scout").is_none());
    }

    #[test]
    fn redis_session_cache_roundtrips_removes_and_expires_records() {
        let redis_url = std::env::var("MIR2_TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let cache = RedisGatewaySessionCache::new(
            &redis_url,
            format!("mir2:test:session:{}", std::process::id()),
            1,
        );
        if cache.ping().is_err() {
            eprintln!("skipping redis session cache test because Redis is unavailable");
            return;
        }

        let mut session = GatewaySession::new(GatewayConfig::default());
        login_and_start(&mut session);

        let refreshed =
            refresh_session_cache(&cache, &session).expect("active session should cache");
        let cached =
            cached_session_record(&cache, &session).expect("redis cache should read session");
        let listed = cache.list();
        assert_eq!(cached, refreshed);
        assert_eq!(listed, vec![refreshed]);
        assert_eq!(cached.gold, session.world_snapshot().gold);
        let route = cache
            .route_character("Scout")
            .expect("redis cache should route online character");
        assert_eq!(route.zone_id.as_deref(), Some("primary"));
        assert_eq!(route.map_file_name.as_deref(), Some("0"));
        assert!(route.updated_at_ms > 0);
        assert!(super::fresh_route_request_for_character(&cache, "Scout", 5_000).is_some());

        let removed = remove_session_cache(&cache, &session).expect("active session key");
        assert_eq!(removed.account_id, "demo");
        assert!(cached_session_record(&cache, &session).is_none());
        assert!(cache.list().is_empty());

        refresh_session_cache(&cache, &session).expect("active session should cache again");
        std::thread::sleep(Duration::from_millis(1_200));
        assert!(cached_session_record(&cache, &session).is_none());
        assert!(cache.list().is_empty());

        cache.remove(&GatewaySessionCacheKey {
            account_id: "demo".to_string(),
            character_index: 0,
        });
    }

    #[test]
    fn redis_session_cache_route_lease_blocks_competing_owner() {
        let redis_url = std::env::var("MIR2_TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let cache = RedisGatewaySessionCache::new(
            &redis_url,
            format!("mir2:test:session:lease:{}", std::process::id()),
            30,
        );
        if cache.ping().is_err() {
            eprintln!("skipping redis session cache lease test because Redis is unavailable");
            return;
        }

        let mut first = GatewaySession::new(GatewayConfig::default());
        let mut second = GatewaySession::new(GatewayConfig::default());
        login_and_start(&mut first);
        login_and_start(&mut second);

        let first_record = super::refresh_session_cache_with_route_lease(&cache, &first, 30)
            .expect("first redis lease should succeed")
            .expect("first redis session should cache");
        let second_error = super::refresh_session_cache_with_route_lease(&cache, &second, 30)
            .expect_err("second redis owner should be blocked while first lease is fresh");
        let route = cache
            .route_character("Scout")
            .expect("redis route should remain visible");

        assert!(second_error.contains("route lease"));
        assert_eq!(route.route_lease_owner.as_deref(), Some(first.session_id()));
        assert_eq!(
            super::remove_owned_session_cache(&cache, &first),
            Some(first_record.key)
        );
        assert!(cache.route_character("Scout").is_none());
    }

    #[test]
    fn redis_session_cache_character_index_removes_matching_record() {
        let redis_url = std::env::var("MIR2_TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let cache = RedisGatewaySessionCache::new(
            &redis_url,
            format!("mir2:test:session:character:{}", std::process::id()),
            30,
        );
        if cache.ping().is_err() {
            eprintln!("skipping redis session cache character test because Redis is unavailable");
            return;
        }

        let mut session = GatewaySession::new(GatewayConfig::default());
        login_and_start(&mut session);
        refresh_session_cache(&cache, &session).expect("active session should cache");

        let removed = cache
            .remove_character("Scout")
            .expect("redis character index should remove record");

        assert_eq!(removed.key.account_id, "demo");
        assert!(cached_session_record(&cache, &session).is_none());
    }
}
