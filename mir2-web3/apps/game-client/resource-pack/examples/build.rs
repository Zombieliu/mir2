//! Offline create-only builder. Input inventory and any previous catalog digest
//! must come from the caller's authenticated build/release workflow.
use anyhow::{ensure, Context, Result};
use mir2_resource_pack::{
    build, write_catalog_new, BuildOptions, PackStore, ResourceInput, MAX_CATALOG_BYTES,
};
use std::{fs, path::PathBuf};

fn bounded(path: &std::path::Path) -> Result<Vec<u8>> {
    ensure!(path.is_absolute(), "absolute input path required");
    ensure!(
        fs::metadata(path)?.len() <= MAX_CATALOG_BYTES as u64,
        "input byte bound"
    );
    let bytes = fs::read(path)?;
    ensure!(bytes.len() <= MAX_CATALOG_BYTES, "input grew beyond bound");
    Ok(bytes)
}

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(args.len() == 4 || args.len() == 6,
        "usage: build INVENTORY.json SOURCE_ROOT OBJECTS_ROOT NEW_CATALOG.json [PREVIOUS_CATALOG.json TRUSTED_PREVIOUS_SHA256]");
    let inventory = PathBuf::from(&args[0]);
    let source = PathBuf::from(&args[1]);
    let objects = PathBuf::from(&args[2]);
    let output = PathBuf::from(&args[3]);
    ensure!(output.is_absolute(), "absolute new catalog path required");
    ensure!(!output.try_exists()?, "fresh catalog output required");
    let inputs: Vec<ResourceInput> = serde_json::from_slice(&bounded(&inventory)?)?;
    let previous = if args.len() == 6 {
        let bytes = bounded(&PathBuf::from(&args[4]))?;
        Some(PackStore::open(
            &objects,
            &bytes,
            args[5].to_str().context("SHA encoding")?,
        )?)
    } else {
        None
    };
    let result = build(
        &source,
        &objects,
        &inputs,
        previous.as_ref(),
        BuildOptions::default(),
    )?;
    write_catalog_new(&output, &result.catalog_bytes, &result.catalog_sha256)?;
    println!(
        "catalogSha256={} resources={} requiredPacks={} newPacks={} reusedResources={}",
        result.catalog_sha256,
        result.catalog.resources.len(),
        result.catalog.packs.len(),
        result.new_packs.len(),
        result.reused_resources
    );
    Ok(())
}
