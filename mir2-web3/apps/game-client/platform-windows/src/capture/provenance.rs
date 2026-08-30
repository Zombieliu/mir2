//! Trusted build provenance for native visual captures.
//!
//! Strict capture is available only for a complete staged Candidate whose
//! payload, build attestation, canonical release statement, and detached CMS
//! signature all validate against an out-of-band trusted signer thumbprint.
//! Development, incomplete, dirty, modified, and unsigned packages remain on
//! the draft sidecar schema.

use super::{is_sha256_hex, sha256_hex, NativeCaptureBuild};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Component, Path},
};

const TRUSTED_SIGNER_ENV: &str = "MIR2_NATIVE_TRUSTED_SIGNER_THUMBPRINT";
const VERSION_SCHEMA: &str = "mir2.windows.candidate-version.v4";
const MANIFEST_SCHEMA: &str = "mir2.windows.package-manifest.v4";
const ATTESTATION_SCHEMA: &str = "mir2.windows.build-attestation.v2";
const STATEMENT_SCHEMA: &str = "mir2.windows.release-statement.v1";
const WORKTREE_SCOPE: &str = "git-status-z+diff+all-untracked-content-v2";
const SIGNATURE_FORMAT: &str = "CMS/PKCS7-detached";
const MANIFEST_RULE: &str = "Payload files are hashed; manifest/version are bound by the detached signed release statement; statement/signature are excluded to avoid self-reference.";
const MANIFEST_EXCLUDES: [&str; 4] = [
    "PACKAGE-MANIFEST.json",
    "VERSION.json",
    "RELEASE-STATEMENT.json",
    "RELEASE-STATEMENT.p7s",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageManifest {
    schema: String,
    coverage: PackageCoverage,
    #[serde(rename = "fileCount")]
    file_count: u64,
    #[serde(rename = "totalBytes")]
    total_bytes: u64,
    #[serde(rename = "aggregateSha256")]
    aggregate_sha256: String,
    files: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageCoverage {
    excludes: Vec<String>,
    rule: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ManifestEntry {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CandidateVersion {
    schema: String,
    candidate: String,
    git_revision: String,
    worktree_dirty: bool,
    worktree_status_scope: String,
    worktree_status_sha256: String,
    build_attestation_sha256: String,
    build_completed_utc: String,
    exe_name: String,
    exe_sha256: String,
    exe_size_bytes: u64,
    package_manifest_schema: String,
    package_manifest_sha256: String,
    package_manifest_aggregate_sha256: String,
    package_manifest_file_count: u64,
    package_file_count: u64,
    release_statement_schema: String,
    signature_format: String,
    staged: bool,
    built_by_packaging_script: bool,
    client_only: bool,
    accepted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildAttestation {
    schema: String,
    exe_sha256: String,
    exe_size_bytes: u64,
    git_revision: String,
    worktree_dirty: bool,
    worktree_status_scope: String,
    worktree_status_sha256: String,
    worktree_status_line_count: u64,
    cargo_version: String,
    rustc_version: String,
    build_command: BuildCommand,
    path_remapping: PathRemapping,
    build_completed_utc: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildCommand {
    executable: String,
    toolchain: String,
    subcommand: String,
    manifest_path: String,
    bin: String,
    release: bool,
    locked: bool,
    target: String,
    profile: String,
    target_dir: String,
    extra_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathRemapping {
    enabled: bool,
    environment_variable: String,
    flags: Vec<PathRemappingFlag>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PathRemappingFlag {
    source_token: String,
    destination: String,
}

pub(super) fn trusted_build_provenance() -> NativeCaptureBuild {
    // Strict evidence must never outlive the package bytes it describes. A
    // cached success would allow later captures to remain strict after a lazy
    // asset, manifest, or sibling payload file was modified.
    load_trusted_build_provenance()
}

fn load_trusted_build_provenance() -> NativeCaptureBuild {
    let trusted_thumbprint = env::var(TRUSTED_SIGNER_ENV)
        .ok()
        .and_then(|value| normalize_thumbprint(&value));
    let Some(trusted_thumbprint) = trusted_thumbprint else {
        return NativeCaptureBuild::default();
    };
    let Some(executable_path) = env::current_exe().ok() else {
        return NativeCaptureBuild::default();
    };
    let Some(package_root) = executable_path.parent() else {
        return NativeCaptureBuild::default();
    };
    if !has_no_reparse_ancestors(package_root) {
        return NativeCaptureBuild::default();
    }
    let Some(executable_bytes) = read_limited(&executable_path, u64::MAX) else {
        return NativeCaptureBuild::default();
    };
    let Some(version_bytes) = read_limited(&package_root.join("VERSION.json"), 256 * 1024) else {
        return NativeCaptureBuild::default();
    };
    let Some(manifest_bytes) = read_limited(
        &package_root.join("PACKAGE-MANIFEST.json"),
        32 * 1024 * 1024,
    ) else {
        return NativeCaptureBuild::default();
    };
    let Some(attestation_bytes) =
        read_limited(&package_root.join("BUILD-ATTESTATION.json"), 512 * 1024)
    else {
        return NativeCaptureBuild::default();
    };
    let Some(statement_bytes) =
        read_limited(&package_root.join("RELEASE-STATEMENT.json"), 16 * 1024)
    else {
        return NativeCaptureBuild::default();
    };
    let Some(signature_bytes) =
        read_limited(&package_root.join("RELEASE-STATEMENT.p7s"), 256 * 1024)
    else {
        return NativeCaptureBuild::default();
    };
    if !verify_detached_cms_signature(&statement_bytes, &signature_bytes, &trusted_thumbprint) {
        return NativeCaptureBuild::default();
    }
    let Some(actual_payload_files) = collect_payload_files(package_root) else {
        return NativeCaptureBuild::default();
    };
    let executable_name = executable_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    parse_trusted_build_provenance(
        &version_bytes,
        &manifest_bytes,
        &attestation_bytes,
        &statement_bytes,
        executable_name,
        executable_bytes.len() as u64,
        &sha256_hex(&executable_bytes),
        &actual_payload_files,
        true,
    )
    .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn parse_trusted_build_provenance(
    version_bytes: &[u8],
    package_manifest_bytes: &[u8],
    attestation_bytes: &[u8],
    statement_bytes: &[u8],
    executable_name: &str,
    executable_size: u64,
    executable_sha256: &str,
    actual_payload_files: &[ManifestEntry],
    signature_verified: bool,
) -> Option<NativeCaptureBuild> {
    if !signature_verified || executable_name.is_empty() || !is_sha256_hex(executable_sha256) {
        return None;
    }
    let executable_sha256_upper = executable_sha256.to_ascii_uppercase();
    let version: CandidateVersion = serde_json::from_slice(version_bytes).ok()?;
    let manifest: PackageManifest = serde_json::from_slice(package_manifest_bytes).ok()?;
    let attestation: BuildAttestation = serde_json::from_slice(attestation_bytes).ok()?;

    if version.schema != VERSION_SCHEMA
        || !valid_candidate_name(&version.candidate)
        || !valid_revision(&version.git_revision)
        || version.worktree_dirty
        || version.worktree_status_scope != WORKTREE_SCOPE
        || !is_upper_sha256(&version.worktree_status_sha256)
        || version.worktree_status_sha256 != expected_clean_worktree_sha256(&version.git_revision)
        || version.exe_name != executable_name
        || version.exe_sha256 != executable_sha256_upper
        || version.exe_size_bytes != executable_size
        || version.package_manifest_schema != MANIFEST_SCHEMA
        || version.release_statement_schema != STATEMENT_SCHEMA
        || version.signature_format != SIGNATURE_FORMAT
        || !version.staged
        || version.built_by_packaging_script
        || !version.client_only
        || version.accepted
        || !valid_utc_timestamp(&version.build_completed_utc)
    {
        return None;
    }

    if attestation.schema != ATTESTATION_SCHEMA
        || attestation.exe_sha256 != executable_sha256_upper
        || attestation.exe_size_bytes != executable_size
        || attestation.git_revision != version.git_revision
        || attestation.worktree_dirty
        || attestation.worktree_status_scope != WORKTREE_SCOPE
        || attestation.worktree_status_sha256 != version.worktree_status_sha256
        || attestation.worktree_status_line_count != 0
        || !valid_tool_version(&attestation.cargo_version, "cargo 1.95.0")
        || !valid_tool_version(&attestation.rustc_version, "rustc 1.95.0")
        || attestation.build_completed_utc != version.build_completed_utc
        || !valid_utc_timestamp(&attestation.build_completed_utc)
        || !valid_build_command(&attestation.build_command)
        || !valid_path_remapping(&attestation.path_remapping)
    {
        return None;
    }

    let attestation_sha256 = sha256_hex(attestation_bytes).to_ascii_uppercase();
    if version.build_attestation_sha256 != attestation_sha256 {
        return None;
    }

    if manifest.schema != MANIFEST_SCHEMA
        || manifest.coverage.rule != MANIFEST_RULE
        || manifest.coverage.excludes
            != MANIFEST_EXCLUDES
                .iter()
                .map(|value| (*value).to_owned())
                .collect::<Vec<_>>()
        || manifest.file_count != manifest.files.len() as u64
        || manifest.file_count != version.package_manifest_file_count
        || version.package_file_count != manifest.file_count.checked_add(4)?
        || !valid_manifest_entries(&manifest.files)
        || manifest.files != actual_payload_files
    {
        return None;
    }
    let total_bytes = manifest
        .files
        .iter()
        .try_fold(0u64, |sum, entry| sum.checked_add(entry.size))?;
    if manifest.total_bytes != total_bytes {
        return None;
    }
    let aggregate_sha256 = manifest_aggregate_sha256(&manifest.files);
    if manifest.aggregate_sha256 != aggregate_sha256
        || version.package_manifest_aggregate_sha256 != aggregate_sha256
    {
        return None;
    }
    let manifest_sha256 = sha256_hex(package_manifest_bytes).to_ascii_uppercase();
    if version.package_manifest_sha256 != manifest_sha256 {
        return None;
    }

    let expected_statement = release_statement(
        &version.candidate,
        &executable_sha256_upper,
        &manifest_sha256,
        &aggregate_sha256,
        &sha256_hex(version_bytes).to_ascii_uppercase(),
        &attestation_sha256,
        &version.git_revision,
        false,
        &version.worktree_status_sha256,
    );
    if statement_bytes != expected_statement.as_bytes() {
        return None;
    }

    Some(NativeCaptureBuild {
        source_revision: Some(version.git_revision),
        executable_sha256: Some(executable_sha256.to_ascii_lowercase()),
        asset_manifest_sha256: Some(manifest_sha256.to_ascii_lowercase()),
    })
}

fn valid_build_command(command: &BuildCommand) -> bool {
    command.executable == "cargo"
        && command.toolchain == "+1.95.0"
        && command.subcommand == "build"
        && command.manifest_path == "apps/game-client/platform-windows/Cargo.toml"
        && command.bin == "mir2-platform-windows"
        && command.release
        && command.locked
        && command.target == "x86_64-pc-windows-msvc"
        && command.profile == "release"
        && command.target_dir == "target-attested-windows-candidate"
        && command.extra_args.is_empty()
}

fn valid_path_remapping(remapping: &PathRemapping) -> bool {
    remapping.enabled
        && remapping.environment_variable == "RUSTFLAGS"
        && remapping.flags.len() == 2
        && remapping.flags[0]
            == (PathRemappingFlag {
                source_token: "<REPO_ROOT>".to_owned(),
                destination: ".".to_owned(),
            })
        && remapping.flags[1]
            == (PathRemappingFlag {
                source_token: "<CARGO_HOME>".to_owned(),
                destination: "cargo-home".to_owned(),
            })
}

fn valid_manifest_entries(entries: &[ManifestEntry]) -> bool {
    let mut previous: Option<&str> = None;
    let mut case_folded = BTreeSet::new();
    for entry in entries {
        if !valid_package_path(&entry.path)
            || !is_upper_sha256(&entry.sha256)
            || previous.is_some_and(|value| value >= entry.path.as_str())
            || !case_folded.insert(entry.path.to_ascii_lowercase())
        {
            return false;
        }
        previous = Some(&entry.path);
    }
    true
}

fn valid_package_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains(':')
        && !value.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn manifest_aggregate_sha256(entries: &[ManifestEntry]) -> String {
    let mut canonical = String::new();
    for entry in entries {
        canonical.push_str(&entry.path);
        canonical.push('\t');
        canonical.push_str(&entry.size.to_string());
        canonical.push('\t');
        canonical.push_str(&entry.sha256);
        canonical.push('\n');
    }
    sha256_hex(canonical.as_bytes()).to_ascii_uppercase()
}

#[allow(clippy::too_many_arguments)]
fn release_statement(
    candidate: &str,
    exe_sha256: &str,
    manifest_sha256: &str,
    manifest_aggregate_sha256: &str,
    version_sha256: &str,
    attestation_sha256: &str,
    git_revision: &str,
    worktree_dirty: bool,
    dirty_digest: &str,
) -> String {
    format!(
        "{{\"schema\":\"{STATEMENT_SCHEMA}\",\"candidate\":\"{candidate}\",\"exeSha256\":\"{exe_sha256}\",\"packageManifestSha256\":\"{manifest_sha256}\",\"packageManifestAggregateSha256\":\"{manifest_aggregate_sha256}\",\"versionSha256\":\"{version_sha256}\",\"buildAttestationSha256\":\"{attestation_sha256}\",\"gitRevision\":\"{git_revision}\",\"worktreeDirty\":{},\"worktreeStatusSha256\":\"{dirty_digest}\"}}",
        if worktree_dirty { "true" } else { "false" }
    )
}

fn valid_candidate_name(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix("WN-CANDIDATE-") else {
        return false;
    };
    !suffix.is_empty()
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_upper_sha256(value: &str) -> bool {
    is_sha256_hex(value) && value.bytes().all(|byte| !byte.is_ascii_lowercase())
}

fn valid_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 33
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || &bytes[27..] != b"+00:00"
        || bytes.iter().enumerate().any(|(index, byte)| {
            !matches!(index, 4 | 7 | 10 | 13 | 16 | 19 | 27 | 30) && !byte.is_ascii_digit()
        })
    {
        return false;
    }

    let Some(year) = decimal(bytes, 0, 4) else {
        return false;
    };
    let Some(month) = decimal(bytes, 5, 7) else {
        return false;
    };
    let Some(day) = decimal(bytes, 8, 10) else {
        return false;
    };
    let Some(hour) = decimal(bytes, 11, 13) else {
        return false;
    };
    let Some(minute) = decimal(bytes, 14, 16) else {
        return false;
    };
    let Some(second) = decimal(bytes, 17, 19) else {
        return false;
    };
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && day >= 1 && day <= days_in_month && hour <= 23 && minute <= 59 && second <= 59
}

fn decimal(bytes: &[u8], start: usize, end: usize) -> Option<u32> {
    bytes.get(start..end)?.iter().try_fold(0u32, |value, byte| {
        byte.is_ascii_digit()
            .then_some(value * 10 + u32::from(*byte - b'0'))
    })
}

fn valid_tool_version(value: &str, expected: &str) -> bool {
    let Some(details) = value.strip_prefix(expected) else {
        return false;
    };
    details.starts_with(" (")
        && details.ends_with(')')
        && details.len() > 3
        && !details.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
}

fn expected_clean_worktree_sha256(revision: &str) -> String {
    let empty_hash = sha256_hex(&[]).to_ascii_uppercase();
    let canonical = format!(
        "SCOPE\n{WORKTREE_SCOPE}\nREVISION\n{revision}\nSTATUS-Z\n0\n{empty_hash}\nINDEX-DIFF\n0\n{empty_hash}\nWORKTREE-DIFF\n0\n{empty_hash}\nUNTRACKED\n0\n\n"
    );
    sha256_hex(canonical.as_bytes()).to_ascii_uppercase()
}

fn normalize_thumbprint(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_uppercase();
    (normalized.len() == 40 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(normalized)
}

fn read_limited(path: &Path, maximum: u64) -> Option<Vec<u8>> {
    let metadata = safe_metadata(path)?;
    if !metadata.is_file() || metadata.len() > maximum {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    (bytes.len() as u64 == metadata.len()).then_some(bytes)
}

fn collect_payload_files(package_root: &Path) -> Option<Vec<ManifestEntry>> {
    let root_metadata = safe_metadata(package_root)?;
    if !root_metadata.is_dir() {
        return None;
    }
    let mut paths = Vec::new();
    collect_files_recursive(package_root, package_root, &mut paths)?;
    let mut entries = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = relative_package_path(package_root, &path)?;
        if MANIFEST_EXCLUDES.contains(&relative.as_str()) {
            continue;
        }
        let metadata = safe_metadata(&path)?;
        let bytes = fs::read(&path).ok()?;
        if bytes.len() as u64 != metadata.len() {
            return None;
        }
        entries.push(ManifestEntry {
            path: relative,
            size: bytes.len() as u64,
            sha256: sha256_hex(&bytes).to_ascii_uppercase(),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    valid_manifest_entries(&entries).then_some(entries)
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    files: &mut Vec<std::path::PathBuf>,
) -> Option<()> {
    for entry in fs::read_dir(current).ok()? {
        let path = entry.ok()?.path();
        let metadata = safe_metadata(&path)?;
        if metadata.is_dir() {
            collect_files_recursive(root, &path, files)?;
        } else if metadata.is_file() {
            if path.strip_prefix(root).is_err() {
                return None;
            }
            files.push(path);
        } else {
            return None;
        }
    }
    Some(())
}

fn relative_package_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return None;
        };
        parts.push(value.to_str()?.to_owned());
    }
    let value = parts.join("/");
    valid_package_path(&value).then_some(value)
}

fn safe_metadata(path: &Path) -> Option<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return None;
        }
    }
    Some(metadata)
}

fn has_no_reparse_ancestors(path: &Path) -> bool {
    path.ancestors()
        .all(|ancestor| safe_metadata(ancestor).is_some())
}

#[cfg(windows)]
fn verify_detached_cms_signature(
    content: &[u8],
    signature: &[u8],
    expected_thumbprint: &str,
) -> bool {
    use std::{ffi::CStr, ptr};
    use windows_sys::Win32::Foundation::{GetLastError, SetLastError, CRYPT_E_NO_SIGNER};
    use windows_sys::Win32::Security::Cryptography::{
        CertFreeCertificateContext, CertGetCertificateContextProperty, CertGetEnhancedKeyUsage,
        CertGetIntendedKeyUsage, CertVerifyTimeValidity, CryptVerifyDetachedMessageSignature,
        CERT_CONTEXT, CERT_DIGITAL_SIGNATURE_KEY_USAGE, CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG,
        CERT_HASH_PROP_ID, CRYPT_VERIFY_MESSAGE_PARA, CTL_USAGE, PKCS_7_ASN_ENCODING,
        X509_ASN_ENCODING,
    };

    let Ok(content_len) = u32::try_from(content.len()) else {
        return false;
    };
    let Ok(signature_len) = u32::try_from(signature.len()) else {
        return false;
    };
    if content.is_empty() || signature.is_empty() {
        return false;
    }
    let parameters = CRYPT_VERIFY_MESSAGE_PARA {
        cbSize: std::mem::size_of::<CRYPT_VERIFY_MESSAGE_PARA>() as u32,
        dwMsgAndCertEncodingType: X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
        hCryptProv: 0,
        pfnGetSignerCertificate: None,
        pvGetArg: ptr::null_mut(),
    };
    let content_pointers = [content.as_ptr()];
    let content_lengths = [content_len];
    let mut signer: *mut CERT_CONTEXT = ptr::null_mut();
    let verified = unsafe {
        CryptVerifyDetachedMessageSignature(
            &parameters,
            0,
            signature.as_ptr(),
            signature_len,
            1,
            content_pointers.as_ptr(),
            content_lengths.as_ptr(),
            &mut signer,
        ) != 0
    };
    if !verified || signer.is_null() {
        if !signer.is_null() {
            unsafe {
                CertFreeCertificateContext(signer);
            }
        }
        return false;
    }

    unsafe {
        SetLastError(0);
    }
    let extra_signer = unsafe {
        CryptVerifyDetachedMessageSignature(
            &parameters,
            1,
            signature.as_ptr(),
            signature_len,
            1,
            content_pointers.as_ptr(),
            content_lengths.as_ptr(),
            ptr::null_mut(),
        ) != 0
    };
    let extra_signer_error = unsafe { GetLastError() };
    if extra_signer || extra_signer_error as i32 != CRYPT_E_NO_SIGNER {
        unsafe {
            CertFreeCertificateContext(signer);
        }
        return false;
    }

    let result = (|| unsafe {
        if (*signer).pCertInfo.is_null()
            || CertVerifyTimeValidity(ptr::null(), (*signer).pCertInfo) != 0
        {
            return false;
        }

        let mut hash_size = 0u32;
        if CertGetCertificateContextProperty(
            signer,
            CERT_HASH_PROP_ID,
            ptr::null_mut(),
            &mut hash_size,
        ) == 0
            || hash_size != 20
        {
            return false;
        }
        let mut hash = vec![0u8; hash_size as usize];
        if CertGetCertificateContextProperty(
            signer,
            CERT_HASH_PROP_ID,
            hash.as_mut_ptr().cast(),
            &mut hash_size,
        ) == 0
        {
            return false;
        }
        let actual_thumbprint = hash
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        if actual_thumbprint != expected_thumbprint {
            return false;
        }

        let mut key_usage = [0u8; 2];
        SetLastError(0);
        let has_key_usage = CertGetIntendedKeyUsage(
            (*signer).dwCertEncodingType,
            (*signer).pCertInfo,
            key_usage.as_mut_ptr(),
            key_usage.len() as u32,
        ) != 0;
        let key_usage_error = GetLastError();
        if (has_key_usage && key_usage[0] & CERT_DIGITAL_SIGNATURE_KEY_USAGE as u8 == 0)
            || (!has_key_usage && key_usage_error != 0)
        {
            return false;
        }

        let mut usage_size = 0u32;
        if CertGetEnhancedKeyUsage(
            signer,
            CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG,
            ptr::null_mut(),
            &mut usage_size,
        ) == 0
            || usage_size < std::mem::size_of::<CTL_USAGE>() as u32
        {
            return false;
        }
        let usage_word_count = (usage_size as usize).div_ceil(std::mem::size_of::<usize>());
        let mut usage_buffer = vec![0usize; usage_word_count];
        let usage = usage_buffer.as_mut_ptr().cast::<CTL_USAGE>();
        if CertGetEnhancedKeyUsage(
            signer,
            CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG,
            usage,
            &mut usage_size,
        ) == 0
            || (*usage).rgpszUsageIdentifier.is_null()
        {
            return false;
        }
        (0..(*usage).cUsageIdentifier as usize).any(|index| {
            let identifier = *(*usage).rgpszUsageIdentifier.add(index);
            !identifier.is_null()
                && CStr::from_ptr(identifier.cast::<i8>())
                    .to_bytes()
                    .eq(b"1.3.6.1.5.5.7.3.3")
        })
    })();
    unsafe {
        CertFreeCertificateContext(signer);
    }
    result
}

#[cfg(not(windows))]
fn verify_detached_cms_signature(
    _content: &[u8],
    _signature: &[u8],
    _expected_thumbprint: &str,
) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        version: Vec<u8>,
        manifest: Vec<u8>,
        attestation: Vec<u8>,
        statement: Vec<u8>,
        actual_files: Vec<ManifestEntry>,
        executable_hash: String,
    }

    fn fixture() -> Fixture {
        fixture_with_worktree_hash(None)
    }

    fn fixture_with_worktree_hash(worktree_hash_override: Option<String>) -> Fixture {
        let executable_hash = "a".repeat(64);
        let revision = "78c1e62aed8f7eea82d2a84110a3a3e48c229161";
        let worktree_hash =
            worktree_hash_override.unwrap_or_else(|| expected_clean_worktree_sha256(revision));
        let completed = "2026-08-24T00:00:00.0000000+00:00";
        let attestation = serde_json::to_vec(&json!({
            "schema": ATTESTATION_SCHEMA,
            "exeSha256": executable_hash.to_ascii_uppercase(),
            "exeSizeBytes": 1234,
            "gitRevision": revision,
            "worktreeDirty": false,
            "worktreeStatusScope": WORKTREE_SCOPE,
            "worktreeStatusSha256": worktree_hash,
            "worktreeStatusLineCount": 0,
            "cargoVersion": "cargo 1.95.0 (fixture)",
            "rustcVersion": "rustc 1.95.0 (fixture)",
            "buildCommand": {
                "executable": "cargo", "toolchain": "+1.95.0", "subcommand": "build",
                "manifestPath": "apps/game-client/platform-windows/Cargo.toml",
                "bin": "mir2-platform-windows", "release": true, "locked": true,
                "target": "x86_64-pc-windows-msvc", "profile": "release",
                "targetDir": "target-attested-windows-candidate", "extraArgs": []
            },
            "pathRemapping": {
                "enabled": true, "environmentVariable": "RUSTFLAGS",
                "flags": [
                    {"sourceToken": "<REPO_ROOT>", "destination": "."},
                    {"sourceToken": "<CARGO_HOME>", "destination": "cargo-home"}
                ]
            },
            "buildCompletedUtc": completed
        }))
        .expect("attestation fixture");
        let attestation_hash = sha256_hex(&attestation).to_ascii_uppercase();
        let actual_files = vec![
            ManifestEntry {
                path: "BUILD-ATTESTATION.json".to_owned(),
                size: attestation.len() as u64,
                sha256: attestation_hash.clone(),
            },
            ManifestEntry {
                path: "mir2-platform-windows.exe".to_owned(),
                size: 1234,
                sha256: executable_hash.to_ascii_uppercase(),
            },
        ];
        let aggregate = manifest_aggregate_sha256(&actual_files);
        let manifest = serde_json::to_vec(&json!({
            "schema": MANIFEST_SCHEMA,
            "coverage": {"excludes": MANIFEST_EXCLUDES, "rule": MANIFEST_RULE},
            "fileCount": actual_files.len(),
            "totalBytes": actual_files.iter().map(|entry| entry.size).sum::<u64>(),
            "aggregateSha256": aggregate,
            "files": actual_files.iter().map(|entry| json!({
                "path": entry.path, "size": entry.size, "sha256": entry.sha256
            })).collect::<Vec<_>>()
        }))
        .expect("manifest fixture");
        let manifest_hash = sha256_hex(&manifest).to_ascii_uppercase();
        let version = serde_json::to_vec(&json!({
            "schema": VERSION_SCHEMA,
            "candidate": "WN-CANDIDATE-TEST",
            "gitRevision": revision,
            "worktreeDirty": false,
            "worktreeStatusScope": WORKTREE_SCOPE,
            "worktreeStatusSha256": worktree_hash,
            "buildAttestationSha256": attestation_hash,
            "buildCompletedUtc": completed,
            "exeName": "mir2-platform-windows.exe",
            "exeSha256": executable_hash.to_ascii_uppercase(),
            "exeSizeBytes": 1234,
            "packageManifestSchema": MANIFEST_SCHEMA,
            "packageManifestSha256": manifest_hash,
            "packageManifestAggregateSha256": aggregate,
            "packageManifestFileCount": actual_files.len(),
            "packageFileCount": actual_files.len() + 4,
            "releaseStatementSchema": STATEMENT_SCHEMA,
            "signatureFormat": SIGNATURE_FORMAT,
            "staged": true,
            "builtByPackagingScript": false,
            "clientOnly": true,
            "accepted": false
        }))
        .expect("version fixture");
        let statement = release_statement(
            "WN-CANDIDATE-TEST",
            &executable_hash.to_ascii_uppercase(),
            &manifest_hash,
            &aggregate,
            &sha256_hex(&version).to_ascii_uppercase(),
            &attestation_hash,
            revision,
            false,
            &worktree_hash,
        )
        .into_bytes();
        Fixture {
            version,
            manifest,
            attestation,
            statement,
            actual_files,
            executable_hash,
        }
    }

    fn parse(fixture: &Fixture, signature_verified: bool) -> Option<NativeCaptureBuild> {
        parse_trusted_build_provenance(
            &fixture.version,
            &fixture.manifest,
            &fixture.attestation,
            &fixture.statement,
            "mir2-platform-windows.exe",
            1234,
            &fixture.executable_hash,
            &fixture.actual_files,
            signature_verified,
        )
    }

    #[test]
    fn complete_signed_candidate_contract_is_accepted() {
        let fixture = fixture();
        let build = parse(&fixture, true).expect("trusted package");
        assert_eq!(
            build.source_revision.as_deref(),
            Some("78c1e62aed8f7eea82d2a84110a3a3e48c229161")
        );
        assert_eq!(
            build.executable_sha256.as_deref(),
            Some(fixture.executable_hash.as_str())
        );
        let expected_manifest_hash = sha256_hex(&fixture.manifest);
        assert_eq!(
            build.asset_manifest_sha256.as_deref(),
            Some(expected_manifest_hash.as_str())
        );
    }

    #[test]
    fn unsigned_candidate_contract_is_rejected() {
        assert!(parse(&fixture(), false).is_none());
    }

    #[test]
    fn unknown_version_field_is_rejected() {
        let mut fixture = fixture();
        let mut version: serde_json::Value =
            serde_json::from_slice(&fixture.version).expect("version JSON");
        version["operatorDigest"] = json!("forged");
        fixture.version = serde_json::to_vec(&version).expect("modified version");
        assert!(parse(&fixture, true).is_none());
    }

    #[test]
    fn modified_payload_is_rejected_even_when_manifest_json_is_unchanged() {
        let mut fixture = fixture();
        fixture.actual_files[1].sha256 = "B".repeat(64);
        assert!(parse(&fixture, true).is_none());
    }

    #[test]
    fn noncanonical_release_statement_is_rejected() {
        let mut fixture = fixture();
        fixture.statement.push(b'\n');
        assert!(parse(&fixture, true).is_none());
    }

    #[test]
    fn forged_clean_worktree_digest_is_rejected() {
        let fixture = fixture_with_worktree_hash(Some("D".repeat(64)));
        assert!(parse(&fixture, true).is_none());
    }

    #[test]
    fn timestamp_parser_requires_the_attested_dotnet_utc_shape() {
        assert!(valid_utc_timestamp("2024-02-29T23:59:59.1234567+00:00"));
        assert!(!valid_utc_timestamp("2023-02-29T23:59:59.1234567+00:00"));
        assert!(!valid_utc_timestamp("2024-02-29T24:00:00.1234567+00:00"));
        assert!(!valid_utc_timestamp("xxxxxxxxxxxxxxxxxxxZ"));
        assert!(!valid_utc_timestamp("2024-02-29T23:59:59Z"));
    }

    #[test]
    fn tool_version_requires_an_exact_pinned_version_token() {
        assert!(valid_tool_version("cargo 1.95.0 (fixture)", "cargo 1.95.0"));
        assert!(!valid_tool_version("cargo 1.95.0evil", "cargo 1.95.0"));
        assert!(!valid_tool_version(
            "cargo 1.95.1 (fixture)",
            "cargo 1.95.0"
        ));
    }

    #[test]
    fn trusted_thumbprint_is_exact_and_normalized() {
        assert_eq!(
            normalize_thumbprint(" aabbccddeeff00112233445566778899aabbccdd ").as_deref(),
            Some("AABBCCDDEEFF00112233445566778899AABBCCDD")
        );
        assert!(normalize_thumbprint("AABB").is_none());
        assert!(normalize_thumbprint(&"Z".repeat(40)).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_cryptoapi_verifies_the_real_single_signer_fixture() {
        const SIGNATURE: &str = concat!(
            "MIIElgYJKoZIhvcNAQcCoIIEhzCCBIMCAQExDTALBglghkgBZQMEAgEwCwYJKoZIhvcNAQcB",
            "oIIDATCCAv0wggHloAMCAQICCQCKEtOxdfwfWDANBgkqhkiG9w0BAQsFADAnMSUwIwYDVQQD",
            "ExxNaXIyIFByb3ZlbmFuY2UgVGVzdCBGaXh0dXJlMCAXDTIwMDEwMTAwMDAwMFoYDzIwOTkw",
            "MTAxMDAwMDAwWjAnMSUwIwYDVQQDExxNaXIyIFByb3ZlbmFuY2UgVGVzdCBGaXh0dXJlMIIB",
            "IjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArtFuAaJ319f0vnTvRtbLsrwBCch4n36T",
            "lyOwgT4LjlxLgc8S5gilGst/qF+D3cOAPjhDEEs+u/fuT42fTxjAOxdKI66ausFBpc5oAvec",
            "+zK/qEpdrpGZJ9j4TOnOjG12YtI8zpXhx5qj0rOOUs1YAyyaf3YYhFV2As+T83Oqw5QpKYP",
            "CnwT0nY3rmbSS2ADTPdJ3THALMxtWnBzjHm5RebUxwPpYXoBM/OdAbSm3gXVisBYC3Y8YqJ",
            "5UGgbA5Wo0dYmpmY7BwbZrAa9HJyN+5KbliC1qaYtxyUdAwUWYaPEYcxbdPsyL2Bbay120h",
            "B70/ZxjD8qqxk89mT+d2RfqBQIDAQABoyowKDAWBgNVHSUBAf8EDDAKBggrBgEFBQcDAzAO",
            "BgNVHQ8BAf8EBAMCB4AwDQYJKoZIhvcNAQELBQADggEBAGQiTnNcG0FNGHeV4EqNRh+4HctI",
            "nO80FCnDW+qz+bCgCc0uBzboGADAmBSzjCllcZSv2dc/rxUnoriBZfxMk2brXzoK4L/YkRnP",
            "RpAsaFBqo50UWwb00U3VqNV94+Ypg83rGee/ZOqCCvcNjDdBYneOF0GOrRJmH3nLKGQHZgP",
            "+Fh1hDgDUAR7sYJP8jsD8z2fygAR8E/29eBdPV5tR/BClGQGUaYnB5yDlCnVYqp61n3d5M",
            "6f7xTQDOXcSjwszvuBQsFW444lYTQ4s+kj3ew3S39z47L7g7SbzbKBjMj9VafD9SOVym6WJ",
            "8FGcw/F0NffjaenfYnmzPI8/6Vp4tAgxggFbMIIBVwIBATA0MCcxJTAjBgNVBAMTHE1pcjIg",
            "UHJvdmVuYW5jZSBUZXN0IEZpeHR1cmUCCQCKEtOxdfwfWDALBglghkgBZQMEAgEwCwYJKoZI",
            "hvcNAQEBBIIBAH32PtiaQ8aJkZFenLON8ZNoKIB6AAheEormAdbgdxJ27G5KEXIJ2ulIe0XY",
            "4MBfzuDaKPToRsc73wYsi5q8099xq/pw3+Qt87mYzJad/tfhNjmQGpIru6MwqWnoKXkn5aKD",
            "bySuag9K4IKGYs1b09F+NHSeKlACL6QyrNIjWnK3XnWRrM1ErtRFxWobIPtyhD5nQEaROfR",
            "h5Rr99Rnyld5OXVnmi1LnYOdvIMF4HUjwVru9VkNe07kZZWOdHcdFoavg2K/Z1Y0r91ixcR",
            "8W0Vj2nOlUjD6HP+WZ3mbopO19lC43EQBrMNErdiqiZ9GmqlbOIx+RaoayfGdrqjs4xsI="
        );
        const THUMBPRINT: &str = "20E82EF0A1CEBEF469DFD14DC97EAE7B89034C5E";
        let content = b"mir2-provenance-cms-fixture-v1";
        let signature = decode_base64(SIGNATURE);

        assert!(verify_detached_cms_signature(
            content, &signature, THUMBPRINT
        ));
        assert!(!verify_detached_cms_signature(
            b"mir2-provenance-cms-fixture-v2",
            &signature,
            THUMBPRINT
        ));
        assert!(!verify_detached_cms_signature(
            content,
            &signature,
            "00E82EF0A1CEBEF469DFD14DC97EAE7B89034C5E"
        ));
    }

    #[cfg(windows)]
    fn decode_base64(value: &str) -> Vec<u8> {
        let mut output = Vec::with_capacity(value.len() * 3 / 4);
        let mut accumulator = 0u32;
        let mut bits = 0u8;
        for byte in value.bytes().take_while(|byte| *byte != b'=') {
            let sextet = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => panic!("invalid fixture base64"),
            };
            accumulator = (accumulator << 6) | u32::from(sextet);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push((accumulator >> bits) as u8);
                accumulator &= (1u32 << bits) - 1;
            }
        }
        output
    }
}
