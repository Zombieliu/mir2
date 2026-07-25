use std::collections::{BTreeMap, BTreeSet};
use std::env;
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
    #[serde(default, rename = "zoneOwnerId")]
    pub zone_owner_id: Option<String>,
    #[serde(default, rename = "zoneOwnerFencingToken")]
    pub zone_owner_fencing_token: Option<u64>,
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
    #[serde(default, rename = "zoneOwnerId")]
    pub zone_owner_id: Option<String>,
    #[serde(default, rename = "zoneOwnerFencingToken")]
    pub zone_owner_fencing_token: Option<u64>,
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
            zone_owner_id: record.zone_owner_id,
            zone_owner_fencing_token: record.zone_owner_fencing_token,
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
    fn refresh_owned_route_lease_record(
        &self,
        mut record: GatewaySessionCacheRecord,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<bool, String> {
        let current = match self.get(&record.key) {
            Some(current) if current.route_lease_owner.as_deref() == Some(owner) => current,
            Some(current) if current.route_lease_owner.is_none() => current,
            Some(_) | None => return Ok(false),
        };
        let lease = self.renew_route_lease(&current.key, owner, ttl_seconds)?;
        record.route_lease_owner = Some(lease.owner);
        record.route_lease_expires_at_ms = Some(lease.expires_at_ms);
        self.put(record);
        Ok(true)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewaySessionCacheRuntimeBackend {
    InMemory,
    Redis,
}

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

    fn refresh_owned_route_lease_record(
        &self,
        mut record: GatewaySessionCacheRecord,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<bool, String> {
        let mut records = self
            .records
            .lock()
            .expect("gateway session cache mutex should not be poisoned");
        if !records.contains_key(&record.key) {
            return Ok(false);
        }
        let mut leases = self
            .route_leases
            .lock()
            .expect("gateway route lease mutex should not be poisoned");
        let now_ms = current_unix_ms();
        let Some(existing) = leases.get_mut(&record.key) else {
            return Ok(false);
        };
        if existing.owner != owner {
            return Err(format!(
                "route lease for {}/{} is held by {} until {}",
                record.key.account_id,
                record.key.character_index,
                existing.owner,
                existing.expires_at_ms
            ));
        }
        if existing.expires_at_ms <= now_ms {
            return Ok(false);
        }
        existing.expires_at_ms = now_ms.saturating_add(ttl_seconds.max(1).saturating_mul(1_000));
        record.updated_at_ms = now_ms;
        record.route_lease_owner = Some(existing.owner.clone());
        record.route_lease_expires_at_ms = Some(existing.expires_at_ms);
        records.insert(record.key.clone(), record);
        Ok(true)
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
    addr: Arc<Mutex<String>>,
    sentinel_addrs: Arc<Vec<String>>,
    sentinel_master_name: Option<String>,
    namespace: String,
    ttl_seconds: u64,
    timeout: Duration,
}

impl RedisGatewaySessionCache {
    pub fn new(redis_url: &str, namespace: impl Into<String>, ttl_seconds: u64) -> Self {
        Self {
            addr: Arc::new(Mutex::new(redis_addr_from_url(redis_url))),
            sentinel_addrs: Arc::new(Vec::new()),
            sentinel_master_name: None,
            namespace: namespace.into(),
            ttl_seconds: ttl_seconds.max(1),
            timeout: Duration::from_millis(500),
        }
    }

    pub fn with_sentinels(
        sentinel_urls: &str,
        master_name: impl Into<String>,
        namespace: impl Into<String>,
        ttl_seconds: u64,
    ) -> Result<Self, String> {
        let sentinel_addrs = sentinel_urls
            .split(',')
            .map(redis_addr_from_url)
            .filter(|address| !address.trim().is_empty())
            .collect::<Vec<_>>();
        if sentinel_addrs.iter().collect::<BTreeSet<_>>().len() < 3 {
            return Err(
                "Redis HA requires at least three distinct MIR2_GATEWAY_REDIS_SENTINEL_ADDRS"
                    .to_string(),
            );
        }
        let master_name = master_name.into();
        if master_name.trim().is_empty() {
            return Err("Redis Sentinel master name must not be empty".to_string());
        }
        let timeout = Duration::from_millis(500);
        let addr = discover_redis_master(&sentinel_addrs, &master_name, timeout)?;
        Ok(Self {
            addr: Arc::new(Mutex::new(addr)),
            sentinel_addrs: Arc::new(sentinel_addrs),
            sentinel_master_name: Some(master_name),
            namespace: namespace.into(),
            ttl_seconds: ttl_seconds.max(1),
            timeout,
        })
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

    fn character_index_key_prefix(&self) -> String {
        format!("{}:character:", self.namespace)
    }

    fn route_lease_key(&self, key: &GatewaySessionCacheKey) -> String {
        format!(
            "{}:lease:{}:{}",
            self.namespace,
            sanitize_cache_key_part(&key.account_id),
            key.character_index
        )
    }

    fn route_lease_key_prefix(&self) -> String {
        format!("{}:lease:", self.namespace)
    }

    fn is_character_index_key(&self, key: &str) -> bool {
        key.starts_with(&self.character_index_key_prefix())
    }

    fn is_route_lease_key(&self, key: &str) -> bool {
        key.starts_with(&self.route_lease_key_prefix())
    }

    fn is_session_record_key(&self, key: &str) -> bool {
        let namespace_prefix = format!("{}:", self.namespace);
        key.starts_with(&namespace_prefix)
            && !self.is_character_index_key(key)
            && !self.is_route_lease_key(key)
    }

    fn execute(&self, args: &[String]) -> Result<RedisValue, String> {
        let current = self
            .addr
            .lock()
            .map_err(|_| "Redis master address mutex poisoned".to_string())?
            .clone();
        match execute_redis_at(&current, args, self.timeout) {
            Ok(value) => Ok(value),
            Err(first_error) => {
                let Some(master_name) = self.sentinel_master_name.as_deref() else {
                    return Err(first_error);
                };
                let discovered =
                    discover_redis_master(&self.sentinel_addrs, master_name, self.timeout)
                        .map_err(|sentinel_error| {
                            format!(
                                "{first_error}; Redis Sentinel discovery failed: {sentinel_error}"
                            )
                        })?;
                *self
                    .addr
                    .lock()
                    .map_err(|_| "Redis master address mutex poisoned".to_string())? =
                    discovered.clone();
                execute_redis_at(&discovered, args, self.timeout).map_err(|retry_error| {
                    format!(
                        "{first_error}; Redis retry through discovered master {discovered} failed: {retry_error}"
                    )
                })
            }
        }
    }

    pub fn ping(&self) -> Result<(), String> {
        match self.execute(&["PING".to_string()])? {
            RedisValue::Simple(value) if value == "PONG" => Ok(()),
            other => Err(format!("unexpected redis PING response: {other:?}")),
        }
    }

    pub fn current_master_address(&self) -> Result<String, String> {
        self.addr
            .lock()
            .map(|address| address.clone())
            .map_err(|_| "Redis master address mutex poisoned".to_string())
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

    fn get_records_by_redis_keys(&self, keys: &[String]) -> Vec<GatewaySessionCacheRecord> {
        if keys.is_empty() {
            return Vec::new();
        }
        let mut args = Vec::with_capacity(keys.len() + 1);
        args.push("MGET".to_string());
        args.extend(keys.iter().cloned());
        let response = match self.execute(&args) {
            Ok(response) => response,
            Err(error) => {
                eprintln!("redis session-cache MGET failed: {error}");
                return Vec::new();
            }
        };
        let RedisValue::Array(values) = response else {
            eprintln!("unexpected redis session-cache MGET response: {response:?}");
            return Vec::new();
        };
        values
            .into_iter()
            .filter_map(|value| match value {
                RedisValue::Bulk(Some(value)) => serde_json::from_str(&value).ok(),
                RedisValue::Bulk(None) => None,
                other => {
                    eprintln!("unexpected redis session-cache MGET item: {other:?}");
                    None
                }
            })
            .collect()
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
        let record_keys = keys
            .into_iter()
            .filter(|key| self.is_session_record_key(key))
            .collect::<Vec<_>>();
        let mut records = self.get_records_by_redis_keys(&record_keys);
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

    fn refresh_owned_route_lease_record(
        &self,
        mut record: GatewaySessionCacheRecord,
        owner: &str,
        ttl_seconds: u64,
    ) -> Result<bool, String> {
        let ttl_seconds = ttl_seconds.max(1);
        let now_ms = current_unix_ms();
        record.updated_at_ms = now_ms;
        record.route_lease_owner = Some(owner.to_string());
        record.route_lease_expires_at_ms =
            Some(now_ms.saturating_add(ttl_seconds.saturating_mul(1_000)));
        let redis_key = self.redis_key(&record.key);
        let lease_key = self.route_lease_key(&record.key);
        let character_index_key = self.character_index_key(&record.character_name);
        let value = serde_json::to_string(&record)
            .map_err(|error| format!("redis session-cache record serialization failed: {error}"))?;
        let script = r#"
local owner = redis.call("GET", KEYS[1])
if owner == ARGV[1] then
  redis.call("EXPIRE", KEYS[1], ARGV[2])
  redis.call("SETEX", KEYS[2], ARGV[3], ARGV[4])
  redis.call("SETEX", KEYS[3], ARGV[3], KEYS[2])
  return 1
end
if owner == false then
  return 0
end
return -1
"#;
        match self.execute(&[
            "EVAL".to_string(),
            script.to_string(),
            "3".to_string(),
            lease_key,
            redis_key,
            character_index_key,
            owner.to_string(),
            ttl_seconds.to_string(),
            self.ttl_seconds.to_string(),
            value,
        ])? {
            RedisValue::Integer(1) => Ok(true),
            RedisValue::Integer(0) => Ok(false),
            RedisValue::Integer(-1) => Err(format!(
                "route lease for {}/{} is held by another owner",
                record.key.account_id, record.key.character_index
            )),
            other => Err(format!(
                "unexpected redis owned route lease refresh response: {other:?}"
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

    fn route_lease_count(&self) -> usize {
        let prefix = self.route_lease_key_prefix();
        match self.list_keys() {
            Ok(keys) => keys
                .into_iter()
                .filter(|key| key.starts_with(&prefix))
                .count(),
            Err(error) => {
                eprintln!("redis route-lease count failed: {error}");
                0
            }
        }
    }

    fn status(&self) -> GatewaySessionCacheStatus {
        match self.ping() {
            Ok(()) => {
                let keys = match self.list_keys() {
                    Ok(keys) => keys,
                    Err(error) => {
                        return GatewaySessionCacheStatus {
                            configured: true,
                            backend: "redis".to_string(),
                            ttl_seconds: Some(self.ttl_seconds),
                            record_count: 0,
                            stale_record_count: 0,
                            route_lease_count: 0,
                            healthy: false,
                            last_error: Some(error),
                        };
                    }
                };
                let route_lease_count = keys
                    .iter()
                    .filter(|key| self.is_route_lease_key(key))
                    .count();
                let record_keys = keys
                    .into_iter()
                    .filter(|key| self.is_session_record_key(key))
                    .collect::<Vec<_>>();
                let records = self.get_records_by_redis_keys(&record_keys);
                GatewaySessionCacheStatus {
                    configured: true,
                    backend: "redis".to_string(),
                    ttl_seconds: Some(self.ttl_seconds),
                    record_count: records.len(),
                    stale_record_count: stale_record_count(
                        &records,
                        self.ttl_seconds.saturating_mul(1_000),
                    ),
                    route_lease_count,
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

pub fn gateway_session_cache_requires_redis_from_env() -> bool {
    if env_flag_enabled("MIR2_GATEWAY_REQUIRE_REDIS_CACHE") {
        return true;
    }
    mir2_simulation::account_store_requires_postgres_source_from_env()
}

pub fn gateway_session_cache_runtime_backend_from_env(
) -> Result<GatewaySessionCacheRuntimeBackend, String> {
    if env::var("MIR2_GATEWAY_REDIS_SENTINEL_ADDRS")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(GatewaySessionCacheRuntimeBackend::Redis);
    }
    match env::var("MIR2_GATEWAY_REDIS_CACHE_URL") {
        Ok(redis_url) if !redis_url.trim().is_empty() => {
            Ok(GatewaySessionCacheRuntimeBackend::Redis)
        }
        _ if gateway_session_cache_requires_redis_from_env() => Err(
            "prod-like Gateway requires MIR2_GATEWAY_REDIS_CACHE_URL for Redis session/routing cache"
                .to_string(),
        ),
        _ => Ok(GatewaySessionCacheRuntimeBackend::InMemory),
    }
}

pub fn gateway_session_cache_from_env() -> Result<SharedGatewaySessionCache, String> {
    match gateway_session_cache_runtime_backend_from_env()? {
        GatewaySessionCacheRuntimeBackend::Redis => {
            let ttl_seconds = env::var("MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(30);
            let cache = match env::var("MIR2_GATEWAY_REDIS_SENTINEL_ADDRS")
                .ok()
                .filter(|value| !value.trim().is_empty())
            {
                Some(sentinel_urls) => RedisGatewaySessionCache::with_sentinels(
                    &sentinel_urls,
                    env::var("MIR2_GATEWAY_REDIS_SENTINEL_MASTER")
                        .unwrap_or_else(|_| "mir2-primary".to_string()),
                    "mir2:gateway:session",
                    ttl_seconds,
                )?,
                None => {
                    let redis_url = env::var("MIR2_GATEWAY_REDIS_CACHE_URL").map_err(|_| {
                        "MIR2_GATEWAY_REDIS_CACHE_URL or MIR2_GATEWAY_REDIS_SENTINEL_ADDRS is required for Redis session/routing cache"
                            .to_string()
                    })?;
                    RedisGatewaySessionCache::new(&redis_url, "mir2:gateway:session", ttl_seconds)
                }
            };
            if gateway_session_cache_requires_redis_from_env() {
                cache.ping().map_err(|error| {
                    format!("required Redis session/routing cache is unavailable: {error}")
                })?;
            }
            Ok(Arc::new(cache))
        }
        GatewaySessionCacheRuntimeBackend::InMemory => {
            Ok(Arc::new(InMemoryGatewaySessionCache::default()))
        }
    }
}

pub fn default_gateway_session_cache_from_env() -> SharedGatewaySessionCache {
    gateway_session_cache_from_env().unwrap_or_else(|error| panic!("{error}"))
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
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

fn execute_redis_at(
    address: &str,
    args: &[String],
    timeout: Duration,
) -> Result<RedisValue, String> {
    let mut stream = TcpStream::connect(address)
        .map_err(|error| format!("redis connect {address} failed: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("redis read timeout setup failed: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("redis write timeout setup failed: {error}"))?;
    write_resp_command(&mut stream, args)?;
    read_resp_value(&mut stream)
}

fn discover_redis_master(
    sentinel_addrs: &[String],
    master_name: &str,
    timeout: Duration,
) -> Result<String, String> {
    let command = [
        "SENTINEL".to_string(),
        "get-master-addr-by-name".to_string(),
        master_name.to_string(),
    ];
    let mut failures = Vec::new();
    for sentinel in sentinel_addrs {
        match execute_redis_at(sentinel, &command, timeout) {
            Ok(RedisValue::Array(values)) if values.len() == 2 => {
                let host = redis_string_value(&values[0]);
                let port = redis_string_value(&values[1]);
                if let (Some(host), Some(port)) = (host, port) {
                    if !host.trim().is_empty() && port.parse::<u16>().is_ok() {
                        return Ok(format!("{host}:{port}"));
                    }
                }
                failures.push(format!("{sentinel}: invalid master address response"));
            }
            Ok(other) => failures.push(format!("{sentinel}: unexpected response {other:?}")),
            Err(error) => failures.push(format!("{sentinel}: {error}")),
        }
    }
    Err(format!(
        "no Redis Sentinel resolved master {master_name}: {}",
        failures.join("; ")
    ))
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
    let zone_owner_lease = session.zone_owner_lease();
    Some(GatewaySessionCacheRecord {
        key: GatewaySessionCacheKey {
            account_id: identity.account_id,
            character_index: identity.character_index,
        },
        character_name: identity.character_name,
        zone_id: Some(session.zone_id().as_str().to_string()),
        zone_owner_id: Some(zone_owner_lease.owner_id().to_string()),
        zone_owner_fencing_token: Some(zone_owner_lease.fencing_token()),
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
        affinity_key: None,
        explicit_line: None,
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
        GatewaySessionCacheRuntimeBackend, InMemoryGatewaySessionCache, RedisGatewaySessionCache,
    };
    use crate::{
        GatewayConfig, GatewaySession, MapZoneSessionRouter, SharedSessionRouter,
        SharedZoneRuntimeFactory, ZoneId, ZoneRegistry,
    };
    use mir2_protocol::{ClientPacket, MirDirection};
    use std::{
        sync::{Arc, Mutex, OnceLock},
        time::Duration,
    };

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_isolated_session_cache_env<T>(
        vars: &[(&str, Option<&str>)],
        action: impl FnOnce() -> T,
    ) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned");
        let names = [
            "MIR2_GATEWAY_REDIS_CACHE_URL",
            "MIR2_GATEWAY_REDIS_SENTINEL_ADDRS",
            "MIR2_GATEWAY_REDIS_SENTINEL_MASTER",
            "MIR2_GATEWAY_REQUIRE_REDIS_CACHE",
            "MIR2_GATEWAY_SESSION_CACHE_TTL_SECONDS",
            "MIR2_RUNTIME_ENV",
            "MIR2_DEPLOYMENT_ENV",
            "MIR2_ENV",
            "MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES",
        ];
        let previous = names.map(|name| (name, std::env::var(name).ok()));
        for name in names {
            std::env::remove_var(name);
        }
        for (name, value) in vars {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }

        let result = action();

        for (name, value) in previous {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    fn login_and_start(session: &mut GatewaySession) {
        let _ = session.handle_packet(ClientPacket::Login {
            account_id: "demo".to_string(),
            password: "demo".to_string(),
        });
        let _ = session.handle_packet(ClientPacket::StartGame { character_index: 0 });
    }

    #[test]
    fn session_cache_environment_defaults_to_in_memory_for_local_dev() {
        with_isolated_session_cache_env(&[], || {
            assert!(!super::gateway_session_cache_requires_redis_from_env());
            assert_eq!(
                super::gateway_session_cache_runtime_backend_from_env(),
                Ok(GatewaySessionCacheRuntimeBackend::InMemory)
            );
            let cache = super::gateway_session_cache_from_env()
                .expect("local dev should use in-memory cache");
            assert_eq!(cache.status().backend, "in_memory");
        });
    }

    #[test]
    fn session_cache_environment_uses_redis_when_url_is_configured() {
        with_isolated_session_cache_env(
            &[(
                "MIR2_GATEWAY_REDIS_CACHE_URL",
                Some("redis://127.0.0.1:6379"),
            )],
            || {
                assert!(!super::gateway_session_cache_requires_redis_from_env());
                assert_eq!(
                    super::gateway_session_cache_runtime_backend_from_env(),
                    Ok(GatewaySessionCacheRuntimeBackend::Redis)
                );
                let cache = super::gateway_session_cache_from_env()
                    .expect("redis url should select redis cache without prod ping gate");
                assert_eq!(cache.status().backend, "redis");
            },
        );
    }

    #[test]
    fn session_cache_environment_requires_redis_for_production_like_gateway() {
        with_isolated_session_cache_env(&[("MIR2_RUNTIME_ENV", Some("staging"))], || {
            assert!(super::gateway_session_cache_requires_redis_from_env());
            let error = super::gateway_session_cache_runtime_backend_from_env()
                .expect_err("staging should reject missing Redis cache");
            assert!(error.contains("MIR2_GATEWAY_REDIS_CACHE_URL"));
            assert!(super::gateway_session_cache_from_env().is_err());
        });
    }

    #[test]
    fn session_cache_environment_explicit_flag_requires_redis() {
        with_isolated_session_cache_env(
            &[("MIR2_GATEWAY_REQUIRE_REDIS_CACHE", Some("true"))],
            || {
                assert!(super::gateway_session_cache_requires_redis_from_env());
                let error = super::gateway_session_cache_runtime_backend_from_env()
                    .expect_err("explicit flag should reject missing Redis cache");
                assert!(error.contains("MIR2_GATEWAY_REDIS_CACHE_URL"));
            },
        );
    }

    #[test]
    fn session_cache_environment_postgres_account_policy_requires_redis() {
        with_isolated_session_cache_env(
            &[("MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES", Some("1"))],
            || {
                assert!(super::gateway_session_cache_requires_redis_from_env());
                let error = super::gateway_session_cache_runtime_backend_from_env()
                    .expect_err("Postgres account-source policy should require Redis routing");
                assert!(error.contains("MIR2_GATEWAY_REDIS_CACHE_URL"));
            },
        );
    }

    #[test]
    fn session_cache_environment_required_redis_must_be_reachable() {
        with_isolated_session_cache_env(
            &[
                ("MIR2_GATEWAY_REQUIRE_REDIS_CACHE", Some("1")),
                ("MIR2_GATEWAY_REDIS_CACHE_URL", Some("redis://127.0.0.1:0")),
            ],
            || {
                assert_eq!(
                    super::gateway_session_cache_runtime_backend_from_env(),
                    Ok(GatewaySessionCacheRuntimeBackend::Redis)
                );
                let error = super::gateway_session_cache_from_env()
                    .err()
                    .expect("required Redis should be pinged before startup succeeds");
                assert!(error.contains("required Redis session/routing cache is unavailable"));
            },
        );
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
        assert_eq!(cached.zone_owner_id.as_deref(), Some("in-process:primary"));
        assert_eq!(cached.zone_owner_fencing_token, Some(1));
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
        assert_eq!(route.zone_owner_id.as_deref(), Some("in-process:primary"));
        assert_eq!(route.zone_owner_fencing_token, Some(1));
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
    fn redis_session_cache_key_filters_separate_records_from_indexes_and_leases() {
        let cache = RedisGatewaySessionCache::new("redis://127.0.0.1:6379", "mir2:test-status", 30);
        let record_key = cache.redis_key(&GatewaySessionCacheKey {
            account_id: "acct".to_string(),
            character_index: 3,
        });
        let character_key = cache.character_index_key("Scout");
        let lease_key = cache.route_lease_key(&GatewaySessionCacheKey {
            account_id: "acct".to_string(),
            character_index: 3,
        });

        assert!(cache.is_session_record_key(&record_key));
        assert!(!cache.is_session_record_key(&character_key));
        assert!(!cache.is_session_record_key(&lease_key));
        assert!(cache.is_character_index_key(&character_key));
        assert!(cache.is_route_lease_key(&lease_key));
    }

    #[test]
    fn redis_sentinel_cache_rediscovers_a_promoted_master_after_connection_failure() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        fn spawn_ping_server(listener: TcpListener) -> thread::JoinHandle<()> {
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("accept Redis ping");
                let mut request = [0_u8; 128];
                let _ = stream.read(&mut request);
                stream.write_all(b"+PONG\r\n").expect("write Redis PONG");
            })
        }

        let first_master_listener =
            TcpListener::bind("127.0.0.1:0").expect("bind first fake Redis master");
        let first_master = first_master_listener
            .local_addr()
            .expect("first fake master address");
        let second_master_listener =
            TcpListener::bind("127.0.0.1:0").expect("bind second fake Redis master");
        let second_master = second_master_listener
            .local_addr()
            .expect("second fake master address");
        let current_master = Arc::new(Mutex::new(first_master));

        let sentinel_listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Redis Sentinel");
        let sentinel_address = sentinel_listener.local_addr().expect("sentinel address");
        let sentinel_master = Arc::clone(&current_master);
        let sentinel = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = sentinel_listener.accept().expect("accept Sentinel query");
                let mut request = [0_u8; 256];
                let _ = stream.read(&mut request);
                let master = *sentinel_master.lock().expect("master mutex");
                let host = master.ip().to_string();
                let port = master.port().to_string();
                let response = format!(
                    "*2\r\n${}\r\n{}\r\n${}\r\n{}\r\n",
                    host.len(),
                    host,
                    port.len(),
                    port
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write Sentinel response");
            }
        });
        let unused_sentinel_a =
            TcpListener::bind("127.0.0.1:0").expect("bind second sentinel address");
        let unused_sentinel_b =
            TcpListener::bind("127.0.0.1:0").expect("bind third sentinel address");
        let sentinel_urls = format!(
            "{sentinel_address},{},{}",
            unused_sentinel_a.local_addr().expect("second sentinel"),
            unused_sentinel_b.local_addr().expect("third sentinel")
        );

        let first_master_server = spawn_ping_server(first_master_listener);
        let cache = RedisGatewaySessionCache::with_sentinels(
            &sentinel_urls,
            "mir2-primary",
            "mir2:test:sentinel",
            30,
        )
        .expect("Sentinel should resolve first master");
        cache.ping().expect("first master should answer");
        first_master_server.join().expect("first master server");

        *current_master.lock().expect("master mutex") = second_master;
        let second_master_server = spawn_ping_server(second_master_listener);
        cache
            .ping()
            .expect("cache should rediscover and use promoted master");

        second_master_server.join().expect("second master server");
        sentinel.join().expect("sentinel server");
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
    fn owned_route_refresh_does_not_resurrect_removed_session() {
        let mut session = GatewaySession::new(GatewayConfig::default());
        let cache = InMemoryGatewaySessionCache::default();
        login_and_start(&mut session);
        let record = super::refresh_session_cache_with_route_lease(&cache, &session, 30)
            .expect("lease should refresh")
            .expect("session should cache");

        assert_eq!(
            super::remove_owned_session_cache(&cache, &session),
            Some(record.key.clone())
        );
        let refreshed = cache
            .refresh_owned_route_lease_record(record, session.session_id(), 30)
            .expect("missing owned lease should be skipped");

        assert!(!refreshed);
        assert!(cache.route_character("Scout").is_none());
        assert_eq!(cache.route_lease_count(), 0);
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
        assert_eq!(route.zone_owner_id.as_deref(), Some("in-process:primary"));
        assert_eq!(route.zone_owner_fencing_token, Some(1));
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
        assert_eq!(cache.route_lease_count(), 1);
        assert_eq!(route.route_lease_owner.as_deref(), Some(first.session_id()));
        assert_eq!(
            super::remove_owned_session_cache(&cache, &first),
            Some(first_record.key)
        );
        assert!(cache.route_character("Scout").is_none());
        assert_eq!(cache.route_lease_count(), 0);
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
