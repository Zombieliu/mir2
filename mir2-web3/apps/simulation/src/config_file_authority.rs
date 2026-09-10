//! One in-process authority per durable File store, protected from other
//! processes by a stable sidecar lock (the JSON file itself is atomically replaced).
use super::*;
use std::sync::Weak;

#[derive(Debug)]
pub(super) struct FileAuthority {
    pub path: PathBuf,
    pub store: SharedAccountStore,
    pub persist_lock: Arc<Mutex<()>>,
    pub write_state: Arc<Mutex<AccountStoreWriteState>>,
    pub clock_driver: Arc<Mutex<guild_clock_driver::ClockDriverState>>,
    pub clock_updates: Arc<std::sync::atomic::AtomicU64>,
    _writer_lock: File,
    marker_lock: Mutex<()>,
}
fn registry() -> &'static Mutex<BTreeMap<PathBuf, Weak<FileAuthority>>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<PathBuf, Weak<FileAuthority>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}
fn authority_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let parent = absolute
        .parent()
        .ok_or("file authority path has no parent")?;
    fs::create_dir_all(parent).map_err(|e| format!("file authority directory failed: {e}"))?;
    if absolute.exists() {
        return fs::canonicalize(&absolute).map_err(|e| e.to_string());
    }
    let name = absolute
        .file_name()
        .ok_or("file authority path has no filename")?;
    Ok(fs::canonicalize(parent)
        .map_err(|e| e.to_string())?
        .join(name))
}
/// A write-ahead publication fence. Dropping without explicit settlement blocks
/// this process too; reopening already sees the durable nonempty marker.
pub(super) struct PublicationGuard<'a> {
    authority: &'a FileAuthority,
    settled: bool,
}
impl PublicationGuard<'_> {
    /// Only call after all stores have committed, or a definite failure has been
    /// fully compensated. An unknown outcome must never use this method.
    pub fn settle(mut self) -> Result<(), String> {
        let _marker = self
            .authority
            .marker_lock
            .lock()
            .map_err(|_| "publication marker mutex poisoned")?;
        let state = self
            .authority
            .write_state
            .lock()
            .map_err(|_| "publication write state poisoned")?;
        if let AccountStoreWriteState::Frozen { reason } = &*state {
            return Err(format!("cannot settle frozen publication: {reason}"));
        }
        if let Err(error) = self
            .authority
            ._writer_lock
            .set_len(0)
            .and_then(|_| self.authority._writer_lock.sync_all())
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = &self.authority._writer_lock;
            let fence = file
                .seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(b"FROZEN: marker settlement failed\n"))
                .and_then(|_| file.sync_all());
            return Err(format!(
                "publication marker settlement failed: {error}; replacement fence: {fence:?}"
            ));
        }
        self.settled = true;
        Ok(())
    }
}
impl Drop for PublicationGuard<'_> {
    fn drop(&mut self) {
        if !self.settled {
            if let Ok(mut state) = self.authority.write_state.lock() {
                if matches!(*state, AccountStoreWriteState::Writable) {
                    *state = AccountStoreWriteState::Frozen {
                        reason: "publication did not reach verified settlement; explicit reconciliation required".into(),
                    };
                }
            }
        }
    }
}
impl FileAuthority {
    /// Identity is a durable domain, not a disposable cache. Its path is always
    /// derived from the currently bound canonical account authority.
    pub(super) fn item_identity_path(&self, config: &SimulationConfig) -> Result<PathBuf, String> {
        let path = config.account_store_path.as_deref().ok_or("item identity requires File authority")?;
        if !self.owns(config, path) {
            return Err("item identity requires a live canonical File authority binding".into());
        }
        let mut name = self.path.as_os_str().to_os_string();
        name.push(".item-identity.json");
        Ok(PathBuf::from(name))
    }
    pub fn acquire(path: &Path, default_character: CharacterRecord) -> Result<Arc<Self>, String> {
        let path = authority_path(path)?;
        // Windows file names are case-insensitive under the normal account-store
        // directory configuration, including before the first JSON publication.
        #[cfg(windows)]
        let key = PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase());
        #[cfg(not(windows))]
        let key = path.clone();
        let mut registry = registry()
            .lock()
            .map_err(|_| "file authority registry poisoned")?;
        registry.retain(|_, entry| entry.strong_count() > 0);
        if let Some(authority) = registry.get(&key).and_then(Weak::upgrade) {
            return Ok(authority);
        }
        let mut lock_name = path.as_os_str().to_os_string();
        lock_name.push(".writer.lock");
        let lock_path = PathBuf::from(lock_name);
        let writer_lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| format!("file authority lock open failed: {e}"))?;
        writer_lock.try_lock().map_err(|e| {
            format!(
                "another process owns file account store {}: {e}",
                path.display()
            )
        })?;
        writer_lock
            .sync_all()
            .map_err(|e| format!("file authority lock sync failed: {e}"))?;
        #[cfg(unix)]
        File::open(path.parent().ok_or("file authority has no parent")?)
            .and_then(|directory| directory.sync_all())
            .map_err(|e| format!("file authority directory sync failed: {e}"))?;
        // An empty lock file is the historical clean state. Any durable bytes,
        // including a truncated marker after a crash, require reconciliation.
        // Never interpret malformed data as permission to publish again.
        let marker = {
            use std::io::{Read, Seek, SeekFrom};
            let mut file = &writer_lock;
            let mut bytes = Vec::new();
            file.seek(SeekFrom::Start(0))
                .and_then(|_| file.read_to_end(&mut bytes))
                .map_err(|e| format!("file authority marker read failed: {e}"))?;
            bytes
        };
        let write_state = if marker.is_empty() {
            AccountStoreWriteState::Writable
        } else {
            AccountStoreWriteState::Frozen {
                reason: format!(
                    "durable publication marker requires explicit reconciliation: {}",
                    String::from_utf8_lossy(&marker)
                ),
            }
        };
        // Validate bytes before supplying historical demo defaults. Failure drops
        // the OS lock and never rewrites or replaces the invalid source image.
        let store = FileAccountStoreRepository::new(&path).load(default_character)?;
        let authority = Arc::new(Self {
            path,
            store: Arc::new(Mutex::new(store)),
            persist_lock: Arc::new(Mutex::new(())),
            write_state: Arc::new(Mutex::new(write_state)),
            clock_driver: Arc::new(Mutex::new(guild_clock_driver::ClockDriverState::default())),
            clock_updates: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            _writer_lock: writer_lock,
            marker_lock: Mutex::new(()),
        });
        registry.insert(key, Arc::downgrade(&authority));
        Ok(authority)
    }
    /// Preserve uncertainty across Config release and process restart. Append,
    /// never truncate: a failed update must not erase an earlier durable fence.
    /// No ordinary load or retry may clear this marker.
    pub fn persist_freeze(&self, reason: &str) -> Result<(), String> {
        use std::io::{Seek, SeekFrom, Write};
        let _guard = self
            .marker_lock
            .lock()
            .map_err(|_| "file authority marker mutex poisoned")?;
        let mut file = &self._writer_lock;
        file.seek(SeekFrom::End(0))
            .map_err(|e| format!("file authority marker seek failed: {e}"))?;
        file.write_all(format!("FROZEN: {reason}\n").as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("file authority durable freeze failed: {e}"))
    }
    /// Caller holds the shared persist mutex throughout the complete operation.
    pub fn begin_publication(&self) -> Result<PublicationGuard<'_>, String> {
        use std::io::{Seek, SeekFrom, Write};
        let _marker = self
            .marker_lock
            .lock()
            .map_err(|_| "publication marker mutex poisoned")?;
        if let AccountStoreWriteState::Frozen { reason } = &*self
            .write_state
            .lock()
            .map_err(|_| "publication write state poisoned")?
        {
            return Err(format!("publication is frozen: {reason}"));
        }
        if self
            ._writer_lock
            .metadata()
            .map_err(|e| e.to_string())?
            .len()
            != 0
        {
            return Err("publication marker is already present; reconciliation required".into());
        }
        let mut file = &self._writer_lock;
        file.seek(SeekFrom::Start(0))
            .and_then(|_| file.write_all(b"PENDING PUBLICATION\n"))
            .and_then(|_| file.sync_all())
            .map_err(|e| {
                format!("publication marker creation failed before repository write: {e}")
            })?;
        Ok(PublicationGuard {
            authority: self,
            settled: false,
        })
    }
    pub fn owns(&self, config: &SimulationConfig, path: &Path) -> bool {
        self.path == path
            && Arc::ptr_eq(&self.store, &config.account_store)
            && Arc::ptr_eq(&self.persist_lock, &config.account_store_persist_lock)
            && Arc::ptr_eq(&self.write_state, &config.account_store_write_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "mir2-file-authority-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
            .join("accounts.json")
    }
    #[test]
    fn pending_publication_requires_explicit_settlement() {
        let path = path();
        let authority =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        authority.begin_publication().unwrap().settle().unwrap();
        drop(authority);
        let authority =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        assert!(matches!(
            *authority.write_state.lock().unwrap(),
            AccountStoreWriteState::Writable
        ));
        drop(authority.begin_publication().unwrap());
        assert!(matches!(
            *authority.write_state.lock().unwrap(),
            AccountStoreWriteState::Frozen { .. }
        ));
        drop(authority);
        let reopened =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        assert!(matches!(
            *reopened.write_state.lock().unwrap(),
            AccountStoreWriteState::Frozen { .. }
        ));
        assert!(reopened.begin_publication().is_err());
        drop(reopened);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn config_fences_before_external_writer_and_preserves_uncertain_restart() {
        let path = path();
        let config = SimulationConfig::default()
            .with_account_store_path(&path)
            .with_account_store_database_url("postgres://unused.invalid/publication-test");
        let mut marker_name = path.as_os_str().to_os_string();
        marker_name.push(".writer.lock");
        let marker_path = PathBuf::from(marker_name);
        let error = config
            .save_account_store_with_plan_writer(None, |_, _, _| {
                assert!(
                    fs::metadata(&marker_path).unwrap().len() > 0,
                    "publication must be durable before invoking external repository"
                );
                Err("simulated lost external commit acknowledgement".into())
            })
            .unwrap_err();
        assert!(error.contains("lost external"));
        assert!(config.ensure_account_store_writable().is_err());
        drop(config);
        let reopened = SimulationConfig::default().with_account_store_path(&path);
        assert!(reopened.ensure_account_store_writable().is_err());
        drop(reopened);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn definite_file_failure_settles_marker_and_allows_retry() {
        let path = path();
        let config = SimulationConfig::default().with_account_store_path(&path);
        config
            .inject_account_store_transaction_fault(AccountStoreTransactionFault::BeforeFileRename);
        assert!(config.save_account_store().is_err());
        config.ensure_account_store_writable().unwrap();
        config.save_account_store().unwrap();
        drop(config);
        let reopened = SimulationConfig::default().with_account_store_path(&path);
        reopened.ensure_account_store_writable().unwrap();
        drop(reopened);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn durable_freeze_child_reopen_probe() {
        let Some(path) = std::env::var_os("MIR2_DURABLE_FREEZE_CHILD_PROBE") else {
            return;
        };
        let authority = FileAuthority::acquire(
            Path::new(&path),
            SimulationConfig::default().default_character,
        )
        .unwrap();
        assert!(matches!(
            *authority.write_state.lock().unwrap(),
            AccountStoreWriteState::Frozen { .. }
        ));
    }
    #[test]
    fn durable_freeze_survives_last_authority_release() {
        let path = path();
        let authority =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        authority
            .persist_freeze("ambiguous external commit")
            .unwrap();
        drop(authority);
        let reopened =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        match &*reopened.write_state.lock().unwrap() {
            AccountStoreWriteState::Frozen { reason } => {
                assert!(reason.contains("ambiguous external commit"))
            }
            _ => panic!("restart must not clear durable uncertainty"),
        }
        drop(reopened);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("config::file_authority::tests::durable_freeze_child_reopen_probe")
            .arg("--nocapture")
            .env("MIR2_DURABLE_FREEZE_CHILD_PROBE", &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn malformed_publication_marker_is_fail_closed_and_preserved() {
        let path = path();
        let authority =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        let mut lock_name = authority.path.as_os_str().to_os_string();
        lock_name.push(".writer.lock");
        let lock_path = PathBuf::from(lock_name);
        drop(authority);
        fs::write(&lock_path, [0xff, 0]).unwrap();
        let reopened =
            FileAuthority::acquire(&path, SimulationConfig::default().default_character).unwrap();
        assert!(matches!(
            *reopened.write_state.lock().unwrap(),
            AccountStoreWriteState::Frozen { .. }
        ));
        drop(reopened);
        assert_eq!(fs::read(&lock_path).unwrap(), [0xff, 0]);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn guild_clock_file_configs_share_store_mutex_and_frozen_state() {
        let path = path();
        let first = SimulationConfig::default().with_account_store_path(&path);
        let second = SimulationConfig::default()
            .with_account_store_path(path.parent().unwrap().join(".").join("accounts.json"));
        assert!(Arc::ptr_eq(&first.account_store, &second.account_store));
        assert!(Arc::ptr_eq(
            &first.account_store_persist_lock,
            &second.account_store_persist_lock
        ));
        first.account_store.lock().unwrap().next_character_index = 123;
        assert_eq!(
            second.account_store.lock().unwrap().next_character_index,
            123
        );
        let isolated = first
            .fork_with_isolated_account_store()
            .unwrap()
            .with_account_store_database_url("postgres://unused.invalid/guild-clock-test");
        isolated.inject_account_store_repository_writer_probe(Ok(
            AccountStoreRepositorySave::default(),
        ));
        let error = isolated
            .commit_account_store_transaction(&["demo".into()], |_| Ok(()))
            .unwrap_err();
        assert!(error.contains("shared authority"));
        assert_eq!(
            isolated.account_store_repository_writer_probe_invocations(),
            0,
            "reject before any mirror write"
        );
        drop(isolated);
        first.freeze_account_store_writes("test ambiguous clock commit".into());
        assert!(second
            .ensure_account_store_writable()
            .unwrap_err()
            .contains("test ambiguous clock commit"));
        let isolated = first.fork_with_isolated_account_store().unwrap();
        assert!(!isolated
            .file_authority
            .as_ref()
            .unwrap()
            .owns(&isolated, isolated.account_store_path.as_deref().unwrap()));
        drop(isolated);
        drop(first);
        drop(second);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn guild_clock_file_independent_configs_preserve_concurrent_account_writes() {
        let path = path();
        let first = SimulationConfig::default().with_account_store_path(&path);
        let second = SimulationConfig::default().with_account_store_path(&path);
        let character_index = first.default_character.index;
        let starting_gold =
            first.account_store.lock().unwrap().accounts["demo"].saves[&character_index].gold;
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles = [first, second]
            .into_iter()
            .map(|config| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    for _ in 0..10 {
                        config
                            .commit_account_store_transaction(&["demo".into()], |store| {
                                store
                                    .accounts
                                    .get_mut("demo")
                                    .unwrap()
                                    .saves
                                    .get_mut(&character_index)
                                    .unwrap()
                                    .gold += 1;
                                Ok(())
                            })
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
        let loaded = FileAccountStoreRepository::new(&path)
            .load(SimulationConfig::default().default_character)
            .unwrap();
        assert_eq!(
            loaded.accounts["demo"].saves[&character_index].gold,
            starting_gold + 20,
            "each config must mutate the latest shared image"
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn guild_clock_file_child_lock_probe() {
        let Some(path) = std::env::var_os("MIR2_FILE_AUTHORITY_CHILD_PROBE") else {
            return;
        };
        let result = FileAuthority::acquire(
            Path::new(&path),
            SimulationConfig::default().default_character,
        );
        assert!(result.unwrap_err().contains("another process owns"));
    }
    #[test]
    fn guild_clock_file_second_process_is_rejected_and_release_allows_reopen() {
        let path = path();
        let config = SimulationConfig::default().with_account_store_path(&path);
        config.ensure_account_store_writable().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("config::file_authority::tests::guild_clock_file_child_lock_probe")
            .arg("--nocapture")
            .env("MIR2_FILE_AUTHORITY_CHILD_PROBE", &path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "child probe must actually run"
        );
        drop(config);
        let reopened = SimulationConfig::default().with_account_store_path(&path);
        reopened.ensure_account_store_writable().unwrap();
        drop(reopened);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
