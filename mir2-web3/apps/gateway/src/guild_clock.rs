//! A server-lifetime clock, independent of client/session/Zone activity.
use crate::GatewayConfig;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
pub struct GuildClockService {
    stop: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}
impl GuildClockService {
    pub fn start(config: GatewayConfig) -> std::io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = stop.clone();
        let worker = std::thread::Builder::new()
            .name("shared-guild-clock".into())
            .spawn(move || {
                let mut previous_error = None;
                loop {
                    let current_error = config.tick_shared_guild_clock().err();
                    if current_error != previous_error {
                        if let Some(error) = &current_error {
                            eprintln!("[guild-clock] {error}");
                        } else if previous_error.is_some() {
                            eprintln!("[guild-clock] persistence recovered");
                        }
                        previous_error = current_error;
                    }
                    let (lock, wake) = &*signal;
                    let Ok(stopped) = lock.lock() else {
                        break;
                    };
                    let Ok((stopped, _)) =
                        wake.wait_timeout_while(stopped, Duration::from_secs(10), |stop| !*stop)
                    else {
                        break;
                    };
                    if *stopped {
                        break;
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for GuildClockService {
    fn drop(&mut self) {
        if let Ok(mut stopped) = self.stop.0.lock() {
            *stopped = true;
            self.stop.1.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_clock_starts_without_any_gateway_session_and_stops_promptly() {
        let directory=std::env::temp_dir().join(format!("mir2-server-clock-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let path=directory.join("accounts.json");
        let config=GatewayConfig::default().with_account_store_path(&path);
        let second=GatewayConfig::default().with_account_store_path(&path);
        let first_guard=GuildClockService::start(config.clone()).unwrap();
        let second_guard=GuildClockService::start(second.clone()).unwrap();
        let deadline=std::time::Instant::now()+Duration::from_secs(3);
        loop {
            let value=serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
            if value["guildClock"]["ownerToken"].is_string() {break;}
            assert!(std::time::Instant::now()<deadline,"clock never started in empty server");
            std::thread::sleep(Duration::from_millis(10));
        }
        let stop_started=std::time::Instant::now();
        drop(first_guard);drop(second_guard);
        assert!(stop_started.elapsed()<Duration::from_secs(2));
        let first=serde_json::to_value(&*config.account_store.lock().unwrap()).unwrap();
        let last=serde_json::to_value(&*second.account_store.lock().unwrap()).unwrap();
        assert_eq!(first["guildClock"],last["guildClock"]);
        assert_eq!(first["guildClock"]["generation"],2);
        drop(second);drop(config);std::fs::remove_dir_all(directory).unwrap();
    }
}
