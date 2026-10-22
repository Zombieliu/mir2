//! Read-only installed-file verification. Keep the existing path, hard-link and
//! complete SHA256 checks; only independent files run concurrently.
use super::Status;
use crate::{fs_safe as safe, local_io, model::FileEntry};
use anyhow::{ensure, Result};
use std::path::Path;

/// Return pending indexes in manifest order. Neither workers nor completion
/// callbacks write the installation or start the next update phase.
pub(super) fn scan(
    game_dir: &Path,
    files: &[FileEntry],
    status: &impl Status,
    observed: impl FnMut(usize),
) -> Result<Vec<usize>> {
    scan_with_workers(
        game_dir,
        files,
        status,
        local_io::workers(files.len()),
        observed,
    )
}

// A private worker override lets unit tests and the opt-in read-only benchmark
// compare the very same verifier without adding a production tuning setting.
fn scan_with_workers(
    game_dir: &Path,
    files: &[FileEntry],
    status: &impl Status,
    worker_count: usize,
    mut observed: impl FnMut(usize),
) -> Result<Vec<usize>> {
    ensure!(!status.cancelled(), "update cancelled");
    ensure!(game_dir.is_absolute(), "relative game directory");
    let checked = local_io::try_run(
        files.iter().enumerate().map(|(index, entry)| {
            ensure!(!status.cancelled(), "update cancelled");
            Ok((index, entry))
        }),
        worker_count,
        |(index, entry)| {
            let matches = safe::matches(&safe::target(game_dir, &entry.path)?, entry)?;
            Ok((index, matches))
        },
        |_, completed| {
            ensure!(!status.cancelled(), "update cancelled");
            observed(completed);
            ensure!(!status.cancelled(), "update cancelled");
            Ok(())
        },
    )?;
    // try_run joins every worker before success, failure or cancellation returns.
    ensure!(!status.cancelled(), "update cancelled");
    Ok(checked
        .into_iter()
        .filter_map(|(index, matches)| (!matches).then_some(index))
        .collect())
}

#[cfg(test)]
#[path = "installed_scan_tests.rs"]
mod tests;
