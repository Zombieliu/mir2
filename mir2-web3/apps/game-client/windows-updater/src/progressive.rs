//! Background completion for an already authenticated progressive generation.
//! The catalog is the durable work list; scratch prefixes are never availability.
//! No discovery, selector writes, external URLs, or executable downloads occur here.
use crate::{
    fs_safe as safe, generation, model,
    update::{HttpsSource, Source},
};
use anyhow::{ensure, Context, Result};
use mir2_resource_pack::{ScopedPackStore, ScopedRead};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    task::{Context as TaskContext, Poll, Waker},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_ATTEMPTS: u8 = 3;
const RECORD_SCHEMA: &str = "mir2.windows.progressive-downloads.v1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSnapshot {
    pub total_packs: usize,
    pub available_packs: usize,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub foreground_pending: usize,
    /// All response-body bytes in this process, including retries.
    pub response_body_bytes: u64,
    pub last_error: Option<String>,
    pub complete: bool,
}

#[derive(Default)]
struct Queue {
    foreground: BTreeSet<String>,
    attempts: BTreeMap<String, u8>,
    retry_after: BTreeMap<String, Instant>,
    wakers: Vec<Waker>,
    failures: BTreeMap<String, String>,
    last_error: Option<String>,
    dirty: bool,
}
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
    stop: AtomicBool,
    wire: AtomicU64,
}

/// Bookkeeping is untrusted and never supplies paths, objects or expected SHA.
/// On restart only declared requested pack IDs are restored. Availability is
/// rebuilt from full validation of each immutable cached object.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResumeRecord {
    schema: String,
    manifest_sha256: String,
    catalog_sha256: String,
    scope_sha256: String,
    foreground: BTreeSet<String>,
}

