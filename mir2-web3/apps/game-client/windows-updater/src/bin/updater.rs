#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{
    fs_safe as safe, generation,
    generation_runtime::{GameLease, ReadyServer},
    platform, transaction, ui, update,
};
use std::{
    cell::Cell,
    path::Path,
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
struct Status {
    window: ui::ProgressWindow,
    locale: String,
    interactive: bool,
    fallback: Cell<bool>,
    retry_requested: Cell<bool>,
    launch_committed: Cell<bool>,
    installed_available: Cell<bool>,
    play_installed_requested: Cell<bool>,
}
impl update::Status for Status {
    fn set(&self, key: &str, percent: u32) {
        self.window.set_stage(key, percent);
    }
    fn cancelled(&self) -> bool {
        self.update_cancelled()
    }
    fn confirm_update(&self, version: &str, download_bytes: u64) -> Result<()> {
        if self.interactive {
            match self.window.confirm_update_choice(
                &self.locale,
                version,
                download_bytes,
                self.installed_available.get(),
            ) {
                ui::LauncherAction::Proceed => {}
                ui::LauncherAction::PlayInstalled if self.installed_available.get() => {
                    self.play_installed_requested.set(true);
                    anyhow::bail!("player chose the verified installed game");
                }
                _ => anyhow::bail!("update cancelled"),
            }
        }
        Ok(())
    }
    fn transfer(&self, received: u64, total: u64) {
        self.window.transfer(received, total);
    }
}
impl Status {
    fn update_cancelled(&self) -> bool {
        self.window.cancelled() || self.play_installed_requested.get()
    }
    fn commit_launch(&self) -> Result<bool> {
        if self.launch_committed.get() {
            return Ok(true);
        }
        let accepted = self.window.begin_launch();
        ensure!(
            accepted || self.window.cancelled(),
            "Windows launcher did not respond before game start"
        );
        self.launch_committed.set(accepted);
        Ok(accepted)
    }
    fn confirm_play(&self, root: &Path) -> Result<bool> {
        if !self.interactive {
            return Ok(true);
        }
        let version = match generation::active_record(root)? {
            Some(generation::ActiveRecord {
                active: generation::Selection::Generation { sequence, .. },
                ..
            }) => format!("r{sequence}"),
            _ => update::verify_launch(root)?,
        };
        match self
            .window
            .ready(&self.locale, &version, self.fallback.get())
        {
            ui::LauncherAction::Proceed | ui::LauncherAction::PlayInstalled => {
                self.window.set_stage("launching", 100);
                Ok(true)
            }
            ui::LauncherAction::Retry => {
                self.retry_requested.set(true);
                Ok(false)
            }
            ui::LauncherAction::Exit => Ok(false),
        }
    }
}
fn wait_healthy(child: &mut Child) -> Result<bool> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if child.try_wait()?.is_some() {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Ok(true)
}
#[derive(Debug, PartialEq, Eq)]
enum HealthObservation {
    Pending,
    Healthy,
    Failed,
}
struct PackedHealth {
    started: Instant,
    first_ready: Option<Instant>,
}
impl PackedHealth {
    fn observe(&mut self, now: Instant, exited: bool, ready: bool) -> HealthObservation {
        if exited || now.duration_since(self.started) >= Duration::from_secs(120) {
            return HealthObservation::Failed;
        }
        if ready {
            self.first_ready.get_or_insert(now);
        }
        if self
            .first_ready
            .is_some_and(|first| now.duration_since(first) >= Duration::from_secs(10))
        {
            HealthObservation::Healthy
        } else {
            HealthObservation::Pending
        }
    }
}
fn packed_health(child: &mut Child, ready: &mut ReadyServer) -> Result<bool> {
    let mut health = PackedHealth {
        started: Instant::now(),
        first_ready: None,
    };
    loop {
        let now = Instant::now();
        let exited = child.try_wait()?.is_some();
        let received = if !exited && now.duration_since(health.started) < Duration::from_secs(120) {
            ready.poll(child.id())?
        } else {
            false
        };
        match health.observe(Instant::now(), exited, received) {
            HealthObservation::Healthy => return Ok(true),
            HealthObservation::Failed => return Ok(false),
            HealthObservation::Pending => {}
        }
        thread::sleep(Duration::from_millis(50));
    }
}
fn launch_selected(root: &std::path::Path, status: &Status) -> Result<()> {
    let launch_lease = GameLease::acquire(root)?;
    let record = generation::active_record(root)?.context("packed selector missing")?;
    match &record.active {
        generation::Selection::LegacyV4 { exe_sha256, .. } => {
            // This is an explicit authenticated health rollback, never fallback
            // to a v4 network feed after a packed protocol/integrity failure.
            generation::verify_current_selection(root)?;
            let legacy_exe = root.join("game/mir2-platform-windows.exe");
            let _exe_lease = safe::FileLease::verified(
                &legacy_exe,
                &mir2_windows_updater::model::FileEntry {
                    path: "mir2-platform-windows.exe".into(),
                    size: safe::regular(&legacy_exe)?.len(),
                    sha256: exe_sha256.clone(),
                },
            )?;
            if !status.commit_launch()? {
                return Ok(());
            }
            drop(launch_lease);
            let mut child = Command::new(legacy_exe)
                .current_dir(root.join("game"))
                .spawn()?;
            status.window.close();
            let exit = child.wait()?;
            update::log(
                root,
                "game-exit",
                serde_json::json!({"legacyRollback":true,"code":exit.code()}),
            );
            Ok(())
        }
        generation::Selection::Generation {
            manifest_sha256, ..
        } => {
            let verified = generation::verify_generation_for_launch(root, manifest_sha256)?;
            let _exe_lease = safe::FileLease::verified(
                &verified.root.join(&verified.manifest.exe.path),
                &verified.manifest.exe,
            )?;
            let mut ready = ReadyServer::new(&verified.attempt, &verified.manifest_sha256)?;
            let userdata = safe::target(root, "userdata")?;
            std::fs::create_dir_all(&userdata)?;
            safe::ancestors(&userdata)?;
            if !status.commit_launch()? {
                return Ok(());
            }
            generation::begin_launch(root, &verified.attempt)?;
            drop(launch_lease);
            let mut child = Command::new(verified.root.join(&verified.manifest.exe.path))
                .current_dir(&userdata)
                .arg("--generation-root")
                .arg(&verified.root)
                .arg("--generation-manifest-sha256")
                .arg(&verified.manifest_sha256)
                .arg("--generation-attempt")
                .arg(&verified.attempt)
                .arg("--generation-ready-pipe")
                .arg(ready.pipe_name())
                .spawn();
            let health = match child.as_mut() {
                Ok(child) => packed_health(child, &mut ready),
                Err(error) => Err(anyhow::anyhow!("could not start packed game: {error}")),
            };
            if !matches!(health, Ok(true)) {
                if let Ok(child) = child.as_mut() {
                    if child.try_wait()?.is_none() {
                        child.kill()?;
                    }
                    child.wait()?;
                }
                if record.health == generation::Health::Pending {
                    // The actual child exited before acquiring this lease.
                    // A live game or orphaned child must prevent rollback.
                    let rollback_lease = GameLease::acquire(root)?;
                    generation::rollback(root, &record.attempt)?;
                    drop(rollback_lease);
                    update::log(
                        root,
                        "packed-launch-rollback",
                        serde_json::json!({"manifest":manifest_sha256,"attempt":record.attempt,"readyAndAlive":false}),
                    );
                    ui::show_error(&status.locale, &ui::stage(&status.locale, "recovered"));
                    return launch_selected(root, status);
                }
                return Err(health.err().unwrap_or_else(|| {
                    anyhow::anyhow!(
                        "packed game did not report ready and remain alive for ten seconds"
                    )
                }));
            }
            if record.health == generation::Health::Pending {
                generation::accept(root, &record.attempt)?;
            }
            status.window.close();
            let mut child = child?;
            let exit = child.wait()?;
            update::log(
                root,
                "game-exit",
                serde_json::json!({"generation":manifest_sha256,"code":exit.code()}),
            );
            Ok(())
        }
    }
}
/// Recovery and local verification have no network dependency. This short
/// critical section ends before Home, including while its optional probe waits
/// for HTTP. A failed recovery is fatal; a damaged installation can show Home
/// with play disabled and still offer the explicit repair/update action.
fn recover_local(root: &Path) -> Result<()> {
    ensure!(
        !platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is running"
    );
    let _idle = GameLease::acquire(root)?;
    let packed = generation::active_record(root)?;
    let legacy_pending = transaction::pending_activation(root)?;
    ensure!(
        !(packed.is_some() && legacy_pending),
        "packed and legacy pending states conflict"
    );
    if !legacy_pending {
        generation::recover(root)?;
    }
    transaction::recover(root).context("local recovery failed; launch prevented")?;
    Ok(())
}
fn prepare_home(root: &Path) -> Result<Option<String>> {
    let _lock = platform::InstallLock::acquire(root).map_err(anyhow::Error::msg)?;
    recover_local(root)?;
    match update::verify_installed(root) {
        Ok(version) => Ok(Some(version)),
        Err(error) => {
            update::log(root, "installed-verification-error", format!("{error:#}"));
            Ok(None)
        }
    }
}
struct ProbeStatus(Arc<AtomicBool>);
impl update::Status for ProbeStatus {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
struct BackgroundProbe(Arc<AtomicBool>);
impl Drop for BackgroundProbe {
    fn drop(&mut self) {
        // A pending HTTP read is deliberately never joined by the play path.
        // Subsequent metadata requests/callbacks stop once Home is left.
        self.0.store(true, Ordering::Relaxed);
    }
}
fn background_probe(root: &Path, status: &Status) -> BackgroundProbe {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let root = root.to_owned();
    let window = status.window.clone();
    let locale = status.locale.clone();
    let spawned = thread::Builder::new()
        .name("mir2-optional-update-probe".into())
        .spawn(move || {
            let status = ProbeStatus(worker_stop);
            let result = (|| -> Result<update::Probe> {
                if let Some(source) = update::engine_maintenance::probe_network_source(&root)? {
                    if let Some(probe) = update::engine_maintenance::probe(&root, &source, &status)?
                    {
                        if probe.update_available {
                            return Ok(probe);
                        }
                    }
                }
                ensure!(
                    !status.0.load(Ordering::Relaxed),
                    "optional check cancelled"
                );
                let source = update::HttpsSource::new_packed()?;
                if let Some(probe) = generation::probe_update(&root, &source, &status)? {
                    return Ok(probe);
                }
                let source = update::HttpsSource::new()?;
                update::probe_update(&root, &source, &status)
            })();
            if status.0.load(Ordering::Relaxed) {
                return;
            }
            match result {
                Ok(probe) => window.probe_result(
                    &locale,
                    probe.update_available.then_some(probe.version.as_str()),
                    false,
                ),
                Err(_) => window.probe_result(&locale, None, true),
            }
        });
    if spawned.is_err() {
        status.window.probe_result(&status.locale, None, true);
    }
    BackgroundProbe(stop)
}
fn run_installed_attempt(root: &Path, status: &Status) -> Result<()> {
    let _lock = platform::InstallLock::acquire(root).map_err(anyhow::Error::msg)?;
    recover_local(root)?;
    update::verify_installed(root).context("installed game changed; launch prevented")?;
    if status.window.cancelled() {
        return Ok(());
    }
    status.window.set_stage("launching", 100);
    if generation::active_record(root)?.is_some() {
        launch_selected(root, status)
    } else {
        launch_legacy(root, status, transaction::pending_activation(root)?)
    }
}
fn run_attempt(root: &Path, args: &[String], status: &Status) -> Result<()> {
    // --check-only is a packaging/tooling operation, retaining its established
    // download, activation and pending-health contract without a Home dialog.
    if !status.interactive {
        return run_update_attempt(root, args, status);
    }
    let version = prepare_home(root)?;
    status.installed_available.set(version.is_some());
    status.window.installed_available(version.is_some());
    let probe = background_probe(root, status);
    let action = status.window.home(
        &status.locale,
        version.as_deref().unwrap_or_default(),
        version.is_some(),
    );
    drop(probe);
    match action {
        ui::LauncherAction::PlayInstalled if version.is_some() => {
            run_installed_attempt(root, status)
        }
        ui::LauncherAction::Proceed | ui::LauncherAction::Retry => {
            let outcome = run_update_attempt(root, args, status);
            if status.play_installed_requested.replace(false) {
                // All update locks, leases and stack frames are gone before the
                // freshly revalidated local process starts. No later network
                // fallback or second confirmation is needed for this choice.
                run_installed_attempt(root, status)
            } else {
                outcome
            }
        }
        _ => Ok(()),
    }
}
fn run_update_attempt(root: &Path, args: &[String], status: &Status) -> Result<()> {
    let _lock = platform::InstallLock::acquire(root).map_err(anyhow::Error::msg)?;
    ensure!(
        !platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is running"
    );
    let packed_before = generation::active_record(&root)?;
    let legacy_pending = transaction::pending_activation(&root)?;
    ensure!(
        !(packed_before.is_some() && legacy_pending),
        "packed and legacy pending states conflict"
    );
    if !legacy_pending {
        let activation_lease = GameLease::acquire(&root)?;
        let packed_source = update::HttpsSource::new_packed()?;
        let recovered = generation::recover(&root)?;
        if !recovered {
            let maintenance = (|| -> Result<_> {
                match update::engine_maintenance::network_source(&root)? {
                    Some(source) => update::engine_maintenance::check(&root, &source, status),
                    None => Ok(None),
                }
            })();
            match maintenance {
                Ok(Some(handoff)) => {
                    if status.interactive && !status.window.begin_launch() {
                        return Ok(());
                    }
                    drop(activation_lease);
                    drop(_lock);
                    match handoff.spawn(args == ["--check-only"]) {
                        Ok(mut child) => {
                            status.window.close();
                            let exit = child.wait()?;
                            update::log(
                                &root,
                                "engine-maintenance-handoff-exit",
                                serde_json::json!({"engineSha256":handoff.engine_sha256,"code":exit.code(),"automaticRollback":"OS spawn failure only"}),
                            );
                            ensure!(exit.success(),"verified engine exited unsuccessfully; engine health requires release validation");
                            return Ok(());
                        }
                        Err(error) => {
                            let _rollback_lock = platform::InstallLock::acquire(&root)
                                .map_err(anyhow::Error::msg)?;
                            let _rollback_idle = GameLease::acquire(&root)?;
                            update::engine_maintenance::failed_spawn(&root, &handoff)?;
                            return Err(error).context("Windows could not spawn verified engine; previous pointer restored");
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    if status.update_cancelled() {
                        return Ok(());
                    }
                    update::log(&root, "engine-maintenance-error", format!("{error:#}"));
                    status.fallback.set(true);
                }
            }
        }
        let packed = if recovered {
            true
        } else {
            match generation::check_update(&root, &packed_source, status) {
                Ok(value) => value.is_some(),
                Err(error) => {
                    if status.update_cancelled() {
                        return Ok(());
                    }
                    update::log(&root, "packed-update-error", format!("{error:#}"));
                    // Continue the same authenticated healthy selector. A
                    // LegacyV4 selection exists only after explicit rollback;
                    // this branch never discovers or downloads a v4 release.
                    if generation::verified_healthy_selection(&root)?.is_some() {
                        status.fallback.set(true);
                        true
                    } else {
                        return Err(error);
                    }
                }
            }
        };
        if packed {
            if args == ["--check-only"] {
                status.window.close();
                return Ok(());
            }
            if status.update_cancelled() {
                return Ok(());
            }
            if !status.confirm_play(root)? {
                return Ok(());
            }
            drop(activation_lease);
            return launch_selected(&root, status);
        }
        drop(activation_lease);
    }
    let source = update::HttpsSource::new()?;
    let mut activated = transaction::pending_activation(&root)?;
    match update::check_update(&root, &source, status) {
        Ok(outcome) => {
            activated = outcome.activated;
            update::log(&root, "update", outcome);
        }
        Err(error) => {
            update::log(&root, "update-error", format!("{error:#}"));
            transaction::recover(&root).context("recovery failed; launch prevented")?;
            if status.update_cancelled() {
                if !status.play_installed_requested.get() {
                    status.window.close();
                }
                return Ok(());
            }
            update::verify_launch(&root).context("installed fallback is not valid")?;
            status.fallback.set(true);
        }
    }
    if args == ["--check-only"] {
        status.window.close();
        return Ok(());
    }
    update::verify_launch(&root)?;
    if status.update_cancelled() {
        if !status.play_installed_requested.get() {
            status.window.close();
        }
        return Ok(());
    }
    if !status.confirm_play(root)? {
        return Ok(());
    }
    launch_legacy(root, status, activated)
}
fn launch_legacy(root: &Path, status: &Status, activated: bool) -> Result<()> {
    let start = || {
        Command::new(root.join("game/mir2-platform-windows.exe"))
            .current_dir(root.join("game"))
            .spawn()
    };
    let mut exe_lease = Some(safe::FileLease::verified(
        &root.join("game/mir2-platform-windows.exe"),
        &update::launch_executable_entry(&root)?,
    )?);
    if !status.commit_launch()? {
        return Ok(());
    }
    let mut child = match start() {
        Ok(child) => child,
        Err(error) => {
            drop(exe_lease.take());
            if activated {
                update::quarantine_and_rollback(&root)?;
            }
            return Err(error)
                .context("Windows could not start the game; update restored where available");
        }
    };
    if activated {
        if !wait_healthy(&mut child)? {
            update::log(
                &root,
                "launch-rollback",
                "new game exited unsuccessfully within ten seconds",
            );
            drop(exe_lease.take());
            update::quarantine_and_rollback(&root)?;
            update::verify_launch(&root)?;
            exe_lease = Some(safe::FileLease::verified(
                &root.join("game/mir2-platform-windows.exe"),
                &update::launch_executable_entry(&root)?,
            )?);
            ui::show_error(&status.locale, &ui::stage(&status.locale, "recovered"));
            child = start().context("previous game could not start")?;
        } else {
            transaction::accept(&root)?;
        }
    }
    status.window.close();
    // Keep the per-install update lock for the child's entire lifetime.
    let exit = child.wait()?;
    drop(exe_lease);
    update::log(&root, "game-exit", serde_json::json!({"code":exit.code()}));
    Ok(())
}
fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let scratch = exe.parent().context("scratch engine directory")?;
    ensure!(
        scratch.file_name().is_some_and(|s| s == ".update"),
        "start the game through Mir2Launcher.exe"
    );
    let root = safe::install_root(scratch.parent().context("installation directory")?)?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.is_empty() || args == ["--check-only"],
        "unsupported updater argument"
    );
    loop {
        let locale = mir2_windows_updater::locale();
        let status = Status {
            window: ui::ProgressWindow::spawn(&locale),
            locale,
            interactive: args.is_empty(),
            fallback: Cell::new(false),
            retry_requested: Cell::new(false),
            launch_committed: Cell::new(false),
            installed_available: Cell::new(false),
            play_installed_requested: Cell::new(false),
        };
        ensure!(
            status.window.is_open(),
            "Windows could not open the game launcher window"
        );
        match run_attempt(&root, &args, &status) {
            Ok(()) if status.retry_requested.get() => continue,
            Ok(()) => return Ok(()),
            Err(_) if status.window.cancelled() => return Ok(()),
            Err(error) => {
                update::log(&root, "engine-error", format!("{error:#}"));
                if !status.interactive || !status.window.is_open() {
                    return Err(error);
                }
                if !status.window.retry(&status.locale, &format!("{error:#}")) {
                    return Ok(());
                }
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        let locale = mir2_windows_updater::locale();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(root) = exe.parent().and_then(|p| p.parent()) {
                update::log(root, "engine-error", format!("{error:#}"));
            }
        }
        ui::show_error(
            &locale,
            &format!("{}\n\n{error:#}", ui::stage(&locale, "failed")),
        );
        std::process::exit(1);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn home_preparation_disables_invalid_game_and_releases_its_local_locks() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("game")).unwrap();
        std::fs::write(
            root.path().join("game/mir2-platform-windows.exe"),
            b"unsigned",
        )
        .unwrap();
        assert_eq!(prepare_home(root.path()).unwrap(), None);
        assert!(!root.path().join("updater/active.txt").exists());
        assert!(!root.path().join("state/active-generation.json").exists());
        assert!(!root.path().join(".update/transaction.json").exists());
        let path = root.path().to_owned();
        let (can_lock, can_lease) = thread::spawn(move || {
            let lock = platform::InstallLock::acquire(&path);
            let lease = GameLease::acquire(&path);
            (lock.is_ok(), lease.is_ok())
        })
        .join()
        .unwrap();
        assert!(can_lock, "Home retained the update write lock");
        assert!(can_lease, "Home retained the exclusive game lease");
    }
    #[test]
    fn late_interactive_ready_requires_its_own_ten_seconds_and_cannot_extend_deadline() {
        let start = Instant::now();
        let mut health = PackedHealth {
            started: start,
            first_ready: None,
        };
        assert_eq!(
            health.observe(start + Duration::from_secs(14), false, false),
            HealthObservation::Pending
        );
        assert_eq!(
            health.observe(start + Duration::from_secs(15), false, true),
            HealthObservation::Pending
        );
        assert_eq!(
            health.observe(start + Duration::from_secs(24), false, true),
            HealthObservation::Pending
        );
        assert_eq!(
            health.observe(start + Duration::from_secs(25), false, true),
            HealthObservation::Healthy
        );
        assert_eq!(health.first_ready, Some(start + Duration::from_secs(15)));
        assert_eq!(
            health.observe(start + Duration::from_secs(25), true, true),
            HealthObservation::Failed
        );
        let mut late = PackedHealth {
            started: start,
            first_ready: None,
        };
        assert_eq!(
            late.observe(start + Duration::from_secs(119), false, true),
            HealthObservation::Pending
        );
        assert_eq!(
            late.observe(start + Duration::from_secs(120), false, true),
            HealthObservation::Failed
        );
    }

    #[test]
    fn a_successful_early_process_exit_is_not_legacy_first_launch_health() {
        let mut child = Command::new("cmd.exe")
            .args(["/d", "/c", "exit", "0"])
            .spawn()
            .unwrap();
        assert!(!wait_healthy(&mut child).unwrap());
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn early_zero_exit_without_authenticated_ready_is_not_generation_health() {
        let attempt = "A".repeat(64);
        let mut ready = ReadyServer::new(&attempt, &"B".repeat(64)).unwrap();
        let mut child = Command::new("cmd.exe")
            .args(["/d", "/c", "exit", "0"])
            .spawn()
            .unwrap();
        assert!(!packed_health(&mut child, &mut ready).unwrap());
        assert!(child.wait().unwrap().success());
    }
}
