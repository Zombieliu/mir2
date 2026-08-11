use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use postgres::{GenericClient, NoTls};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const CHANNEL_IDENTITY_SCHEMA_VERSION: u32 = 1;
const MAX_PROVIDER_SUBJECT_BYTES: usize = 4 * 1024;
const CRAZYGAMES_PUBLIC_KEY_URL: &str = "https://sdk.crazygames.com/publicKey.json";
const CRAZYGAMES_PUBLIC_KEY_TTL: Duration = Duration::from_secs(5 * 60);
/// Steam Web API endpoint for validating a user's auth session ticket. This is
/// the server-side verifier: it never trusts a client-claimed SteamID.
const STEAM_AUTH_USER_TICKET_URL: &str =
    "https://api.steampowered.com/ISteamUserAuth/AuthenticateUserTicket/v1/";

struct CrazyGamesPublicKeyCache {
    public_key: String,
    fetched_at: Instant,
}

static CRAZYGAMES_PUBLIC_KEY_CACHE: OnceLock<tokio::sync::Mutex<Option<CrazyGamesPublicKeyCache>>> =
    OnceLock::new();

type ChannelIdentityPostgresPool = Pool<PostgresConnectionManager<NoTls>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChannelIdentityProvider {
    SuiPasskey,
    SuiWallet,
    CrazyGames,
    CrazyGamesGuest,
    Itch,
    Steam,
    DirectGuest,
}

