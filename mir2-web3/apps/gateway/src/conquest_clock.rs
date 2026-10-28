//! Siege scheduling belongs to the server lifetime, including an empty server.
use crate::{GatewayConfig, SharedInProcessZoneRuntimeFactory};
use rand::{rngs::OsRng, RngCore};
use std::io;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CONQUEST_CLOCK_CADENCE: Duration = Duration::from_secs(10);
const CONQUEST_ERROR_REPEAT: Duration = Duration::from_secs(60);

pub struct ConquestClockService {
    stop: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl ConquestClockService {
    pub fn start(
        config: GatewayConfig,
        factory: Arc<SharedInProcessZoneRuntimeFactory>,
    ) -> io::Result<Self> {
        // This token remains unchanged throughout this service's lifetime. It
        // fences a second process clock without depending on a player session.
        let clock_owner = new_clock_owner()?;
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = stop.clone();
        let worker = std::thread::Builder::new()
            .name("shared-conquest-clock".into())
            .spawn(move || {
                let started = Instant::now();
                let mut diagnostics = ClockDiagnostics::default();
                loop {
                    match signal.0.lock() {
                        Ok(stopped) if *stopped => break,
                        Ok(_) => {}
                        Err(_) => {
                            eprintln!("[conquest-clock] stop mutex poisoned; clock stopped");
                            break;
                        }
                    }
                    let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let now_ms = epoch_now_ms()?;
                        factory.advance_shared_conquest(&config, now_ms, &clock_owner)
                    }));
                    let error = match tick {
                        Ok(result) => result.err(),
                        Err(_) => Some(
                            "conquest clock panicked; durable progression remains fail closed"
                                .into(),
                        ),
                    };
                    if let Some(message) = diagnostics.observe(error, started.elapsed()) {
                        eprintln!("[conquest-clock] {message}");
                    }
                    let (lock, wake) = &*signal;
                    let stopped = match lock.lock() {
                        Ok(stopped) => stopped,
                        Err(_) => {
                            eprintln!("[conquest-clock] stop mutex poisoned; clock stopped");
                            break;
                        }
                    };
                    let (stopped, _) =
                        match wake
                            .wait_timeout_while(stopped, CONQUEST_CLOCK_CADENCE, |stop| !*stop)
                        {
                            Ok(result) => result,
                            Err(_) => {
                                eprintln!("[conquest-clock] stop wait poisoned; clock stopped");
                                break;
                            }
                        };
                    if *stopped {
                        break;
                    }
                }
                // Flush accepted castle damage before releasing this process's
                // lease. Graceful restart must not restore the previous 10s HP.
                let shutdown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    factory.advance_shared_conquest(&config, epoch_now_ms()?, &clock_owner)?;
                    config.release_shared_conquest_clock(&clock_owner)
                }));
                match shutdown {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!("[conquest-clock] shutdown flush failed: {error}"),
                    Err(_) => {
                        eprintln!("[conquest-clock] shutdown flush panicked; recovery required")
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for ConquestClockService {
    fn drop(&mut self) {
        // Even a poisoned stop mutex must wake the worker; no shutdown waits for
        // the full cadence merely because a stop setter failed.
        match self.stop.0.lock() {
            Ok(mut stopped) => *stopped = true,
            Err(poisoned) => *poisoned.into_inner() = true,
        }
        self.stop.1.notify_all();
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                eprintln!("[conquest-clock] worker terminated with a panic");
            }
        }
    }
}

fn new_clock_owner() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    OsRng.try_fill_bytes(&mut bytes).map_err(|error| {
        io::Error::other(format!("conquest lease entropy unavailable: {error}"))
    })?;
    let mut owner = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write as _;
        // String formatting cannot fail; retain an explicit error boundary.
        write!(&mut owner, "{byte:02x}")
            .map_err(|_| io::Error::other("conquest lease token formatting failed"))?;
    }
    Ok(owner)
}

fn epoch_now_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "conquest system clock is before Unix epoch")?;
    let now_ms =
        u64::try_from(elapsed.as_millis()).map_err(|_| "conquest system clock exhausted")?;
    if now_ms > i64::MAX as u64 {
        return Err("conquest system clock exceeds durable calendar".into());
    }
    Ok(now_ms)
}

