//! Authenticated generation mounting and real interactive-login readiness.
//! This path never selects a catalog from an unsigned environment override.
use bevy::prelude::*;
use mir2_resource_pack::PackStore;
use mir2_windows_updater::{fs_safe, generation, generation_runtime, platform::GameGuard};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) struct LaunchContext(Option<PackedLaunch>);

#[derive(Resource)]
struct PackedLaunch {
    // These actual handles quarantine the running generation even if the
    // launcher exits unexpectedly. They live until the app itself is dropped.
    _lease: generation_runtime::GameLease,
    _legacy_guard: GameGuard,
    _store: Arc<PackStore>,
    _generation_directory: fs_safe::DirectoryLease,
    _config: fs_safe::FileLease,
    attempt: String,
    manifest_sha256: String,
    ready_pipe: String,
    ready_sent: bool,
    failed: bool,
}

pub(crate) fn load_and_mount() -> Result<LaunchContext, String> {
    load_inner().map_err(|error| format!("Packed generation could not start: {error:#}"))
}

fn load_inner() -> anyhow::Result<LaunchContext> {
    use anyhow::{ensure, Context};
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !args
        .iter()
        .any(|arg| arg.to_string_lossy().starts_with("--generation-"))
    {
        let exe = std::env::current_exe()?;
        let in_generation = exe
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_some_and(|name| name == "generations");
        ensure!(
            !in_generation && option_env!("MIR2_PACKED_RELEASE") != Some("1"),
            "start this packed release through Mir2Launcher.exe"
        );
        return Ok(LaunchContext(None));
    }
    let mut values = BTreeMap::new();
    ensure!(
        args.len() == 8,
        "complete generation launch context required"
    );
    for pair in args.chunks_exact(2) {
        let key = pair[0].to_str().context("invalid generation argument")?;
        ensure!(
            [
                "--generation-root",
                "--generation-manifest-sha256",
                "--generation-attempt",
                "--generation-ready-pipe"
            ]
            .contains(&key),
            "unknown generation argument"
        );
        ensure!(
            values.insert(key, pair[1].clone()).is_none(),
            "duplicate generation argument"
        );
    }
    let text = |key: &str| -> anyhow::Result<String> {
        Ok(values
            .get(key)
            .context("missing generation argument")?
            .to_str()
            .context("invalid generation argument value")?
            .to_owned())
    };
    let digest = text("--generation-manifest-sha256")?;
    let attempt = text("--generation-attempt")?;
    ensure!(
        mir2_windows_updater::model::valid_hash(&digest),
        "invalid generation digest"
    );
    ensure!(
        mir2_windows_updater::model::valid_hash(&attempt),
        "invalid generation attempt"
    );
    let supplied_root = PathBuf::from(
        values
            .get("--generation-root")
            .context("missing generation root")?,
    );
    fs_safe::ancestors(&supplied_root)?;
    let supplied_root = supplied_root.canonicalize()?;
    ensure!(
        supplied_root
            .file_name()
            .is_some_and(|name| name == digest.to_ascii_lowercase().as_str()),
        "generation directory mismatch"
    );
    let generations = supplied_root.parent().context("generation parent")?;
    ensure!(
        generations
            .file_name()
            .is_some_and(|name| name == "generations"),
        "invalid generation directory"
    );
    let install_root = fs_safe::install_root(generations.parent().context("installation root")?)?;
    // Acquire before authentication or mount: recovery must not race a child
    // whose parent launcher crashed while the child was still starting.
    let lease = generation_runtime::GameLease::acquire(&install_root)?;
    let generation_directory = fs_safe::DirectoryLease::acquire(&supplied_root)?;
    let verified = generation::verify_generation_for_launch(&install_root, &digest)?;
    ensure!(
        verified.root.canonicalize()? == supplied_root,
        "authenticated generation path mismatch"
    );
    ensure!(
        verified.attempt == attempt,
        "generation attempt differs from active selector"
    );
    ensure!(
        std::env::current_exe()?.canonicalize()?
            == supplied_root
                .join(&verified.manifest.exe.path)
                .canonicalize()?,
        "wrong executable for generation"
    );
    let mut legacy_guard = GameGuard::new(&install_root.join("game/mir2-platform-windows.exe"))?;
    ensure!(!legacy_guard.is_running()?, "legacy game is running");
    ensure!(
        legacy_guard.excluded(),
        "legacy executable could not be excluded while packed game runs"
    );
    // NativeSessionConfig will read this exact file after mounting. Retain a
    // compatible read lease rather than trusting an earlier pathname hash.
    let config_path = supplied_root.join(&verified.manifest.signed_config.path);
    let config = fs_safe::FileLease::verified(&config_path, &verified.manifest.signed_config)?;
    let store = Arc::new(PackStore::open(
        install_root.join("objects/packs"),
        &verified.catalog_bytes,
        &verified.manifest.catalog.sha256,
    )?);
    crate::assets::install_packed_store(store.clone()).map_err(anyhow::Error::msg)?;
    crate::capture::install_verified_packed_provenance(crate::capture::VerifiedPackedProvenance {
        generation_manifest_sha256: verified.manifest_sha256.clone(),
        catalog_sha256: verified.manifest.catalog.sha256.clone(),
        game_source_revision: verified.manifest.game_source_revision.clone(),
        content_profile: verified.manifest.content_profile.clone(),
        executable_sha256: verified.manifest.exe.sha256.clone(),
        pack_sha256: verified
            .manifest
            .packs
            .iter()
            .map(|pack| pack.sha256.clone())
            .collect(),
    })
    .map_err(anyhow::Error::msg)?;
    Ok(LaunchContext(Some(PackedLaunch {
        _lease: lease,
        _legacy_guard: legacy_guard,
        _store: store,
        _generation_directory: generation_directory,
        _config: config,
        attempt,
        manifest_sha256: digest,
        ready_pipe: text("--generation-ready-pipe")?,
        ready_sent: false,
        failed: false,
    })))
}

impl LaunchContext {
    pub(crate) fn configure_app(self, app: &mut App) {
        if let Some(context) = self.0 {
            app.insert_resource(context)
                .add_systems(Update, report_interactive_login);
        }
    }
}

fn report_interactive_login(world: &mut World) {
    let Some(context) = world.get_resource::<PackedLaunch>() else {
        return;
    };
    if context.ready_sent || context.failed {
        return;
    }
    if let Some(error) = crate::assets::packed_error() {
        eprintln!("Packed generation resource failure: {error}");
        world.resource_mut::<PackedLaunch>().failed = true;
        world.write_message(AppExit::error());
        return;
    }
    if !crate::assets::packed_login_ready(world) {
        return;
    }
    let mut context = world.resource_mut::<PackedLaunch>();
    match generation_runtime::send_ready(
        &context.ready_pipe,
        &context.attempt,
        &context.manifest_sha256,
    ) {
        Ok(()) => context.ready_sent = true,
        Err(error) => {
            eprintln!("Packed generation readiness handshake failed: {error:#}");
            context.failed = true;
            world.write_message(AppExit::error());
        }
    }
}
