//! Full offline verification; never signs, downloads, launches or activates.
use anyhow::{ensure, Context, Result};
use mir2_resource_pack::{sha256, PackStore, MAX_CATALOG_BYTES};
use std::{
    collections::BTreeMap,
    fs,
    fs::File,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn no_links(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "absolute verification path required");
    for part in path.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(part) {
            ensure!(!meta.file_type().is_symlink(), "linked verification path");
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                ensure!(
                    meta.file_attributes() & 0x400 == 0,
                    "reparse verification path"
                );
            }
        }
    }
    Ok(())
}
fn plain_handle(file: &File) -> Result<()> {
    ensure!(
        file.metadata()?.is_file(),
        "plain verification file required"
    );
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } != 0,
            "verification handle information"
        );
        ensure!(
            info.nNumberOfLinks == 1 && info.dwFileAttributes & 0x400 == 0,
            "linked verification handle"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            file.metadata()?.nlink() == 1,
            "hardlinked verification handle"
        );
    }
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(
        args.len() == 3 || args.len() == 4,
        "usage: verify OBJECTS_ROOT CATALOG.json EXPECTED_CATALOG_SHA256 [FRESH_REPORT.json]"
    );
    let objects = PathBuf::from(&args[0]);
    let catalog = PathBuf::from(&args[1]);
    no_links(&catalog)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(&catalog)?;
    plain_handle(&file)?;
    let length = file.metadata()?.len();
    ensure!(length <= MAX_CATALOG_BYTES as u64, "catalog byte bound");
    let mut bytes = Vec::with_capacity(length as usize);
    (&file)
        .take(MAX_CATALOG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 == length, "catalog changed during read");
    let expected = args[2].to_str().context("catalog SHA encoding")?;
    let store = PackStore::open(objects, &bytes, expected)?;
    let mut unique = BTreeMap::new();
    let mut logical = 0u64;
    for (path, entry) in &store.catalog().resources {
        let bytes = store.read(path)?;
        ensure!(
            bytes.len() as u64 == entry.size && sha256(&bytes) == entry.sha256,
            "logical resource mismatch"
        );
        unique.insert(entry.sha256.clone(), entry.size);
        logical += entry.size;
    }
    let report = serde_json::json!({
        "schema":"mir2.native.resource-pack-verification.v1", "passed":true,
        "actualFullVerification":true, "metadataOnly":false,
        "catalogSha256":store.catalog_sha256(), "resourceCount":store.catalog().resources.len(),
        "uniquePayloads":unique.len(), "logicalBytes":logical,
        "uniquePayloadBytes":unique.values().sum::<u64>(),
        "packCount":store.catalog().packs.len(), "packBytes":store.catalog().packs.iter().map(|p|p.size).sum::<u64>()
    });
    if args.len() == 4 {
        let path = PathBuf::from(&args[3]);
        no_links(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
            options
                .share_mode(0)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        let mut output = options.open(path)?;
        plain_handle(&output)?;
        serde_json::to_writer_pretty(&mut output, &report)?;
        output.write_all(b"\n")?;
        output.sync_all()?;
        plain_handle(&output)?;
    }
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
