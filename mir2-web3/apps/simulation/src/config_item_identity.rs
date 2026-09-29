//! Unregistered File/Mirror reservation candidate. Owns no rollbackable cursor.
//! `.item-identity.json` is a REQUIRED durable backup component after activation.
//! Missing/corrupt state blocks allocation. Account snapshot restoration cannot
//! reset its high watermark. Initial bootstrap is controlled, never automatic.

use super::item_identity_postgres::{self as pg, CompleteIdentityCensus, IdentitySourceState};
use super::item_identity_reservation::{CommittedItemIds, ReservationPlan};
use super::*;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FileIdentity {
    schema_version: u8,
    // Decimal text avoids rounding by JSON tools and retains all u64 bits.
    high_watermark: String,
    source_version: i64,
    census_sha256: String,
}

fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn read(path: &Path) -> Result<IdentitySourceState, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("item identity authority unavailable; allocation blocked: {e}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8192 {
        return Err("item identity authority is not a bounded regular file".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let record: FileIdentity = serde_json::from_slice(&bytes).map_err(|e| {
        format!("item identity authority invalid; preserve source and reconcile: {e}")
    })?;
    let high_watermark = record
        .high_watermark
        .parse::<u64>()
        .map_err(|_| "invalid item identity high watermark")?;
    if record.schema_version != 1
        || record.source_version <= 0
        || !valid_digest(&record.census_sha256)
        || record.high_watermark != high_watermark.to_string()
    {
        return Err("invalid item identity authority state".into());
    }
    Ok(IdentitySourceState {
        high_watermark,
        version: record.source_version,
        bootstrapped: true,
        census_sha256: Some(record.census_sha256),
    })
}

fn encode(state: &IdentitySourceState) -> Result<Vec<u8>, String> {
    let digest = state
        .census_sha256
        .as_deref()
        .filter(|s| valid_digest(s))
        .ok_or("invalid item identity census digest")?;
    if !state.bootstrapped || state.version <= 0 {
        return Err("item identity not bootstrapped".into());
    }
    serde_json::to_vec(&FileIdentity {
        schema_version: 1,
        high_watermark: state.high_watermark.to_string(),
        source_version: state.version,
        census_sha256: digest.into(),
    })
    .map_err(|e| e.to_string())
}

fn path(config: &SimulationConfig) -> Result<PathBuf, String> {
    config.ensure_file_writer_binding()?;
    config.ensure_account_store_writable()?;
    config
        .file_authority
        .as_ref()
        .ok_or("item identity requires File authority")?
        .item_identity_path(config)
}

/// Read-only status; allocation always re-reads under the canonical persist lock.
pub(super) fn load(config: &SimulationConfig) -> Result<IdentitySourceState, String> {
    let _lock = config
        .account_store_persist_lock
        .lock()
        .map_err(|_| "persist lock poisoned")?;
    read(&path(config)?)
}

/// Initial maintenance bootstrap only. There is intentionally no public/serde
/// constructor for CompleteIdentityCensus. Missing files during normal reserve
/// never call this method. A restore without the sidecar remains blocked.
pub(super) fn bootstrap_file(
    config: &SimulationConfig,
    census: CompleteIdentityCensus,
) -> Result<(), String> {
    if config.account_store_database_url.is_some() {
        return Err("File bootstrap cannot bypass configured PostgreSQL authority".into());
    }
    let _lock = config
        .account_store_persist_lock
        .lock()
        .map_err(|_| "persist lock poisoned")?;
    let path = path(config)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err("item identity bootstrap cannot replace an existing authority".into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    let (high, digest) = census.into_parts();
    let bytes = encode(&IdentitySourceState {
        high_watermark: high,
        version: 1,
        bootstrapped: true,
        census_sha256: Some(digest),
    })?;
    let guard = config
        .begin_file_publication()?
        .ok_or("missing File publication authority")?;
    if let Err(e) = write_file_atomically(&path, &bytes, None) {
        return Err(config.freeze_account_store_writes(format!(
            "item identity bootstrap publication failed: {e}"
        )));
    }
    guard.settle()
}

pub(super) fn reserve_file(
    config: &SimulationConfig,
    expected: &IdentitySourceState,
    count: u32,
) -> Result<CommittedItemIds, String> {
    if config.account_store_database_url.is_some() {
        return Err("File mint cannot bypass configured PostgreSQL authority".into());
    }
    reserve(config, expected, count, None, None)
}

/// Maintenance-only installation of the Mirror sidecar AFTER the controlled PG
/// bootstrap. This does not activate PG and cannot replace an existing sidecar.
pub(super) fn bootstrap_mirror(
    config: &SimulationConfig,
    census: CompleteIdentityCensus,
) -> Result<(), String> {
    let mut client = mirror_client(config)?;
    bootstrap_mirror_with_client(config, census, &mut client)
}

fn bootstrap_mirror_with_client(
    config: &SimulationConfig,
    census: CompleteIdentityCensus,
    client: &mut postgres::Client,
) -> Result<(), String> {
    let _lock = config
        .account_store_persist_lock
        .lock()
        .map_err(|_| "persist lock poisoned")?;
    let path = path(config)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => return Err("Mirror bootstrap cannot replace an existing item authority".into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    let (floor, digest) = census.into_parts();
    let source =
        pg::load(client).map_err(|e| format!("item identity PG bootstrap source: {e:?}"))?;
    if !source.bootstrapped
        || source.high_watermark < floor
        || source.census_sha256.as_deref() != Some(digest.as_str())
    {
        return Err("Mirror bootstrap requires matching complete PG census authority".into());
    }
    let bytes = encode(&source)?;
    let guard = config
        .begin_file_publication()?
        .ok_or("missing File publication authority")?;
    if let Err(error) = write_file_atomically(&path, &bytes, None) {
        return Err(config.freeze_account_store_writes(format!(
            "Mirror item bootstrap publication failed: {error}"
        )));
    }
    guard.settle()
}

fn mirror_client(config: &SimulationConfig) -> Result<postgres::Client, String> {
    if config.account_store_database_mode != AccountStoreDatabaseMode::Mirror {
        return Err("item identity mirror requires configured Mirror mode".into());
    }
    let url = config
        .account_store_database_url
        .as_deref()
        .ok_or("missing item identity PostgreSQL source")?;
    validate_postgres_account_store_database_url(url)?;
    postgres::Client::connect(url, postgres::NoTls)
        .map_err(|e| format!("item identity source connection failed: {e}"))
}

/// PG is the mint source, File is its monotonic mirror. A PG success followed by
/// File failure burns the range and fences this File authority. Never compensate
/// PG downward or reconstruct the burned cursor on retry.
pub(super) fn reserve_mirror(
    config: &SimulationConfig,
    expected: &IdentitySourceState,
    count: u32,
) -> Result<CommittedItemIds, String> {
    let mut client = mirror_client(config)?;
    reserve(config, expected, count, Some(&mut client), None)
}

fn reserve(
    config: &SimulationConfig,
    expected: &IdentitySourceState,
    count: u32,
    pg_client: Option<&mut postgres::Client>,
    fault: Option<AccountStoreFileCommitFault>,
) -> Result<CommittedItemIds, String> {
    let _lock = config
        .account_store_persist_lock
        .lock()
        .map_err(|_| "persist lock poisoned")?;
    let path = path(config)?;
    let before = read(&path)?;
    if &before != expected {
        return Err("stale item identity File source".into());
    }
    let (mut pg_client, source) = match pg_client {
        Some(client) => {
            let source = pg::load(client).map_err(|e| format!("item identity PG source: {e:?}"))?;
            if !source.bootstrapped
                || source.census_sha256 != before.census_sha256
                || source.high_watermark < before.high_watermark
                || source.version < before.version
            {
                return Err(
                    "item identity Mirror source regression/mismatch; reconciliation required"
                        .into(),
                );
            }
            (Some(client), source)
        }
        None => (
            None,
            IdentitySourceState {
                high_watermark: before.high_watermark,
                version: before.version,
                bootstrapped: true,
                census_sha256: before.census_sha256.clone(),
            },
        ),
    };
    let plan = ReservationPlan::prepare(source.version, source.high_watermark, count)
        .map_err(|e| format!("item identity reservation: {e:?}"))?;
    let (version, _, high) = plan.source_cas();
    let after = IdentitySourceState {
        high_watermark: high,
        version: version + 1,
        bootstrapped: true,
        census_sha256: source.census_sha256.clone(),
    };
    let bytes = encode(&after)?;
    let guard = config
        .begin_file_publication()?
        .ok_or("missing File publication authority")?;
    let pg_cursor = if let Some(client) = pg_client.as_mut() {
        // Even a definite PG refusal keeps the fence conservatively; an
        // operator can reconcile it. No ordinary automatic settlement guessed.
        Some(pg::reserve(client, &source, count).map_err(|e| {
            config.freeze_account_store_writes(format!(
                "item identity PG reservation unresolved: {e:?}"
            ))
        })?)
    } else {
        None
    };
    if let Err(error) = write_file_atomically(&path, &bytes, fault) {
        return Err(config.freeze_account_store_writes(format!(
            "item identity File publication failed; committed ranges retired: {error}"
        )));
    }
    guard.settle()?;
    match pg_cursor {
        Some(cursor) => Ok(cursor),
        None => CommittedItemIds::from_committed_source(plan, after.version, after.high_watermark)
            .map_err(|e| format!("item identity receipt mismatch: {e:?}")),
    }
}

#[cfg(test)]
#[path = "config_item_identity_tests.rs"]
mod tests;