impl ChannelIdentityProvider {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "suipasskey" | "sui-passkey" | "passkey" => Ok(Self::SuiPasskey),
            "suiwallet" | "sui-wallet" | "wallet" | "dubhewallet" | "dubhe-wallet" => {
                Ok(Self::SuiWallet)
            }
            "crazygames" | "crazy-games" => Ok(Self::CrazyGames),
            "crazygamesguest" | "crazy-games-guest" => Ok(Self::CrazyGamesGuest),
            "itch" | "itch.io" => Ok(Self::Itch),
            "steam" | "steamworks" => Ok(Self::Steam),
            "directguest" | "direct-guest" | "guest" => Ok(Self::DirectGuest),
            _ => Err(format!("unsupported channel identity provider {value}")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SuiPasskey => "suiPasskey",
            Self::SuiWallet => "suiWallet",
            Self::CrazyGames => "crazyGames",
            Self::CrazyGamesGuest => "crazyGamesGuest",
            Self::Itch => "itch",
            Self::Steam => "steam",
            Self::DirectGuest => "directGuest",
        }
    }

    pub const fn is_primary_capable(self) -> bool {
        matches!(
            self,
            Self::SuiPasskey | Self::SuiWallet | Self::CrazyGames | Self::Steam
        )
    }

    const fn primary_rank(self) -> u8 {
        match self {
            Self::SuiPasskey => 3,
            Self::SuiWallet => 2,
            Self::Steam => 2,
            Self::CrazyGames => 1,
            Self::CrazyGamesGuest | Self::Itch | Self::DirectGuest => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIdentityBinding {
    pub provider: ChannelIdentityProvider,
    /// SHA-256(provider || NUL || provider subject). Raw provider IDs are not persisted.
    pub subject_hash: String,
    pub created_at_ms: u64,
    pub last_seen_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerIdentityAccount {
    pub player_id: String,
    pub primary_provider: ChannelIdentityProvider,
    pub created_at_ms: u64,
    #[serde(default)]
    pub last_authenticated_provider: Option<ChannelIdentityProvider>,
    #[serde(default)]
    pub last_authenticated_at_ms: Option<u64>,
    pub identities: Vec<ChannelIdentityBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIdentityRegistryStatus {
    pub backend: String,
    pub durable: bool,
    pub account_count: usize,
    pub identity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCrazyGamesIdentity {
    pub user_id: String,
    pub game_id: String,
    pub expires_at_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSteamIdentity {
    pub steam_id: String,
    pub expires_at_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrazyGamesPublicKey {
    public_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrazyGamesTokenClaims {
    user_id: String,
    game_id: String,
    exp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelIdentityState {
    schema_version: u32,
    accounts: BTreeMap<String, PlayerIdentityAccount>,
    identity_to_player: BTreeMap<String, String>,
}

impl Default for ChannelIdentityState {
    fn default() -> Self {
        Self {
            schema_version: CHANNEL_IDENTITY_SCHEMA_VERSION,
            accounts: BTreeMap::new(),
            identity_to_player: BTreeMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct ChannelIdentityRegistry {
    state: Arc<Mutex<ChannelIdentityState>>,
    path: Option<Arc<PathBuf>>,
    postgres: Option<Arc<ChannelIdentityPostgresPool>>,
}

impl std::fmt::Debug for ChannelIdentityRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChannelIdentityRegistry")
            .field("durable", &(self.path.is_some() || self.postgres.is_some()))
            .finish_non_exhaustive()
    }
}

impl Default for ChannelIdentityRegistry {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ChannelIdentityRegistry {
    pub fn in_memory() -> Self {
        Self {
            state: Arc::new(Mutex::new(ChannelIdentityState::default())),
            path: None,
            postgres: None,
        }
    }

    pub fn from_env() -> Result<Self, String> {
        if let Some(database_url) = env::var("MIR2_CHANNEL_IDENTITY_DATABASE_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Self::from_postgres_url(&database_url);
        }
        if channel_identity_postgres_required() {
            return Err(
                "MIR2_CHANNEL_IDENTITY_DATABASE_URL is required when production PostgreSQL \
                 channel identity storage is enabled"
                    .to_string(),
            );
        }
        if let Some(path) = env::var("MIR2_CHANNEL_IDENTITY_STORE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Self::from_path(path);
        }
        if channel_identity_store_required() {
            return Err(
                "MIR2_CHANNEL_IDENTITY_DATABASE_URL is required when production channel identity \
                 storage is enabled"
                    .to_string(),
            );
        }
        Ok(Self::in_memory())
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let state = if path.exists() {
            let bytes = fs::read(&path).map_err(|error| {
                format!(
                    "channel identity store read failed at {}: {error}",
                    path.display()
                )
            })?;
            let state =
                serde_json::from_slice::<ChannelIdentityState>(&bytes).map_err(|error| {
                    format!(
                        "channel identity store decode failed at {}: {error}",
                        path.display()
                    )
                })?;
            validate_state(&state)?;
            state
        } else {
            ChannelIdentityState::default()
        };
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            path: Some(Arc::new(path)),
            postgres: None,
        })
    }

    pub fn from_postgres_url(database_url: &str) -> Result<Self, String> {
        let config = database_url
            .parse::<postgres::Config>()
            .map_err(|error| format!("channel identity postgres URL is invalid: {error}"))?;
        let manager = PostgresConnectionManager::new(config, NoTls);
        let max_size = env::var("MIR2_CHANNEL_IDENTITY_PG_POOL_MAX_SIZE")
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .unwrap_or(16)
            .clamp(2, 64);
        let pool = Pool::builder()
            .max_size(max_size)
            .build(manager)
            .map_err(|error| format!("channel identity postgres pool failed: {error}"))?;
        let mut client = pool
            .get()
            .map_err(|error| format!("channel identity postgres connect failed: {error}"))?;
        client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS mir2_player_identity_accounts (
                    player_id TEXT PRIMARY KEY,
                    primary_provider TEXT NOT NULL,
                    created_at_ms BIGINT NOT NULL,
                    last_authenticated_provider TEXT,
                    last_authenticated_at_ms BIGINT
                );
                ALTER TABLE mir2_player_identity_accounts
                    ADD COLUMN IF NOT EXISTS last_authenticated_provider TEXT;
                ALTER TABLE mir2_player_identity_accounts
                    ADD COLUMN IF NOT EXISTS last_authenticated_at_ms BIGINT;
                CREATE TABLE IF NOT EXISTS mir2_channel_identity_bindings (
                    subject_hash TEXT PRIMARY KEY,
                    provider TEXT NOT NULL,
                    player_id TEXT NOT NULL
                        REFERENCES mir2_player_identity_accounts(player_id) ON DELETE CASCADE,
                    created_at_ms BIGINT NOT NULL,
                    last_seen_at_ms BIGINT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS mir2_channel_identity_player_idx
                    ON mir2_channel_identity_bindings(player_id, created_at_ms);
                ",
            )
            .map_err(|error| format!("channel identity postgres migration failed: {error}"))?;
        drop(client);
        Ok(Self {
            state: Arc::new(Mutex::new(ChannelIdentityState::default())),
            path: None,
            postgres: Some(Arc::new(pool)),
        })
    }

    pub fn resolve_or_create(
        &self,
        provider: ChannelIdentityProvider,
        subject: &str,
    ) -> Result<PlayerIdentityAccount, String> {
        self.resolve_or_create_with_outcome(provider, subject)
            .map(|(account, _created)| account)
    }

    pub fn resolve_or_create_with_outcome(
        &self,
        provider: ChannelIdentityProvider,
        subject: &str,
    ) -> Result<(PlayerIdentityAccount, bool), String> {
        validate_subject(subject)?;
        if let Some(client) = &self.postgres {
            return postgres_resolve_or_create(client, provider, subject);
        }
        let identity_key = identity_key(provider, subject);
        let now_ms = unix_now_ms();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "channel identity registry lock was poisoned".to_string())?;

        if let Some(player_id) = state.identity_to_player.get(&identity_key).cloned() {
            let account = state
                .accounts
                .get_mut(&player_id)
                .ok_or_else(|| "channel identity registry index is inconsistent".to_string())?;
            if let Some(binding) = account
                .identities
                .iter_mut()
                .find(|binding| binding.subject_hash == identity_key)
            {
                binding.last_seen_at_ms = now_ms;
            }
            account.last_authenticated_provider = Some(provider);
            account.last_authenticated_at_ms = Some(now_ms);
            let account = account.clone();
            persist_state(self.path.as_deref(), &state)?;
            return Ok((account, false));
        }

        let player_id = new_player_id(&state);
        let account = PlayerIdentityAccount {
            player_id: player_id.clone(),
            primary_provider: provider,
            created_at_ms: now_ms,
            last_authenticated_provider: Some(provider),
            last_authenticated_at_ms: Some(now_ms),
            identities: vec![ChannelIdentityBinding {
                provider,
                subject_hash: identity_key.clone(),
                created_at_ms: now_ms,
                last_seen_at_ms: now_ms,
            }],
        };
        state
            .identity_to_player
            .insert(identity_key, player_id.clone());
        state.accounts.insert(player_id, account.clone());
        persist_state(self.path.as_deref(), &state)?;
        Ok((account, true))
    }

    pub fn link_identity(
        &self,
        player_id: &str,
        provider: ChannelIdentityProvider,
        subject: &str,
    ) -> Result<PlayerIdentityAccount, String> {
        validate_player_id(player_id)?;
        validate_subject(subject)?;
        if let Some(client) = &self.postgres {
            return postgres_link_identity(client, player_id, provider, subject);
        }
        let identity_key = identity_key(provider, subject);
        let now_ms = unix_now_ms();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "channel identity registry lock was poisoned".to_string())?;

        if let Some(existing_player_id) = state.identity_to_player.get(&identity_key) {
            if existing_player_id != player_id {
                return Err("channel identity is already linked to another player".to_string());
            }
        }

        let account = state
            .accounts
            .get_mut(player_id)
            .ok_or_else(|| "player identity account not found".to_string())?;
        if let Some(binding) = account
            .identities
            .iter_mut()
            .find(|binding| binding.subject_hash == identity_key)
        {
            binding.last_seen_at_ms = now_ms;
        } else {
            account.identities.push(ChannelIdentityBinding {
                provider,
                subject_hash: identity_key.clone(),
                created_at_ms: now_ms,
                last_seen_at_ms: now_ms,
            });
        }
        if provider.primary_rank() > account.primary_provider.primary_rank() {
            account.primary_provider = provider;
        }
        account.last_authenticated_provider = Some(provider);
        account.last_authenticated_at_ms = Some(now_ms);
        let account = account.clone();
        state
            .identity_to_player
            .insert(identity_key, player_id.to_string());
        persist_state(self.path.as_deref(), &state)?;
        Ok(account)
    }

    pub fn account(&self, player_id: &str) -> Result<Option<PlayerIdentityAccount>, String> {
        if let Some(client) = &self.postgres {
            validate_player_id(player_id)?;
            let mut client = client
                .get()
                .map_err(|error| format!("channel identity postgres checkout failed: {error}"))?;
            return postgres_load_account(&mut *client, player_id);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| "channel identity registry lock was poisoned".to_string())?;
        Ok(state.accounts.get(player_id).cloned())
    }

    pub fn status(&self) -> Result<ChannelIdentityRegistryStatus, String> {
        if let Some(client) = &self.postgres {
            let mut client = client
                .get()
                .map_err(|error| format!("channel identity postgres checkout failed: {error}"))?;
            let row = client
                .query_one(
                    "SELECT
                        (SELECT COUNT(*) FROM mir2_player_identity_accounts),
                        (SELECT COUNT(*) FROM mir2_channel_identity_bindings)",
                    &[],
                )
                .map_err(|error| format!("channel identity postgres status failed: {error}"))?;
            let account_count: i64 = row.get(0);
            let identity_count: i64 = row.get(1);
            return Ok(ChannelIdentityRegistryStatus {
                backend: "postgres".to_string(),
                durable: true,
                account_count: account_count.max(0) as usize,
                identity_count: identity_count.max(0) as usize,
            });
        }
        let state = self
            .state
            .lock()
            .map_err(|_| "channel identity registry lock was poisoned".to_string())?;
        Ok(ChannelIdentityRegistryStatus {
            backend: self
                .path
                .as_ref()
                .map(|path| format!("json:{}", path.display()))
                .unwrap_or_else(|| "memory".to_string()),
            durable: self.path.is_some(),
            account_count: state.accounts.len(),
            identity_count: state.identity_to_player.len(),
        })
    }
}

fn postgres_resolve_or_create(
    client: &Arc<ChannelIdentityPostgresPool>,
    provider: ChannelIdentityProvider,
    subject: &str,
) -> Result<(PlayerIdentityAccount, bool), String> {
    let subject_hash = identity_key(provider, subject);
    let now_ms = postgres_millis(unix_now_ms());
    let mut client = client
        .get()
        .map_err(|error| format!("channel identity postgres checkout failed: {error}"))?;
    let mut transaction = client
        .transaction()
        .map_err(|error| format!("channel identity postgres transaction failed: {error}"))?;
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtext($1)::BIGINT)",
            &[&subject_hash],
        )
        .map_err(|error| format!("channel identity postgres identity lock failed: {error}"))?;

    let existing = transaction
        .query_opt(
            "SELECT player_id
             FROM mir2_channel_identity_bindings
             WHERE subject_hash = $1",
            &[&subject_hash],
        )
        .map_err(|error| format!("channel identity postgres lookup failed: {error}"))?;
    let (player_id, created) = if let Some(row) = existing {
        let player_id: String = row.get(0);
        transaction
            .execute(
                "UPDATE mir2_channel_identity_bindings
                 SET last_seen_at_ms = $2
                 WHERE subject_hash = $1",
                &[&subject_hash, &now_ms],
            )
            .map_err(|error| format!("channel identity postgres touch failed: {error}"))?;
        transaction
            .execute(
                "UPDATE mir2_player_identity_accounts
                 SET last_authenticated_provider = $2, last_authenticated_at_ms = $3
                 WHERE player_id = $1",
                &[&player_id, &provider.as_str(), &now_ms],
            )
            .map_err(|error| {
                format!("channel identity postgres attribution update failed: {error}")
            })?;
        (player_id, false)
    } else {
        let player_id = loop {
            let candidate = new_player_id(&ChannelIdentityState::default());
            let inserted = transaction
                .execute(
                    "INSERT INTO mir2_player_identity_accounts
                        (player_id, primary_provider, created_at_ms,
                         last_authenticated_provider, last_authenticated_at_ms)
                     VALUES ($1, $2, $3, $2, $3)
                     ON CONFLICT (player_id) DO NOTHING",
                    &[&candidate, &provider.as_str(), &now_ms],
                )
                .map_err(|error| {
                    format!("channel identity postgres player insert failed: {error}")
                })?;
            if inserted == 1 {
                break candidate;
            }
        };
        transaction
            .execute(
                "INSERT INTO mir2_channel_identity_bindings
                    (subject_hash, provider, player_id, created_at_ms, last_seen_at_ms)
                 VALUES ($1, $2, $3, $4, $4)",
                &[&subject_hash, &provider.as_str(), &player_id, &now_ms],
            )
            .map_err(|error| format!("channel identity postgres binding insert failed: {error}"))?;
        (player_id, true)
    };

    let account = postgres_load_account(&mut transaction, &player_id)?
        .ok_or_else(|| "channel identity postgres account disappeared".to_string())?;
    transaction
        .commit()
        .map_err(|error| format!("channel identity postgres commit failed: {error}"))?;
    Ok((account, created))
}

fn postgres_link_identity(
    client: &Arc<ChannelIdentityPostgresPool>,
    player_id: &str,
    provider: ChannelIdentityProvider,
    subject: &str,
) -> Result<PlayerIdentityAccount, String> {
    let subject_hash = identity_key(provider, subject);
    let now_ms = postgres_millis(unix_now_ms());
    let mut client = client
        .get()
        .map_err(|error| format!("channel identity postgres checkout failed: {error}"))?;
    let mut transaction = client
        .transaction()
        .map_err(|error| format!("channel identity postgres transaction failed: {error}"))?;
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtext($1)::BIGINT)",
            &[&subject_hash],
        )
        .map_err(|error| format!("channel identity postgres identity lock failed: {error}"))?;
    if transaction
        .query_opt(
            "SELECT player_id FROM mir2_player_identity_accounts WHERE player_id = $1",
            &[&player_id],
        )
        .map_err(|error| format!("channel identity postgres player lookup failed: {error}"))?
        .is_none()
    {
        return Err("player identity account not found".to_string());
    }
    if let Some(row) = transaction
        .query_opt(
            "SELECT player_id
             FROM mir2_channel_identity_bindings
             WHERE subject_hash = $1",
            &[&subject_hash],
        )
        .map_err(|error| format!("channel identity postgres binding lookup failed: {error}"))?
    {
        let existing_player_id: String = row.get(0);
        if existing_player_id != player_id {
            return Err("channel identity is already linked to another player".to_string());
        }
        transaction
            .execute(
                "UPDATE mir2_channel_identity_bindings
                 SET last_seen_at_ms = $2
                 WHERE subject_hash = $1",
                &[&subject_hash, &now_ms],
            )
            .map_err(|error| format!("channel identity postgres touch failed: {error}"))?;
    } else {
        transaction
            .execute(
                "INSERT INTO mir2_channel_identity_bindings
                    (subject_hash, provider, player_id, created_at_ms, last_seen_at_ms)
                 VALUES ($1, $2, $3, $4, $4)",
                &[&subject_hash, &provider.as_str(), &player_id, &now_ms],
            )
            .map_err(|error| format!("channel identity postgres binding insert failed: {error}"))?;
    }
    transaction
        .execute(
            "UPDATE mir2_player_identity_accounts
             SET primary_provider = CASE
                     WHEN $2 = 'suiPasskey' THEN $2
                     WHEN $2 = 'suiWallet'
                          AND primary_provider <> 'suiPasskey' THEN $2
                     WHEN $2 = 'crazyGames'
                          AND primary_provider IN ('crazyGamesGuest', 'itch', 'directGuest') THEN $2
                     ELSE primary_provider
                 END,
                 last_authenticated_provider = $2,
                 last_authenticated_at_ms = $3
             WHERE player_id = $1",
            &[&player_id, &provider.as_str(), &now_ms],
        )
        .map_err(|error| format!("channel identity postgres primary update failed: {error}"))?;
    let account = postgres_load_account(&mut transaction, player_id)?
        .ok_or_else(|| "channel identity postgres account disappeared".to_string())?;
    transaction
        .commit()
        .map_err(|error| format!("channel identity postgres commit failed: {error}"))?;
    Ok(account)
}

fn postgres_load_account(
    client: &mut impl GenericClient,
    player_id: &str,
) -> Result<Option<PlayerIdentityAccount>, String> {
    let Some(account_row) = client
        .query_opt(
            "SELECT primary_provider, created_at_ms,
                    last_authenticated_provider, last_authenticated_at_ms
             FROM mir2_player_identity_accounts
             WHERE player_id = $1",
            &[&player_id],
        )
        .map_err(|error| format!("channel identity postgres account load failed: {error}"))?
    else {
        return Ok(None);
    };
    let primary_provider_name: String = account_row.get(0);
    let created_at_ms: i64 = account_row.get(1);
    let last_authenticated_provider_name: Option<String> = account_row.get(2);
    let last_authenticated_at_ms: Option<i64> = account_row.get(3);
    let primary_provider = ChannelIdentityProvider::parse(&primary_provider_name)?;
    let bindings = client
        .query(
            "SELECT provider, subject_hash, created_at_ms, last_seen_at_ms
             FROM mir2_channel_identity_bindings
             WHERE player_id = $1
             ORDER BY created_at_ms, subject_hash",
            &[&player_id],
        )
        .map_err(|error| format!("channel identity postgres bindings load failed: {error}"))?
        .into_iter()
        .map(|row| {
            let provider_name: String = row.get(0);
            let created_at_ms: i64 = row.get(2);
            let last_seen_at_ms: i64 = row.get(3);
            Ok(ChannelIdentityBinding {
                provider: ChannelIdentityProvider::parse(&provider_name)?,
                subject_hash: row.get(1),
                created_at_ms: created_at_ms.max(0) as u64,
                last_seen_at_ms: last_seen_at_ms.max(0) as u64,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Some(PlayerIdentityAccount {
        player_id: player_id.to_string(),
        primary_provider,
        created_at_ms: created_at_ms.max(0) as u64,
        last_authenticated_provider: last_authenticated_provider_name
            .as_deref()
            .map(ChannelIdentityProvider::parse)
            .transpose()?,
        last_authenticated_at_ms: last_authenticated_at_ms.map(|value| value.max(0) as u64),
        identities: bindings,
    }))
}

fn postgres_millis(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

pub async fn verify_crazygames_token(token: &str) -> Result<VerifiedCrazyGamesIdentity, String> {
    if token.trim().is_empty() || token.len() > 16 * 1024 {
        return Err("invalid CrazyGames token".to_string());
    }
    let public_key = crazygames_public_key().await?;
    verify_crazygames_token_with_public_key(token, &public_key)
}

async fn crazygames_public_key() -> Result<String, String> {
    let cache = CRAZYGAMES_PUBLIC_KEY_CACHE.get_or_init(|| tokio::sync::Mutex::new(None));
    let mut cached = cache.lock().await;
    if let Some(entry) = cached.as_ref() {
        if entry.fetched_at.elapsed() < CRAZYGAMES_PUBLIC_KEY_TTL {
            return Ok(entry.public_key.clone());
        }
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|error| format!("CrazyGames verifier client failed: {error}"))?
        .get(CRAZYGAMES_PUBLIC_KEY_URL)
        .send()
        .await
        .map_err(|error| format!("CrazyGames public key fetch failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("CrazyGames public key fetch failed: {error}"))?;
    let public_key = response
        .json::<CrazyGamesPublicKey>()
        .await
        .map_err(|error| format!("CrazyGames public key decode failed: {error}"))?;
    if public_key.public_key.trim().is_empty() {
        return Err("CrazyGames public key response was empty".to_string());
    }
    *cached = Some(CrazyGamesPublicKeyCache {
        public_key: public_key.public_key.clone(),
        fetched_at: Instant::now(),
    });
    Ok(public_key.public_key)
}

fn verify_crazygames_token_with_public_key(
    token: &str,
    public_key: &str,
) -> Result<VerifiedCrazyGamesIdentity, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|error| format!("invalid CrazyGames public key: {error}"))?;
    let validation = Validation::new(Algorithm::RS256);
    let claims = decode::<CrazyGamesTokenClaims>(token, &decoding_key, &validation)
        .map_err(|error| format!("invalid CrazyGames token: {error}"))?
        .claims;
    if claims.user_id.trim().is_empty() || claims.game_id.trim().is_empty() {
        return Err("invalid CrazyGames token identity".to_string());
    }
    if let Some(expected_game_id) = env::var("MIR2_CRAZYGAMES_GAME_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        if claims.game_id != expected_game_id {
            return Err("CrazyGames token belongs to another game".to_string());
        }
    } else if channel_identity_store_required() || channel_identity_postgres_required() {
        return Err(
            "MIR2_CRAZYGAMES_GAME_ID is required when durable channel identity storage is enabled"
                .to_string(),
        );
    }
    Ok(VerifiedCrazyGamesIdentity {
        user_id: claims.user_id,
        game_id: claims.game_id,
        expires_at_seconds: claims.exp,
    })
}

/// Verify a Steam auth session ticket via the Steam Web API.
///
/// The client submits the raw ticket from Steamworks `GetAuthTicketForWebApi`.
/// The server calls `ISteamUserAuth/AuthenticateUserTicket` with the publisher
/// web API key; Steam returns the authoritative SteamID (as 64-bit, then
/// converted to the 17-digit string) only when the ticket is valid for this
/// app. A client-claimed SteamID is never trusted.
///
/// Requires `MIR2_STEAM_PUBLISHER_WEB_API_KEY` and `MIR2_STEAM_APP_ID`.
pub async fn verify_steam_ticket(ticket: &str) -> Result<VerifiedSteamIdentity, String> {
    if ticket.trim().is_empty() || ticket.len() > 4096 {
        return Err("invalid Steam auth ticket".to_string());
    }
    let api_key = env::var("MIR2_STEAM_PUBLISHER_WEB_API_KEY")
        .map_err(|_| "MIR2_STEAM_PUBLISHER_WEB_API_KEY is required".to_string())?;
    let app_id =
        env::var("MIR2_STEAM_APP_ID").map_err(|_| "MIR2_STEAM_APP_ID is required".to_string())?;

    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|error| format!("Steam verifier client failed: {error}"))?
        .post(STEAM_AUTH_USER_TICKET_URL)
        .form(&[
            ("key", api_key.as_str()),
            ("appid", app_id.as_str()),
            ("ticket", ticket),
            ("identity", "1"),
        ])
        .send()
        .await
        .map_err(|error| format!("Steam ticket verification failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Steam ticket verification rejected: {error}"))?;

    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("Steam ticket verification decode failed: {error}"))?;
    let params = body
        .get("response")
        .and_then(|response| response.get("params"))
        .ok_or_else(|| "Steam ticket verification returned no params".to_string())?;

    let steam_id: String = params
        .get("steamid")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            params
                .get("steamid")
                .and_then(serde_json::Value::as_u64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| "Steam ticket verification returned no steamid".to_string())?;
    let expires_at_seconds = params
        .get("expires")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    Ok(VerifiedSteamIdentity {
        steam_id,
        expires_at_seconds,
    })
}

fn channel_identity_store_required() -> bool {
    env::var("MIR2_REQUIRE_CHANNEL_IDENTITY_STORE")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn channel_identity_postgres_required() -> bool {
    env::var("MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn validate_subject(subject: &str) -> Result<(), String> {
    if subject.trim().is_empty() {
        return Err("channel identity subject is required".to_string());
    }
    if subject.len() > MAX_PROVIDER_SUBJECT_BYTES {
        return Err("channel identity subject is too large".to_string());
    }
    Ok(())
}

fn validate_player_id(player_id: &str) -> Result<(), String> {
    if !player_id.starts_with("obl_")
        || player_id.len() != 36
        || !player_id[4..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid Obelisk player id".to_string());
    }
    Ok(())
}

fn identity_key(provider: ChannelIdentityProvider, subject: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(provider.as_str().as_bytes());
    hash.update([0]);
    hash.update(subject.trim().as_bytes());
    hex_lower(&hash.finalize())
}

fn new_player_id(state: &ChannelIdentityState) -> String {
    loop {
        let mut bytes = [0_u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let player_id = format!("obl_{}", hex_lower(&bytes));
        if !state.accounts.contains_key(&player_id) {
            return player_id;
        }
    }
}

fn validate_state(state: &ChannelIdentityState) -> Result<(), String> {
    if state.schema_version != CHANNEL_IDENTITY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported channel identity store schema version {}",
            state.schema_version
        ));
    }
    for (identity, player_id) in &state.identity_to_player {
        let account = state
            .accounts
            .get(player_id)
            .ok_or_else(|| format!("identity {identity} references missing player {player_id}"))?;
        if !account
            .identities
            .iter()
            .any(|binding| &binding.subject_hash == identity)
        {
            return Err(format!(
                "identity {identity} is absent from player {player_id} bindings"
            ));
        }
    }
    Ok(())
}

fn persist_state(path: Option<&PathBuf>, state: &ChannelIdentityState) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "channel identity store directory create failed at {}: {error}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("channel identity store encode failed: {error}"))?;
    let temporary = temporary_path(path);
    fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "channel identity store temporary write failed at {}: {error}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!(
            "channel identity store atomic replace failed at {}: {error}",
            path.display()
        )
    })
}