pub struct ProgressiveDownloads {
    store: Arc<ScopedPackStore>,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl ProgressiveDownloads {
    /// Only pinned production HTTPS is available to callers. The existing CMS,
    /// active selection, source, and required-pack checks are repeated here so
    /// a caller cannot manufacture a VerifiedGeneration to authorize downloads.
    pub fn start(
        install_root: &Path,
        verified: &generation::VerifiedGeneration,
        store: Arc<ScopedPackStore>,
    ) -> Result<Arc<Self>> {
        let actual =
            generation::verify_generation_for_launch(install_root, &verified.manifest_sha256)?;
        ensure!(
            actual.attempt == verified.attempt,
            "progressive launch attempt changed"
        );
        let scope = actual
            .manifest
            .required_scope
            .as_ref()
            .context("generation is not progressive")?;
        ensure!(
            actual.required_scope_bytes.is_some(),
            "authenticated scope missing"
        );
        ensure!(
            store.catalog_sha256() == actual.manifest.catalog.sha256
                && store.scope_sha256() == scope.sha256,
            "progressive store differs from authenticated selection"
        );
        Self::start_inner(
            install_root,
            &actual.manifest_sha256,
            store,
            HttpsSource::new_packed()?,
        )
    }

    fn start_inner<S: Source + Send + 'static>(
        install_root: &Path,
        manifest_sha: &str,
        store: Arc<ScopedPackStore>,
        source: S,
    ) -> Result<Arc<Self>> {
        ensure!(
            model::valid_hash(manifest_sha),
            "invalid progressive manifest SHA"
        );
        safe::ancestors(install_root)?;
        let record = safe::target(
            install_root,
            &format!("state/progressive/{manifest_sha}.json"),
        )?;
        let parts = safe::target(
            install_root,
            &format!(".update/progressive-parts/{manifest_sha}"),
        )?;
        // Also excludes an accidental second completion worker in this install.
        let lease = safe::open_download(&safe::target(
            install_root,
            ".update/progressive-download.lock",
        )?)?;
        let mut queue = Queue::default();
        if record.exists() {
            let bytes = safe::read_bounded(&record, 32768)?;
            let resume: ResumeRecord = serde_json::from_slice(&bytes)?;
            ensure!(
                resume.schema == RECORD_SCHEMA
                    && resume.manifest_sha256 == manifest_sha
                    && resume.catalog_sha256 == store.catalog_sha256()
                    && resume.scope_sha256 == store.scope_sha256(),
                "progressive resume binding differs"
            );
            ensure!(
                resume.foreground.len() <= store.catalog().packs.len(),
                "progressive request count bound"
            );
            for sha in resume.foreground {
                ensure!(
                    store.catalog().packs.iter().any(|p| p.sha256 == sha),
                    "undeclared resume request"
                );
                queue.foreground.insert(sha);
            }
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(queue),
            changed: Condvar::new(),
            stop: AtomicBool::new(false),
            wire: AtomicU64::new(0),
        });
        let worker_store = store.clone();
        let worker_shared = shared.clone();
        let install_root = install_root.to_owned();
        let manifest_sha = manifest_sha.to_owned();
        let worker = thread::Builder::new()
            .name("mir2-resource-completion".into())
            .spawn(move || {
                let _lease = lease;
                if let Err(error) = run_worker(
                    &install_root,
                    &manifest_sha,
                    &record,
                    &parts,
                    &worker_store,
                    &worker_shared,
                    source,
                ) {
                    if let Ok(mut queue) = worker_shared.queue.lock() {
                        queue.last_error = Some(format!("Resource completion stopped: {error:#}"));
                        let wakers = take_wakers(&mut queue);
                        drop(queue);
                        for waker in wakers {
                            waker.wake();
                        }
                    }
                }
            })?;
        Ok(Arc::new(Self {
            store,
            shared,
            worker: Some(worker),
        }))
    }

    pub fn store(&self) -> &ScopedPackStore {
        &self.store
    }

    /// Nonblocking, verified reads only. Pending remains a typed state.
    pub fn read(&self, path: &str) -> Result<ScopedRead> {
        let result = self.store.read(path)?;
        if let ScopedRead::Pending(object) = &result {
            let mut queue = self
                .shared
                .queue
                .lock()
                .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
            if queue.foreground.insert(object.sha256.clone()) {
                queue.dirty = true;
            }
            self.shared.changed.notify_one();
        }
        Ok(result)
    }

    /// Fits Bevy's executor; never assumes a Tokio runtime and never blocks a
    /// render thread on HTTP. Register-before-read prevents lost wakeups.
    pub fn poll_read(&self, path: &str, cx: &mut TaskContext<'_>) -> Poll<Result<Vec<u8>>> {
        match self.read(path) {
            Ok(ScopedRead::Ready(bytes)) => return Poll::Ready(Ok(bytes)),
            Err(error) => return Poll::Ready(Err(error)),
            Ok(ScopedRead::Pending(_)) => {}
        }
        {
            let Ok(mut queue) = self.shared.queue.lock() else {
                return Poll::Ready(Err(anyhow::anyhow!("resource queue poisoned")));
            };
            if !queue.wakers.iter().any(|old| old.will_wake(cx.waker())) {
                if queue.wakers.len() >= mir2_resource_pack::MAX_RESOURCES {
                    return Poll::Ready(Err(anyhow::anyhow!("resource waiter count bound")));
                }
                queue.wakers.push(cx.waker().clone());
            }
        }
        match self.read(path) {
            Ok(ScopedRead::Ready(bytes)) => Poll::Ready(Ok(bytes)),
            Ok(ScopedRead::Pending(_)) => Poll::Pending,
            Err(error) => Poll::Ready(Err(error)),
        }
    }

    pub fn snapshot(&self) -> Result<DownloadSnapshot> {
        let pending = self.store.pending_objects()?;
        let total_bytes = self
            .store
            .catalog()
            .packs
            .iter()
            .map(|p| p.size)
            .sum::<u64>();
        let pending_bytes = pending.iter().map(|p| p.size).sum::<u64>();
        let queue = self
            .shared
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
        Ok(DownloadSnapshot {
            total_packs: self.store.catalog().packs.len(),
            available_packs: self.store.catalog().packs.len() - pending.len(),
            total_bytes,
            available_bytes: total_bytes - pending_bytes,
            foreground_pending: pending
                .iter()
                .filter(|p| queue.foreground.contains(&p.sha256))
                .count(),
            response_body_bytes: self.shared.wire.load(Ordering::Relaxed),
            last_error: queue
                .last_error
                .clone()
                .or_else(|| queue.failures.values().next().cloned()),
            complete: pending.is_empty(),
        })
    }

    /// Explicit UI retry, also used after a player repairs network or disk.
    /// No SHA check, lock, signed condition or selector is bypassed.
    pub fn retry(&self) -> Result<()> {
        let mut queue = self
            .shared
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
        queue.attempts.clear();
        queue.retry_after.clear();
        queue.last_error = None;
        queue.failures.clear();
        self.shared.changed.notify_one();
        Ok(())
    }
}

