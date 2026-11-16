//! Offline full-inventory repacker. The caller authenticates TRUSTED_OLD_SHA;
//! no signing, network, extraction, generation activation, or partial mount.
//!
//! Plan (groups are prioritized in array order, default is implicit and last):
//! {"schema":"mir2.native.resource-repack-plan.v1",
//!  "sourceCatalogSha256":"<trusted old catalog SHA256>",
//!  "targetPackBytes":8388608,
//!  "groups":[{"name":"login","paths":["original-ui/Title/320.png"]}]}
//! All logical paths survive; aliases live in their earliest owning group.
//! Output parent must exist, output itself must not exist. Failed output remains.
use anyhow::{ensure, Context, Result};
use mir2_resource_pack::repack::repack_files;
use std::path::Path;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ensure!(args.len() == 5,
        "usage: repack OLD_CATALOG.json TRUSTED_OLD_SHA256 OLD_OBJECTS_ROOT PLAN.json FRESH_OUTPUT_ROOT");
    let receipt = repack_files(
        Path::new(&args[0]),
        args[1].to_str().context("trusted SHA encoding")?,
        Path::new(&args[2]),
        Path::new(&args[3]),
        Path::new(&args[4]),
    )?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