fn temporary_path(path: &Path) -> PathBuf {
    let suffix = format!("tmp-{}-{}", std::process::id(), unix_now_ms());
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("channel-identities.json");
    path.with_file_name(format!("{file_name}.{suffix}"))
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{ChannelIdentityProvider, ChannelIdentityRegistry};

    #[test]
    fn same_provider_subject_resolves_to_stable_player_without_persisting_raw_subject() {
        let registry = ChannelIdentityRegistry::in_memory();
        let first = registry
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, "sui:0xpasskey")
            .expect("first passkey identity should resolve");
        let second = registry
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, "sui:0xpasskey")
            .expect("second passkey identity should resolve");

        assert_eq!(first.player_id, second.player_id);
        assert!(first.player_id.starts_with("obl_"));
        assert_eq!(first.identities.len(), 1);
        assert_ne!(first.identities[0].subject_hash, "sui:0xpasskey");
    }

    #[test]
    fn identity_link_rejects_cross_player_takeover() {
        let registry = ChannelIdentityRegistry::in_memory();
        let passkey = registry
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, "sui:0xpasskey")
            .expect("passkey identity should resolve");
        let crazy = registry
            .resolve_or_create(ChannelIdentityProvider::CrazyGames, "crazy-user-1")
            .expect("CrazyGames identity should resolve");

        let error = registry
            .link_identity(
                &passkey.player_id,
                ChannelIdentityProvider::CrazyGames,
                "crazy-user-1",
            )
            .expect_err("an identity already owned by another player must not move");
        assert!(error.contains("already linked"));
        assert_ne!(passkey.player_id, crazy.player_id);
    }

    #[test]
    fn linking_passkey_promotes_guest_account_primary() {
        let registry = ChannelIdentityRegistry::in_memory();
        let guest = registry
            .resolve_or_create(ChannelIdentityProvider::Itch, "guest:itch-1")
            .expect("itch guest should resolve");
        let linked = registry
            .link_identity(
                &guest.player_id,
                ChannelIdentityProvider::SuiPasskey,
                "sui:0xpasskey-primary",
            )
            .expect("passkey should link");

        assert_eq!(linked.primary_provider, ChannelIdentityProvider::SuiPasskey);
        assert_eq!(
            linked.last_authenticated_provider,
            Some(ChannelIdentityProvider::SuiPasskey)
        );
        assert_eq!(linked.identities.len(), 2);
    }

    #[test]
    fn json_store_roundtrips_identity_index() {
        let path = std::env::temp_dir().join(format!(
            "mir2-channel-identities-{}-{}.json",
            std::process::id(),
            super::unix_now_ms()
        ));
        let first = ChannelIdentityRegistry::from_path(&path).expect("store should initialize");
        let account = first
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, "sui:0xpersisted")
            .expect("identity should persist");
        drop(first);

        let reopened = ChannelIdentityRegistry::from_path(&path).expect("store should reopen");
        let restored = reopened
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, "sui:0xpersisted")
            .expect("identity should restore");
        assert_eq!(restored.player_id, account.player_id);
        assert!(reopened.status().expect("status").durable);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    #[ignore = "requires MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL"]
    fn postgres_store_is_shared_across_gateway_instances() {
        let database_url = std::env::var("MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL")
            .expect("MIR2_CHANNEL_IDENTITY_TEST_DATABASE_URL is required");
        let first = ChannelIdentityRegistry::from_postgres_url(&database_url)
            .expect("first postgres registry should initialize");
        let second = ChannelIdentityRegistry::from_postgres_url(&database_url)
            .expect("second postgres registry should initialize");
        let unique = format!("{}-{}", std::process::id(), super::unix_now_ms());
        let subject = format!("sui:0xpostgres-{unique}");
        let wallet_subject = format!("sui:0xwallet-{unique}");

        let created = first
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, &subject)
            .expect("first gateway should create player");
        let resolved = second
            .resolve_or_create(ChannelIdentityProvider::SuiPasskey, &subject)
            .expect("second gateway should resolve the same player");
        assert_eq!(created.player_id, resolved.player_id);

        let linked = second
            .link_identity(
                &created.player_id,
                ChannelIdentityProvider::SuiWallet,
                &wallet_subject,
            )
            .expect("second gateway should link wallet");
        assert_eq!(linked.identities.len(), 2);
        assert_eq!(linked.primary_provider, ChannelIdentityProvider::SuiPasskey);
        assert_eq!(
            linked.last_authenticated_provider,
            Some(ChannelIdentityProvider::SuiWallet)
        );
        assert!(first
            .account(&created.player_id)
            .expect("first gateway should query player")
            .is_some_and(|account| account.identities.len() == 2));

        let guest = first
            .resolve_or_create(
                ChannelIdentityProvider::Itch,
                &format!("guest:postgres-{unique}"),
            )
            .expect("guest should resolve");
        let promoted = second
            .link_identity(
                &guest.player_id,
                ChannelIdentityProvider::SuiPasskey,
                &format!("sui:0xpromoted-{unique}"),
            )
            .expect("passkey should promote guest account");
        assert_eq!(
            promoted.primary_provider,
            ChannelIdentityProvider::SuiPasskey
        );

        let mut cleanup = postgres::Client::connect(&database_url, postgres::NoTls)
            .expect("cleanup postgres should connect");
        cleanup
            .execute(
                "DELETE FROM mir2_player_identity_accounts WHERE player_id IN ($1, $2)",
                &[&created.player_id, &guest.player_id],
            )
            .expect("test player should clean up");
    }

    #[test]
    fn steam_provider_parses_and_round_trips() {
        assert_eq!(
            ChannelIdentityProvider::parse("steam"),
            Ok(ChannelIdentityProvider::Steam)
        );
        assert_eq!(
            ChannelIdentityProvider::parse("SteamWorks"),
            Ok(ChannelIdentityProvider::Steam)
        );
        assert_eq!(ChannelIdentityProvider::Steam.as_str(), "steam");
        assert!(ChannelIdentityProvider::Steam.is_primary_capable());
    }

    #[test]
    fn steam_provider_resolves_stable_player_and_links() {
        let registry = ChannelIdentityRegistry::in_memory();
        let steam = registry
            .resolve_or_create(ChannelIdentityProvider::Steam, "76561198000000001")
            .expect("Steam identity should resolve");
        let again = registry
            .resolve_or_create(ChannelIdentityProvider::Steam, "76561198000000001")
            .expect("second Steam identity should resolve");
        assert_eq!(steam.player_id, again.player_id);
        assert_eq!(steam.identities.len(), 1);
        assert_ne!(steam.identities[0].subject_hash, "76561198000000001");

        // Link a fresh passkey subject (never created independently) to the
        // Steam account; the account's primary becomes passkey.
        let linked = registry
            .link_identity(
                &steam.player_id,
                ChannelIdentityProvider::SuiPasskey,
                "sui:0xsteam-user",
            )
            .expect("passkey should link to Steam account");
        assert_eq!(linked.primary_provider, ChannelIdentityProvider::SuiPasskey);
        assert_eq!(linked.identities.len(), 2);
    }
}
