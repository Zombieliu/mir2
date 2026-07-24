use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{ZoneBaseSnapshot, ZoneMutationBatch, ZoneReplicationCoverage};

const ZONE_MUTATION_WAL_VERSION: u32 = 1;
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_ZONE_BASE_SNAPSHOT_FILE_BYTES: u64 = 80 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneMutationWalAck {
    pub zone_id: String,
    pub build_id: String,
    pub next_sequence: u64,
    pub latest_digest: String,
    pub durable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoneMutationWalRecord {
    version: u32,
    zone_id: String,
    build_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    batch: Option<ZoneMutationBatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<ZoneMutationWalAnchor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZoneMutationWalAnchor {
    snapshot_id: String,
    next_sequence: u64,
    latest_digest: String,
}

pub struct ZoneMutationWal {
    path: PathBuf,
    file: File,
    zone_id: String,
    build_id: String,
    next_sequence: u64,
    latest_digest: String,
}

#[derive(Debug, Clone)]
pub struct ZoneBaseSnapshotStore {
    path: PathBuf,
    zone_id: String,
    build_id: String,
}

impl ZoneBaseSnapshotStore {
    pub fn new(
        path: impl AsRef<Path>,
        zone_id: impl Into<String>,
        build_id: impl Into<String>,
    ) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let zone_id = zone_id.into();
        let build_id = build_id.into();
        validate_wal_identifier("Zone id", &zone_id)?;
        validate_wal_identifier("build id", &build_id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Zone base snapshot directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        Ok(Self {
            path,
            zone_id,
            build_id,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<ZoneBaseSnapshot>, String> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "failed to open Zone base snapshot {}: {error}",
                    self.path.display()
                ));
            }
        };
        let file_bytes = file
            .metadata()
            .map_err(|error| {
                format!(
                    "failed to inspect Zone base snapshot {}: {error}",
                    self.path.display()
                )
            })?
            .len();
        if file_bytes == 0 || file_bytes > MAX_ZONE_BASE_SNAPSHOT_FILE_BYTES {
            return Err(format!(
                "Zone base snapshot {} has invalid file size {file_bytes}",
                self.path.display()
            ));
        }
        let mut bytes = Vec::with_capacity(usize::try_from(file_bytes).unwrap_or_default());
        file.read_to_end(&mut bytes).map_err(|error| {
            format!(
                "failed to read Zone base snapshot {}: {error}",
                self.path.display()
            )
        })?;
        let snapshot: ZoneBaseSnapshot = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to decode Zone base snapshot {}: {error}",
                self.path.display()
            )
        })?;
        self.validate_identity(&snapshot)?;
        snapshot.verify()?;
        Ok(Some(snapshot))
    }

    pub fn persist(&self, snapshot: &ZoneBaseSnapshot) -> Result<(), String> {
        self.validate_identity(snapshot)?;
        snapshot.verify()?;
        let bytes = serde_json::to_vec(snapshot)
            .map_err(|error| format!("failed to encode Zone base snapshot: {error}"))?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_ZONE_BASE_SNAPSHOT_FILE_BYTES {
            return Err(format!(
                "Zone base snapshot encoded size {} is invalid",
                bytes.len()
            ));
        }
        let parent = self.path.parent().ok_or_else(|| {
            format!(
                "Zone base snapshot path {} has no parent directory",
                self.path.display()
            )
        })?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_path = parent.join(format!(
            ".base-snapshot-v5-{}-{nonce}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut temp_file = options.open(&temp_path).map_err(|error| {
            format!(
                "failed to create temporary Zone base snapshot {}: {error}",
                temp_path.display()
            )
        })?;
        let write_result = (|| -> Result<(), String> {
            temp_file.write_all(&bytes).map_err(|error| {
                format!(
                    "failed to write temporary Zone base snapshot {}: {error}",
                    temp_path.display()
                )
            })?;
            temp_file.flush().map_err(|error| {
                format!(
                    "failed to flush temporary Zone base snapshot {}: {error}",
                    temp_path.display()
                )
            })?;
            temp_file.sync_all().map_err(|error| {
                format!(
                    "failed to fsync temporary Zone base snapshot {}: {error}",
                    temp_path.display()
                )
            })?;
            drop(temp_file);
            fs::rename(&temp_path, &self.path).map_err(|error| {
                format!(
                    "failed to atomically install Zone base snapshot {}: {error}",
                    self.path.display()
                )
            })?;
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!(
                        "failed to fsync Zone base snapshot directory {}: {error}",
                        parent.display()
                    )
                })?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    fn validate_identity(&self, snapshot: &ZoneBaseSnapshot) -> Result<(), String> {
        if snapshot.zone_id != self.zone_id || snapshot.build_id != self.build_id {
            return Err(format!(
                "Zone base snapshot identity mismatch: expected {}/{}, got {}/{}",
                self.zone_id, self.build_id, snapshot.zone_id, snapshot.build_id
            ));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ZoneMutationWal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ZoneMutationWal")
            .field("path", &self.path)
            .field("zone_id", &self.zone_id)
            .field("build_id", &self.build_id)
            .field("next_sequence", &self.next_sequence)
            .field("latest_digest", &self.latest_digest)
            .finish()
    }
}

impl ZoneMutationWal {
    pub fn open(
        path: impl AsRef<Path>,
        zone_id: impl Into<String>,
        build_id: impl Into<String>,
    ) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let zone_id = zone_id.into();
        let build_id = build_id.into();
        validate_wal_identifier("Zone id", &zone_id)?;
        validate_wal_identifier("build id", &build_id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Zone mutation WAL directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|error| {
            format!(
                "failed to open Zone mutation WAL {}: {error}",
                path.display()
            )
        })?;
        let (next_sequence, latest_digest, valid_length) =
            replay_wal(&file, &path, &zone_id, &build_id)?;
        let file_length = file
            .metadata()
            .map_err(|error| {
                format!(
                    "failed to inspect Zone mutation WAL {}: {error}",
                    path.display()
                )
            })?
            .len();
        if valid_length < file_length {
            file.set_len(valid_length).map_err(|error| {
                format!(
                    "failed to truncate partial Zone mutation WAL tail {}: {error}",
                    path.display()
                )
            })?;
            file.sync_data().map_err(|error| {
                format!(
                    "failed to sync repaired Zone mutation WAL {}: {error}",
                    path.display()
                )
            })?;
        }
        file.seek(SeekFrom::End(0)).map_err(|error| {
            format!(
                "failed to seek Zone mutation WAL {}: {error}",
                path.display()
            )
        })?;
        Ok(Self {
            path,
            file,
            zone_id,
            build_id,
            next_sequence,
            latest_digest,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ack(&self) -> ZoneMutationWalAck {
        ZoneMutationWalAck {
            zone_id: self.zone_id.clone(),
            build_id: self.build_id.clone(),
            next_sequence: self.next_sequence,
            latest_digest: self.latest_digest.clone(),
            durable: true,
        }
    }

    pub fn append_batch(
        &mut self,
        batch: &ZoneMutationBatch,
    ) -> Result<ZoneMutationWalAck, String> {
        validate_batch_continuity(
            batch,
            &self.zone_id,
            &self.build_id,
            self.next_sequence,
            &self.latest_digest,
        )?;
        if batch.entries.is_empty() {
            return Ok(self.ack());
        }
        let record = ZoneMutationWalRecord {
            version: ZONE_MUTATION_WAL_VERSION,
            zone_id: self.zone_id.clone(),
            build_id: self.build_id.clone(),
            batch: Some(batch.clone()),
            anchor: None,
        };
        let encoded = serde_json::to_vec(&record)
            .map_err(|error| format!("failed to encode Zone mutation WAL record: {error}"))?;
        self.file.write_all(&encoded).map_err(|error| {
            format!(
                "failed to append Zone mutation WAL {}: {error}",
                self.path.display()
            )
        })?;
        self.file.write_all(b"\n").map_err(|error| {
            format!(
                "failed to terminate Zone mutation WAL record {}: {error}",
                self.path.display()
            )
        })?;
        self.file.flush().map_err(|error| {
            format!(
                "failed to flush Zone mutation WAL {}: {error}",
                self.path.display()
            )
        })?;
        self.file.sync_data().map_err(|error| {
            format!(
                "failed to fsync Zone mutation WAL {}: {error}",
                self.path.display()
            )
        })?;
        self.next_sequence = batch.next_sequence;
        self.latest_digest = batch.latest_digest.clone();
        Ok(self.ack())
    }

    pub fn compact_to_base(
        &mut self,
        snapshot: &ZoneBaseSnapshot,
    ) -> Result<ZoneMutationWalAck, String> {
        snapshot.verify()?;
        if snapshot.zone_id != self.zone_id || snapshot.build_id != self.build_id {
            return Err(format!(
                "Zone mutation WAL identity mismatch: expected {}/{}, got {}/{}",
                self.zone_id, self.build_id, snapshot.zone_id, snapshot.build_id
            ));
        }
        if !snapshot.apply_ready {
            return Err("cannot compact WAL to an incomplete base snapshot".to_string());
        }
        if snapshot.base_sequence < self.next_sequence {
            return Err(format!(
                "base snapshot cursor {} is behind durable WAL cursor {}",
                snapshot.base_sequence, self.next_sequence
            ));
        }
        if snapshot.base_sequence == self.next_sequence
            && snapshot.latest_digest != self.latest_digest
        {
            return Err(format!(
                "base snapshot digest {} conflicts with durable WAL digest {} at cursor {}",
                snapshot.latest_digest, self.latest_digest, self.next_sequence
            ));
        }
        let record = ZoneMutationWalRecord {
            version: ZONE_MUTATION_WAL_VERSION,
            zone_id: self.zone_id.clone(),
            build_id: self.build_id.clone(),
            batch: None,
            anchor: Some(ZoneMutationWalAnchor {
                snapshot_id: snapshot.snapshot_id.clone(),
                next_sequence: snapshot.base_sequence,
                latest_digest: snapshot.latest_digest.clone(),
            }),
        };
        let mut encoded = serde_json::to_vec(&record)
            .map_err(|error| format!("failed to encode Zone mutation WAL base anchor: {error}"))?;
        encoded.push(b'\n');
        replace_wal_atomically(&self.path, &encoded)?;
        self.file = open_wal_file(&self.path)?;
        self.next_sequence = snapshot.base_sequence;
        self.latest_digest = snapshot.latest_digest.clone();
        Ok(self.ack())
    }
}

fn replay_wal(
    file: &File,
    path: &Path,
    expected_zone_id: &str,
    expected_build_id: &str,
) -> Result<(u64, String, u64), String> {
    let reader_file = file.try_clone().map_err(|error| {
        format!(
            "failed to clone Zone mutation WAL handle {}: {error}",
            path.display()
        )
    })?;
    let mut reader = BufReader::new(reader_file);
    let mut next_sequence = 0u64;
    let mut latest_digest = ZERO_DIGEST.to_string();
    let mut valid_length = 0u64;
    let mut saw_record = false;
    loop {
        let mut line = Vec::new();
        let bytes_read = reader.read_until(b'\n', &mut line).map_err(|error| {
            format!(
                "failed to read Zone mutation WAL {}: {error}",
                path.display()
            )
        })?;
        if bytes_read == 0 {
            break;
        }
        if !line.ends_with(b"\n") {
            break;
        }
        line.pop();
        let record: ZoneMutationWalRecord = serde_json::from_slice(&line).map_err(|error| {
            format!(
                "Zone mutation WAL {} contains a corrupt complete record: {error}",
                path.display()
            )
        })?;
        if record.version != ZONE_MUTATION_WAL_VERSION {
            return Err(format!(
                "Zone mutation WAL {} has version {}, expected {}",
                path.display(),
                record.version,
                ZONE_MUTATION_WAL_VERSION
            ));
        }
        if record.zone_id != expected_zone_id || record.build_id != expected_build_id {
            return Err(format!(
                "Zone mutation WAL {} identity mismatch: expected {expected_zone_id}/{expected_build_id}, got {}/{}",
                path.display(),
                record.zone_id,
                record.build_id
            ));
        }
        match (record.batch, record.anchor) {
            (Some(batch), None) => {
                validate_batch_continuity(
                    &batch,
                    expected_zone_id,
                    expected_build_id,
                    next_sequence,
                    &latest_digest,
                )?;
                next_sequence = batch.next_sequence;
                latest_digest = batch.latest_digest;
            }
            (None, Some(anchor)) if !saw_record => {
                validate_wal_identifier("base snapshot id", &anchor.snapshot_id)?;
                validate_wal_digest(&anchor.latest_digest)?;
                next_sequence = anchor.next_sequence;
                latest_digest = anchor.latest_digest;
            }
            (None, Some(_)) => {
                return Err(format!(
                    "Zone mutation WAL {} contains a non-leading base anchor",
                    path.display()
                ));
            }
            _ => {
                return Err(format!(
                    "Zone mutation WAL {} record must contain exactly one batch or base anchor",
                    path.display()
                ));
            }
        }
        saw_record = true;
        valid_length = valid_length.saturating_add(bytes_read as u64);
    }
    Ok((next_sequence, latest_digest, valid_length))
}

fn open_wal_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| {
        format!(
            "failed to reopen Zone mutation WAL {}: {error}",
            path.display()
        )
    })
}

fn replace_wal_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Zone mutation WAL path {} has no parent directory",
            path.display()
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = parent.join(format!(
        ".mutation-wal-v5-{}-{nonce}.tmp",
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write_result = (|| -> Result<(), String> {
        let mut temp = options.open(&temp_path).map_err(|error| {
            format!(
                "failed to create compacted Zone mutation WAL {}: {error}",
                temp_path.display()
            )
        })?;
        temp.write_all(bytes).map_err(|error| {
            format!(
                "failed to write compacted Zone mutation WAL {}: {error}",
                temp_path.display()
            )
        })?;
        temp.flush().map_err(|error| {
            format!(
                "failed to flush compacted Zone mutation WAL {}: {error}",
                temp_path.display()
            )
        })?;
        temp.sync_all().map_err(|error| {
            format!(
                "failed to fsync compacted Zone mutation WAL {}: {error}",
                temp_path.display()
            )
        })?;
        drop(temp);
        fs::rename(&temp_path, path).map_err(|error| {
            format!(
                "failed to atomically replace Zone mutation WAL {}: {error}",
                path.display()
            )
        })?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!(
                    "failed to fsync Zone mutation WAL directory {}: {error}",
                    parent.display()
                )
            })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn validate_wal_digest(value: &str) -> Result<(), String> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err("Zone mutation WAL digest must be 64 lowercase hexadecimal bytes".to_string());
    }
    Ok(())
}

fn validate_batch_continuity(
    batch: &ZoneMutationBatch,
    expected_zone_id: &str,
    expected_build_id: &str,
    expected_sequence: u64,
    expected_digest: &str,
) -> Result<(), String> {
    batch.verify()?;
    if batch.mutation_coverage != ZoneReplicationCoverage::CommandJournal {
        return Err("Zone mutation WAL only accepts command-journal coverage".to_string());
    }
    if batch.zone_id != expected_zone_id {
        return Err(format!(
            "Zone mutation batch targets {}, WAL targets {expected_zone_id}",
            batch.zone_id
        ));
    }
    if batch.build_id != expected_build_id {
        return Err(format!(
            "Zone mutation batch build {} does not match WAL build {expected_build_id}",
            batch.build_id
        ));
    }
    if batch.first_sequence != expected_sequence {
        return Err(format!(
            "Zone mutation batch begins at {}, durable cursor is {expected_sequence}",
            batch.first_sequence
        ));
    }
    if batch.previous_digest != expected_digest {
        return Err(format!(
            "Zone mutation batch previous digest {} does not match durable digest {expected_digest}",
            batch.previous_digest
        ));
    }
    Ok(())
}

fn validate_wal_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > 160 || value.chars().any(char::is_control) {
        return Err(format!("{label} is invalid"));
    }
    Ok(())
}
