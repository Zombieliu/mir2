use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::GatewaySession;

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
    pub map_file_name: Option<String>,
    pub player_object_id: Option<u32>,
    pub player_hp: Option<i32>,
    pub player_max_hp: Option<i32>,
    pub gold: u32,
    pub tick: u64,
}

pub trait GatewaySessionCache: Send + Sync {
    fn get(&self, key: &GatewaySessionCacheKey) -> Option<GatewaySessionCacheRecord>;
    fn put(&self, record: GatewaySessionCacheRecord);
    fn remove(&self, key: &GatewaySessionCacheKey);
    fn remove_character(&self, character_name: &str) -> Option<GatewaySessionCacheRecord>;
}

pub type SharedGatewaySessionCache = Arc<dyn GatewaySessionCache>;

#[derive(Debug, Default)]
pub struct InMemoryGatewaySessionCache {
    records: Mutex<BTreeMap<GatewaySessionCacheKey, GatewaySessionCacheRecord>>,
}

impl GatewaySessionCache for InMemoryGatewaySessionCache {
    fn get(&self, key: &GatewaySessionCacheKey) -> Option<GatewaySessionCacheRecord> {
        self.records
            .lock()
            .expect("gateway session cache mutex should not be poisoned")
            .get(key)
            .cloned()
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
        records.remove(&key)
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
        let record = self.get(key);
        if let Err(error) = self.execute(&["DEL".to_string(), redis_key]) {
            eprintln!("redis session-cache remove failed: {error}");
        }
        if let Some(record) = record {
            let character_index_key = self.character_index_key(&record.character_name);
            if let Err(error) = self.execute(&["DEL".to_string(), character_index_key]) {
                eprintln!("redis session-cache character index remove failed: {error}");
            }
        }
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
        let record = match self.execute(&["GET".to_string(), redis_key.clone()]) {
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
        record
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
        b'-' => Err(format!("redis error response: {}", read_line(stream)?)),
        other => Err(format!("unsupported redis response prefix: {other}")),
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
        map_file_name: snapshot.map_file_name,
        player_object_id: snapshot.player_object_id,
        player_hp: snapshot.player_hp,
        player_max_hp: snapshot.player_max_hp,
        gold: snapshot.gold,
        tick: snapshot.tick,
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

#[cfg(test)]
mod tests {
    use super::{
        cached_session_record, refresh_session_cache, remove_session_cache, GatewaySessionCache,
        GatewaySessionCacheKey, InMemoryGatewaySessionCache, RedisGatewaySessionCache,
    };
    use crate::{GatewayConfig, GatewaySession};
    use mir2_protocol::{ClientPacket, MirDirection};
    use std::time::Duration;

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

        assert_eq!(cached, refreshed);
        assert_eq!(cached.key.account_id, "demo");
        assert_eq!(cached.key.character_index, 0);
        assert_eq!(cached.character_name, "Scout");
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
        assert_eq!(cached, refreshed);
        assert_eq!(cached.gold, session.world_snapshot().gold);

        let removed = remove_session_cache(&cache, &session).expect("active session key");
        assert_eq!(removed.account_id, "demo");
        assert!(cached_session_record(&cache, &session).is_none());

        refresh_session_cache(&cache, &session).expect("active session should cache again");
        std::thread::sleep(Duration::from_millis(1_200));
        assert!(cached_session_record(&cache, &session).is_none());

        cache.remove(&GatewaySessionCacheKey {
            account_id: "demo".to_string(),
            character_index: 0,
        });
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