impl Drop for ProgressiveDownloads {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.changed.notify_all();
        // Never wait for a timed-out network request during native window exit.
        // The worker retains its own store and download lease until it returns.
        if let Some(worker) = self.worker.take().filter(JoinHandle::is_finished) {
            let _ = worker.join();
        }
    }
}

fn take_wakers(queue: &mut Queue) -> Vec<Waker> {
    std::mem::take(&mut queue.wakers)
}

fn persist(
    record: &Path,
    manifest_sha: &str,
    store: &ScopedPackStore,
    queue: &Queue,
) -> Result<()> {
    let bytes = serde_json::to_vec(&ResumeRecord {
        schema: RECORD_SCHEMA.into(),
        manifest_sha256: manifest_sha.into(),
        catalog_sha256: store.catalog_sha256().into(),
        scope_sha256: store.scope_sha256().into(),
        foreground: queue.foreground.clone(),
    })?;
    ensure!(bytes.len() <= 32768, "progressive resume record bound");
    safe::atomic_write(record, &bytes)
}

#[derive(Debug)]
struct Preempted;
impl std::fmt::Display for Preempted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("foreground resource takes priority")
    }
}
impl std::error::Error for Preempted {}

struct PrioritySource<'a, S> {
    source: &'a S,
    shared: &'a Shared,
    sha: &'a str,
    foreground: bool,
}
impl<S> PrioritySource<'_, S> {
    fn priority(&self) -> Result<()> {
        if !self.foreground {
            let queue = self
                .shared
                .queue
                .lock()
                .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
            if queue.foreground.iter().any(|sha| sha != self.sha) {
                return Err(Preempted.into());
            }
        }
        Ok(())
    }
}
impl<S: Source> Source for PrioritySource<'_, S> {
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.source.get(path, limit, progress)
    }
    fn download(
        &self,
        path: &str,
        file: &model::FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        self.source.download(path, file, destination, &mut |n| {
            progress(n)?;
            self.priority()
        })
    }
    fn permits_partial_resume(&self) -> bool {
        self.source.permits_partial_resume()
    }
    fn download_resuming(
        &self,
        path: &str,
        file: &model::FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(crate::update::DownloadProgress) -> Result<()>,
    ) -> Result<u64> {
        self.source
            .download_resuming(path, file, destination, &mut |p| {
                progress(p)?;
                self.priority()
            })
    }
}