#[derive(Default)]
struct ClockDiagnostics {
    previous_error: Option<String>,
    last_logged: Duration,
}

impl ClockDiagnostics {
    fn observe(&mut self, error: Option<String>, elapsed: Duration) -> Option<String> {
        let message = if error != self.previous_error {
            match &error {
                Some(error) => Some(error.clone()),
                None if self.previous_error.is_some() => {
                    Some("persistence and projection recovered".into())
                }
                None => None,
            }
        } else if error.is_some()
            && elapsed.saturating_sub(self.last_logged) >= CONQUEST_ERROR_REPEAT
        {
            error.clone()
        } else {
            None
        };
        self.previous_error = error;
        if message.is_some() {
            self.last_logged = elapsed;
        }
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mir2_simulation::conquest::{sabuk_policy, valid_guild_id};

    #[test]
    fn conquest_clock_tokens_are_valid_distinct_cryptographic_lease_identifiers() {
        let first = new_clock_owner().unwrap();
        let second = new_clock_owner().unwrap();
        assert!(valid_guild_id(&first));
        assert!(valid_guild_id(&second));
        assert_ne!(first, second);
        assert!(first
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }

    #[test]
    fn conquest_clock_failures_are_visible_rate_limited_and_recovery_is_reported_once() {
        let mut diagnostics = ClockDiagnostics::default();
        let failed = Some("durable source unavailable".into());
        assert_eq!(diagnostics.observe(None, Duration::ZERO), None);
        assert_eq!(diagnostics.observe(failed.clone(), Duration::ZERO), failed);
        assert_eq!(
            diagnostics.observe(failed.clone(), Duration::from_secs(10)),
            None
        );
        assert_eq!(
            diagnostics.observe(failed.clone(), Duration::from_secs(59)),
            None
        );
        assert_eq!(
            diagnostics.observe(failed.clone(), Duration::from_secs(60)),
            failed
        );
        assert!(diagnostics
            .observe(
                Some("primary source unavailable".into()),
                Duration::from_secs(61)
            )
            .is_some());
        assert_eq!(
            diagnostics
                .observe(None, Duration::from_secs(62))
                .as_deref(),
            Some("persistence and projection recovered")
        );
        assert_eq!(diagnostics.observe(None, Duration::from_secs(63)), None);
    }

    #[test]
    fn conquest_clock_starts_with_empty_server_persists_lease_and_shutdown_wakes_cadence() {
        let directory = std::env::temp_dir().join(format!(
            "mir2-conquest-clock-{}-{}",
            std::process::id(),
            new_clock_owner().unwrap(),
        ));
        let path = directory.join("accounts.json");
        let mut config = GatewayConfig::default().with_account_store_path(&path);
        config.conquest_policies = vec![sabuk_policy()];
        let factory = Arc::new(SharedInProcessZoneRuntimeFactory::new());
        assert_eq!(factory.active_zone_count(), 0);
        let service = ConquestClockService::start(config.clone(), factory.clone()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let lease = loop {
            let snapshot = config
                .account_store
                .lock()
                .unwrap()
                .shared_conquests
                .get(&1)
                .cloned();
            if let Some(lease) = snapshot.and_then(|snapshot| snapshot.lease) {
                break lease;
            }
            assert!(
                Instant::now() < deadline,
                "independent conquest clock did not start"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(valid_guild_id(&lease.owner));
        assert_eq!(lease.generation, 1);
        assert!(path.is_file(), "empty-server clock did not durably persist");
        assert_eq!(factory.active_zone_count(), 0);
        let stopped = Instant::now();
        drop(service);
        assert!(stopped.elapsed() < Duration::from_secs(2));
        let disk: mir2_simulation::AccountStore =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(
            disk.shared_conquests[&1].lease.is_none(),
            "graceful shutdown must relinquish its clock grant"
        );
        assert_eq!(disk.shared_conquests[&1].clock_generation, lease.generation);
        let reopened = GatewayConfig::default().with_account_store_path(&path);
        assert_eq!(
            reopened.account_store.lock().unwrap().shared_conquests[&1]
                .lease
                .as_ref(),
            None
        );
        drop(reopened);
        drop(config);
        // This is a freshly created isolated test directory, never an install or
        // a user character store.
        assert_eq!(directory.parent(), Some(std::env::temp_dir().as_path()));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
