#![cfg_attr(windows, windows_subsystem = "windows")]
use anyhow::{ensure, Context, Result};
use mir2_windows_updater::{
    fs_safe as safe, generation,
    generation_runtime::{GameLease, ReadyServer},
    platform, transaction, ui, update,
};
use std::{
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
struct Status {
    window: ui::ProgressWindow,
    locale: String,
}
impl update::Status for Status {
    fn set(&self, key: &str, percent: u32) {
        self.window.update(&ui::stage(&self.locale, key), percent);
    }
    fn cancelled(&self) -> bool {
        self.window.cancelled()
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
            drop(launch_lease);
            let mut child = Command::new(legacy_exe)
                .current_dir(root.join("game"))
                .spawn()?;
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
fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let scratch = exe.parent().context("scratch engine directory")?;
    ensure!(
        scratch.file_name().is_some_and(|s| s == ".update"),
        "start the game through Mir2Launcher.exe"
    );
    let root = safe::install_root(scratch.parent().context("installation directory")?)?;
    let _lock = platform::InstallLock::acquire(&root).map_err(anyhow::Error::msg)?;
    ensure!(
        !platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is running"
    );
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    ensure!(
        args.is_empty() || args == ["--check-only"],
        "unsupported updater argument"
    );
    let status = Status {
        locale: mir2_windows_updater::locale(),
        window: ui::ProgressWindow::spawn(&mir2_windows_updater::locale()),
    };
    let packed_before = generation::active_record(&root)?;
    let mut retained_legacy = false;
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
                    Some(source) => update::engine_maintenance::check(&root, &source, &status),
                    None => Ok(None),
                }
            })();
            match maintenance {
                Ok(Some(handoff)) => {
                    status.window.close();
                    drop(activation_lease);
                    drop(_lock);
                    match handoff.spawn(args == ["--check-only"]) {
                        Ok(mut child) => {
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
                Err(error) => update::log(&root, "engine-maintenance-error", format!("{error:#}")),
            }
        }
        let packed = if recovered {
            true
        } else {
            match generation::check_update(&root, &packed_source, &status) {
                Ok(value) => value.is_some(),
                Err(error) => {
                    update::log(&root, "packed-update-error", format!("{error:#}"));
                    // Continue the same authenticated healthy selector. A
                    // LegacyV4 selection exists only after explicit rollback;
                    // this branch never discovers or downloads a v4 release.
                    if generation::verified_healthy_selection(&root)?.is_some() {
                        status
                            .window
                            .update(&ui::stage(&status.locale, "offline"), 100);
                        true
                    } else if generation::retain_verified_legacy_after_rejected_feed(&root, &error)?
                    {
                        // A newer signed installer may legitimately see the old
                        // pack release line. Keep precisely its verified game;
                        // never feed that decision into v4 network discovery.
                        retained_legacy = true;
                        update::log(&root, "packed-below-legacy-floor-retained", "verified installed legacy; no discovery retry or selector/floor writes");
                        status
                            .window
                            .update(&ui::stage(&status.locale, "offline"), 100);
                        false
                    } else {
                        return Err(error);
                    }
                }
            }
        };
        if packed {
            status.window.close();
            if args == ["--check-only"] {
                return Ok(());
            }
            if status.window.cancelled() {
                return Ok(());
            }
            drop(activation_lease);
            return launch_selected(&root, &status);
        }
        drop(activation_lease);
    }
    let mut activated = transaction::pending_activation(&root)?;
    if !retained_legacy {
        let source = update::HttpsSource::new()?;
        match update::check_update(&root, &source, &status) {
            Ok(outcome) => {
                activated = outcome.activated;
                update::log(&root, "update", outcome);
            }
            Err(error) => {
                update::log(&root, "update-error", format!("{error:#}"));
                transaction::recover(&root).context("recovery failed; launch prevented")?;
                if status.window.cancelled() {
                    status.window.close();
                    return Ok(());
                }
                update::verify_launch(&root).context("installed fallback is not valid")?;
                status
                    .window
                    .update(&ui::stage(&status.locale, "offline"), 100);
            }
        }
    }
    if args == ["--check-only"] {
        status.window.close();
        return Ok(());
    }
    update::verify_launch(&root)?;
    if status.window.cancelled() {
        status.window.close();
        return Ok(());
    }
    let start = || {
        Command::new(root.join("game/mir2-platform-windows.exe"))
            .current_dir(root.join("game"))
            .spawn()
    };
    let mut exe_lease = Some(safe::FileLease::verified(
        &root.join("game/mir2-platform-windows.exe"),
        &update::launch_executable_entry(&root)?,
    )?);
    let mut child = match start() {
        Ok(child) => child,
        Err(error) => {
            drop(exe_lease.take());
            if activated {
                update::quarantine_and_rollback(&root)?;
            }
            status.window.close();
            return Err(error)
                .context("Windows could not start the game; update restored where available");
        }
    };
    status.window.close();
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
    // Keep the per-install update lock for the child's entire lifetime.
    let exit = child.wait()?;
    drop(exe_lease);
    update::log(&root, "game-exit", serde_json::json!({"code":exit.code()}));
    Ok(())
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