fn run_worker<S: Source>(
    root: &Path,
    manifest_sha: &str,
    record: &Path,
    parts: &Path,
    store: &ScopedPackStore,
    shared: &Shared,
    source: S,
) -> Result<()> {
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pending = store.pending_objects()?;
        let (object, foreground) =
            {
                let mut queue = shared
                    .queue
                    .lock()
                    .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
                queue
                    .foreground
                    .retain(|sha| pending.iter().any(|p| &p.sha256 == sha));
                if queue.dirty {
                    if let Err(error) = persist(record, manifest_sha, store, &queue) {
                        // A full disk or transient access error must not kill the
                        // worker permanently. Keep the same durable work list and
                        // immutable prefixes; repair/retry resumes this worker.
                        queue.last_error =
                            Some(format!("Resource resume record unavailable: {error:#}"));
                        let wakers = take_wakers(&mut queue);
                        drop(queue);
                        for waker in wakers {
                            waker.wake();
                        }
                        let queue = shared
                            .queue
                            .lock()
                            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
                        drop(
                            shared
                                .changed
                                .wait_timeout(queue, Duration::from_secs(1))
                                .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?,
                        );
                        continue;
                    }
                    queue.dirty = false;
                    if queue.last_error.as_deref().is_some_and(|error| {
                        error.starts_with("Resource resume record unavailable:")
                    }) {
                        queue.last_error = None;
                    }
                }
                let eligible = |sha: &str| {
                    queue.attempts.get(sha).copied().unwrap_or(0) < MAX_ATTEMPTS
                        && queue
                            .retry_after
                            .get(sha)
                            .is_none_or(|at| *at <= Instant::now())
                };
                let foreground = pending
                    .iter()
                    .find(|p| queue.foreground.contains(&p.sha256) && eligible(&p.sha256));
                // While foreground is in backoff/needs explicit retry, do not
                // repeatedly start and preempt unrelated background transfers.
                let next = foreground.or_else(|| {
                    if queue.foreground.is_empty() {
                        pending
                            .iter()
                            .filter(|p| eligible(&p.sha256))
                            .min_by_key(|p| p.size)
                    } else {
                        None
                    }
                });
                let Some(object) = next.cloned() else {
                    // No polling disk or traffic loop when complete, paused, or offline.
                    drop(
                        shared
                            .changed
                            .wait_timeout(queue, Duration::from_secs(1))
                            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?,
                    );
                    continue;
                };
                (object, foreground.is_some())
            };
        let entry = model::FileEntry {
            path: format!("objects/packs/{}", object.file_name()),
            size: object.size,
            sha256: object.sha256.clone(),
        };
        let priority = PrioritySource {
            source: &source,
            shared,
            sha: &object.sha256,
            foreground,
        };
        let result: Result<()> = (|| {
            let target = safe::target(root, &entry.path)?;
            generation::ensure_object(
                &priority,
                &entry.path,
                &entry,
                &target,
                parts,
                &shared.stop,
                &shared.wire,
            )?;
            store.admit_cached_pack(&object.sha256)?;
            Ok(())
        })();
        let mut queue = shared
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
        match result {
            Ok(()) => {
                queue.foreground.remove(&object.sha256);
                queue.attempts.remove(&object.sha256);
                queue.retry_after.remove(&object.sha256);
                queue.failures.remove(&object.sha256);
                queue.dirty = true;
            }
            Err(error) if error.is::<Preempted>() => {}
            Err(error) => {
                if shared.stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let count = queue.attempts.entry(object.sha256.clone()).or_default();
                *count += 1;
                let delay = Duration::from_secs(1u64 << *count);
                queue
                    .retry_after
                    .insert(object.sha256.clone(), Instant::now() + delay);
                queue.failures.insert(
                    object.sha256.clone(),
                    format!("Resource download paused or retrying: {error:#}"),
                );
            }
        }
        let wakers = take_wakers(&mut queue);
        drop(queue);
        for waker in wakers {
            waker.wake();
        }
        // One connection and a short inter-object yield bound background work.
        // Foreground requests interrupt both this wait and the active transfer.
        let queue = shared
            .queue
            .lock()
            .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?;
        if queue.foreground.is_empty() {
            drop(
                shared
                    .changed
                    .wait_timeout(queue, Duration::from_millis(100))
                    .map_err(|_| anyhow::anyhow!("resource queue poisoned"))?,
            );
        }
    }
}

#[cfg(test)]
#[path = "progressive_tests.rs"]
mod tests;
