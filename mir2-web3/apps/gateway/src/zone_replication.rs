use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ZoneMutationBatch, ZoneReplicationCoverage};

const ZONE_MUTATION_WAL_VERSION: u32 = 1;
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

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
    batch: ZoneMutationBatch,
}

pub struct ZoneMutationWal {
    path: PathBuf,
    file: File,
    zone_id: String,
    build_id: String,
    next_sequence: u64,
    latest_digest: String,
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
            batch: batch.clone(),
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
        validate_batch_continuity(
            &record.batch,
            expected_zone_id,
            expected_build_id,
            next_sequence,
            &latest_digest,
        )?;
        next_sequence = record.batch.next_sequence;
        latest_digest = record.batch.latest_digest;
        valid_length = valid_length.saturating_add(bytes_read as u64);
    }
    Ok((next_sequence, latest_digest, valid_length))
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
