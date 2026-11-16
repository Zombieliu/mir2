//! Explicit offline acceptance of a local development File image after a crash.
//! Never rewrites account bytes or replays character journals itself.
use mir2_simulation::{
    account_store_requires_postgres_source_from_env, AccountStore, AccountStoreRepository,
    FileAccountStoreRepository, SimulationConfig,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const PENDING: &[u8] = b"PENDING PUBLICATION\n";
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
fn regular(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("source must be a regular file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("reparse points are not accepted".into());
        }
    }
    Ok(())
}
fn read_bounded(file: &mut File) -> Result<Vec<u8>, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("source exceeds audit limit".into());
    }
    Ok(bytes)
}
fn durable_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x8000_0000); // FILE_FLAG_WRITE_THROUGH
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

fn reconcile(path: &Path, expected: &str, accept: bool) -> Result<serde_json::Value, String> {
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("an exact SHA-256 of the reviewed source is required".into());
    }
    regular(path)?;
    let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let guard_path = sidecar(&path, ".reconciliation.pending");
    if guard_path.exists() {
        return Err(
            "an interrupted reconciliation requires its own audit; do not start the server".into(),
        );
    }
    if sidecar(&path, ".item-identity.json").exists() {
        return Err(
            "item-identity publications require separate multi-image reconciliation".into(),
        );
    }
    let marker_path = sidecar(&path, ".writer.lock");
    regular(&marker_path)?;
    let mut marker = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&marker_path)
        .map_err(|e| e.to_string())?;
    marker
        .try_lock()
        .map_err(|_| "stop the File authority before reconciling".to_string())?;
    let marker_bytes = read_bounded(&mut marker)?;
    if marker_bytes != PENDING {
        return Err("only an exact pending File publication may be reviewed here".into());
    }
    let mut source = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let bytes = read_bounded(&mut source)?;
    let actual = hash(&bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("source hash changed; fence retained".into());
    }
    let raw: AccountStore =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid account image: {e}"))?;
    let validated = FileAccountStoreRepository::new(&path)
        .load(SimulationConfig::default().default_character)?;
    if raw.schema_version != validated.schema_version {
        return Err("legacy images need migration review first".into());
    }
    if raw.accounts.is_empty() {
        return Err("empty account image requires separate review".into());
    }
    let mut report = serde_json::json!({"sourceSha256":actual,"accountCount":raw.accounts.len(),"characterCount":raw.accounts.values().map(|a|a.characters.len()).sum::<usize>(),"accepted":false});
    if !accept {
        return Ok(report);
    }
    // Operator accepts these exact complete bytes, not an inferred transaction
    // outcome. Retain an immutable copy before clearing the publication fence.
    let suffix = format!(
        ".reconcile-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let backup = sidecar(&path, &format!("{suffix}.accounts.json"));
    let marker_backup = sidecar(&path, &format!("{suffix}.marker"));
    let decision = sidecar(&path, &format!("{suffix}.decision.json"));
    durable_new(&backup, &bytes)?;
    durable_new(&marker_backup, &marker_bytes)?;
    let decision_bytes = serde_json::to_vec_pretty(&serde_json::json!({"decision":"accept-current-file-image","sourceSha256":actual,"markerSha256":hash(&marker_bytes),"accountBackup":backup,"markerBackup":marker_backup})).map_err(|e| e.to_string())?;
    durable_new(&decision, &decision_bytes)?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    if fs::read(&path).map_err(|e| e.to_string())? != bytes
        || read_bounded(&mut marker)? != marker_bytes
    {
        return Err("source or marker changed during review; fence retained".into());
    }
    source.sync_all().map_err(|e| e.to_string())?;
    // Independent durable guard: a crash or failed sync after truncation must
    // not make the normal authority mistake an empty writer marker for success.
    durable_new(&guard_path, &decision_bytes)?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    if let Err(error) = marker.set_len(0).and_then(|_| marker.sync_all()) {
        let restored = marker
            .seek(SeekFrom::Start(0))
            .and_then(|_| marker.write_all(PENDING))
            .and_then(|_| marker.sync_all());
        return Err(format!("fence settlement failed: {error}; restoration={restored:?}; independent reconciliation guard retained; do not start the server"));
    }
    fs::remove_file(&guard_path).map_err(|e| {
        format!(
            "marker settled but reconciliation guard removal failed: {e}; do not start the server"
        )
    })?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("missing parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    report["accepted"] = true.into();
    report["decisionFile"] = decision.to_string_lossy().into_owned().into();
    Ok(report)
}

fn main() {
    let result = (|| {
        if std::env::var("MIR2_ACCOUNT_STORE_BACKEND").ok().as_deref() != Some("file")
            || std::env::var("MIR2_RUNTIME_ENV").ok().as_deref() != Some("development")
            || std::env::var("MIR2_ACCOUNT_STORE_DATABASE_URL").is_ok()
            || account_store_requires_postgres_source_from_env()
        {
            return Err("this tool requires explicit development File-only configuration; PostgreSQL/Mirror are unsupported".into());
        }
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        if !(args.len() == 2 || (args.len() == 3 && args[2] == "--accept-current-file-image")) {
            return Err("usage: file_store_reconcile <accounts.json> <reviewed-sha256> [--accept-current-file-image]".into());
        }
        reconcile(Path::new(&args[0]), &args[1], args.len() == 3)
    })();
    match result {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Vec<u8>) {
        let root = std::env::temp_dir().join(format!(
            "mir2-reconcile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("accounts.json");
        let config = SimulationConfig::default();
        let bytes = serde_json::to_vec(&*config.account_store.lock().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
        fs::write(sidecar(&path, ".writer.lock"), PENDING).unwrap();
        (path, bytes)
    }
    #[test]
    fn reviewed_image_is_backed_up_and_fence_settled_without_changing_accounts() {
        let (path, bytes) = fixture();
        assert_eq!(
            reconcile(&path, &hash(&bytes), false).unwrap()["accepted"],
            false
        );
        assert_eq!(fs::read(sidecar(&path, ".writer.lock")).unwrap(), PENDING);
        let result = reconcile(&path, &hash(&bytes), true).unwrap();
        assert_eq!(result["accepted"], true);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(fs::read(sidecar(&path, ".writer.lock")).unwrap().is_empty());
        assert!(Path::new(result["decisionFile"].as_str().unwrap()).exists());
    }
    #[test]
    fn changed_or_invalid_source_preserves_fence() {
        let (path, bytes) = fixture();
        assert!(reconcile(&path, &"0".repeat(64), true).is_err());
        fs::write(&path, b"invalid").unwrap();
        assert!(reconcile(&path, &hash(b"invalid"), true).is_err());
        assert_eq!(fs::read(sidecar(&path, ".writer.lock")).unwrap(), PENDING);
        assert!(!bytes.is_empty());
    }
    #[test]
    fn live_authority_and_nonpending_fences_are_rejected() {
        let (path, bytes) = fixture();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(sidecar(&path, ".writer.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(reconcile(&path, &hash(&bytes), true).is_err());
        drop(lock);
        fs::write(sidecar(&path, ".writer.lock"), b"FROZEN: unknown").unwrap();
        assert!(reconcile(&path, &hash(&bytes), true).is_err());
        assert_eq!(
            fs::read(sidecar(&path, ".writer.lock")).unwrap(),
            b"FROZEN: unknown"
        );
    }
    #[test]
    fn interrupted_reconciliation_guard_refuses_even_a_clean_marker() {
        let (path, bytes) = fixture();
        fs::write(sidecar(&path, ".writer.lock"), b"").unwrap();
        fs::write(
            sidecar(&path, ".reconciliation.pending"),
            b"pending operator decision",
        )
        .unwrap();
        assert!(reconcile(&path, &hash(&bytes), true)
            .unwrap_err()
            .contains("interrupted reconciliation"));
        assert!(sidecar(&path, ".reconciliation.pending").exists());
    }
}
