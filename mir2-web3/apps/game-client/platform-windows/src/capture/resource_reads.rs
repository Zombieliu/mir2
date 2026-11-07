//! Separate, opt-in read observations. This never grants content authority or
//! changes whether the original PNG/capture-v1 pair can be accepted.
use super::{NativeCaptureRequest, VerifiedPackedProvenance};
use mir2_resource_pack::ReadProfileSnapshot;
use serde::Serialize;
use std::{fs, fs::OpenOptions, io, io::Write, path::Path};

const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(super) struct FrozenResourceReads {
    profile: ReadProfileSnapshot,
    provenance: VerifiedPackedProvenance,
}

pub(super) fn freeze(
    profile: Option<ReadProfileSnapshot>,
    provenance: Option<&VerifiedPackedProvenance>,
) -> io::Result<Option<FrozenResourceReads>> {
    let Some(profile) = profile else {
        return Ok(None);
    };
    let provenance =
        provenance.ok_or_else(|| invalid("authenticated packed provenance missing"))?;
    if profile.schema != "mir2.native.verified-resource-reads.v1"
        || !super::is_sha256_hex(&profile.catalog_sha256)
        || !super::is_sha256_hex(&provenance.generation_manifest_sha256)
        || !profile
            .catalog_sha256
            .eq_ignore_ascii_case(&provenance.catalog_sha256)
    {
        return Err(invalid("resource profile/catalog provenance mismatch"));
    }
    Ok(Some(FrozenResourceReads {
        profile,
        provenance: provenance.clone(),
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostic<'a> {
    schema: &'static str,
    diagnostic_only: bool,
    complete_boot_closure_claimed: bool,
    observation_basis: &'static str,
    captured_at: String,
    request_frame: Option<u32>,
    run_id: &'a Option<String>,
    scene: &'a str,
    world_epoch: Option<u64>,
    map_file_name: &'a Option<String>,
    image_path: &'a str,
    image_sha256: &'a str,
    generation: &'a VerifiedPackedProvenance,
    reads: &'a ReadProfileSnapshot,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn serialize(request: &NativeCaptureRequest, image_sha256: &str) -> io::Result<Option<Vec<u8>>> {
    let Some(frozen) = request.resource_reads.as_ref() else {
        return Ok(None);
    };
    if request.build.packed_generation.as_ref() != Some(&frozen.provenance)
        || !super::is_sha256_hex(image_sha256)
    {
        return Err(invalid("capture/profile generation or PNG binding differs"));
    }
    let image_path = request
        .png_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("capture image filename is not UTF-8"))?;
    let bytes = serde_json::to_vec_pretty(&Diagnostic {
        schema: "mir2.native.capture-resource-reads.v1",
        diagnostic_only: true,
        complete_boot_closure_claimed: false,
        observation_basis: "cumulative-successful-verified-reads-before-screenshot-request",
        captured_at: super::format_rfc3339_utc(request.captured_at_ms),
        request_frame: request.request_frame,
        run_id: &request.run_id,
        scene: &request.scene,
        world_epoch: request.world.epoch,
        map_file_name: &request.world.map_file_name,
        image_path,
        image_sha256,
        generation: &frozen.provenance,
        reads: &frozen.profile,
    })
    .map_err(io::Error::other)?;
    if bytes.len() > MAX_DIAGNOSTIC_BYTES {
        return Err(invalid("resource-read diagnostic exceeds byte bound"));
    }
    Ok(Some(bytes))
}

pub(super) fn write(request: &NativeCaptureRequest, image_sha256: &str) -> io::Result<()> {
    let Some(bytes) = serialize(request, image_sha256)? else {
        return Ok(());
    };
    write_new_synced(
        &request.png_path.with_extension("resource-reads.json"),
        &bytes,
    )
}

/// Fsync private bytes before atomic no-clobber publication. A collision never
/// replaces earlier evidence; failed diagnostics do not alter the PNG marker.
fn write_new_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = super::temporary_capture_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    // From here the temporary is ours. An earlier create_new failure never
    // removes a pre-existing file that this process did not create.
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::hard_link(&temporary, path)
    })();
    let cleanup = fs::remove_file(&temporary);
    result.and(cleanup)
}

#[cfg(test)]
#[path = "resource_reads_tests.rs"]
mod tests;
