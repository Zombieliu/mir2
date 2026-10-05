use crate::{
    fs_safe as safe,
    model::*,
    transaction::{self, Change},
};
#[path = "delivery.rs"]
pub mod delivery;
use anyhow::{ensure, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub trait Source {
    /// Fetch a fresh discovery pair. Progress reports additional response-body
    /// bytes across both objects, including discarded origin attempts.
    fn discovery_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let bytes = discovery_object(
            |callback| self.get("latest.json", 32768, callback),
            progress,
        )?;
        let signature =
            discovery_object(|callback| self.get("latest.p7s", 32768, callback), progress)?;
        Ok((bytes, signature))
    }
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>>;
    fn download(
        &self,
        path: &str,
        file: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()>;
}
fn discovery_object(
    fetch: impl FnOnce(&mut dyn FnMut(u64) -> Result<()>) -> Result<Vec<u8>>,
    progress: &mut dyn FnMut(u64) -> Result<()>,
) -> Result<Vec<u8>> {
    let mut received = 0;
    let bytes = fetch(&mut |count| {
        ensure!(count <= 32769, "discovery response read bound");
        let advanced = count.saturating_sub(received);
        received = received.max(count);
        progress(advanced)?;
        ensure!(count <= 32768, "discovery response too large");
        Ok(())
    })?;
    ensure!(bytes.len() <= 32768, "discovery response too large");
    progress((bytes.len() as u64).saturating_sub(received))?;
    Ok(bytes)
}
struct OpenFailure {
    error: anyhow::Error,
    unavailable: bool,
}
pub struct HttpsSource {
    base: url::Url,
    legacy_base: url::Url,
    agent: ureq::Agent,
}
impl HttpsSource {
    pub fn new() -> Result<Self> {
        let base = Self::pinned_base(crate::CDN_FEED_URL)?;
        let legacy_base = Self::pinned_base(crate::FEED_URL)?;
        Ok(Self {
            base,
            legacy_base,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_connect(Duration::from_secs(8))
                .timeout_read(Duration::from_secs(30))
                .timeout_write(Duration::from_secs(30))
                .build(),
        })
    }
    fn pinned_base(feed: &str) -> Result<url::Url> {
        let feed = url::Url::parse(feed)?;
        ensure!(
            feed.scheme() == "https"
                && feed.username().is_empty()
                && feed.password().is_none()
                && feed.query().is_none()
                && feed.fragment().is_none(),
            "invalid update origin"
        );
        Ok(feed.join("./")?)
    }
    fn object_url(base: &url::Url, path: &str) -> Result<url::Url> {
        relative(path)?;
        let mut url = base.clone();
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("invalid update origin"))?;
            segments.pop_if_empty();
            for segment in path.split('/') {
                // Encode a literal object key once. Percent-looking manifest
                // names, spaces and '#' must not become URL control syntax.
                segments.push(segment);
            }
        }
        ensure!(
            url.origin() == base.origin()
                && url.path().starts_with(base.path())
                && url.query().is_none()
                && url.fragment().is_none(),
            "update origin escape"
        );
        Ok(url)
    }
    fn open_at(
        &self,
        base: &url::Url,
        path: &str,
    ) -> std::result::Result<ureq::Response, OpenFailure> {
        let url = Self::object_url(base, path).map_err(|error| OpenFailure {
            error,
            unavailable: false,
        })?;
        let response = self
            .agent
            .get(url.as_str())
            .set("Accept-Encoding", "identity")
            .call()
            .map_err(|error| OpenFailure {
                error: error.into(),
                unavailable: true,
            })?;
        if response.status() != 200 {
            return Err(OpenFailure {
                error: anyhow::anyhow!("invalid update HTTP status {}", response.status()),
                unavailable: true,
            });
        }
        if response.header("Content-Encoding").unwrap_or("identity") != "identity" {
            return Err(OpenFailure {
                error: anyhow::anyhow!("invalid update response encoding"),
                unavailable: false,
            });
        }
        Ok(response)
    }
    fn open(&self, path: &str) -> Result<ureq::Response> {
        // Domains come only from compiled constants. A200 body's validation
        // failure never switches domains to conceal an integrity failure.
        match self.open_at(&self.base, path) {
            Ok(response) => Ok(response),
            Err(failure) if failure.unavailable => self
                .open_at(&self.legacy_base, path)
                .map_err(|failure| failure.error),
            Err(failure) => Err(failure.error),
        }
    }
    fn response_bytes(
        response: ureq::Response,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        if let Some(len) = response.header("Content-Length") {
            ensure!(len.parse::<u64>()? <= limit, "response too large");
        }
        let mut result = Vec::new();
        let mut reader = response.into_reader().take(limit + 1);
        let mut buffer = [0u8; 65536];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            result.extend_from_slice(&buffer[..n]);
            progress(result.len() as u64)?;
            ensure!(result.len() as u64 <= limit, "response too large");
        }
        Ok(result)
    }
}
impl Source for HttpsSource {
    fn discovery_pair(
        &self,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        for (index, base) in [&self.base, &self.legacy_base].into_iter().enumerate() {
            let json = match self.open_at(base, "latest.json") {
                Ok(response) => response,
                Err(failure) if failure.unavailable && index == 0 => continue,
                Err(failure) => return Err(failure.error),
            };
            let bytes = discovery_object(
                |callback| Self::response_bytes(json, 32768, callback),
                progress,
            )?;
            let sig = match self.open_at(base, "latest.p7s") {
                Ok(response) => response,
                // Restart the WHOLE pair on the other pinned origin. Never
                // combine a preferred-origin JSON with a legacy signature.
                Err(failure) if failure.unavailable && index == 0 => continue,
                Err(failure) => return Err(failure.error),
            };
            let signature = discovery_object(
                |callback| Self::response_bytes(sig, 32768, callback),
                progress,
            )?;
            return Ok((bytes, signature));
        }
        unreachable!("two fixed update origins")
    }
    fn get(
        &self,
        path: &str,
        limit: u64,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<Vec<u8>> {
        Self::response_bytes(self.open(path)?, limit, progress)
    }
    fn download(
        &self,
        path: &str,
        entry: &FileEntry,
        destination: &Path,
        progress: &mut dyn FnMut(u64) -> Result<()>,
    ) -> Result<()> {
        let r = self.open(path)?;
        if let Some(len) = r.header("Content-Length") {
            ensure!(len.parse::<u64>()? == entry.size, "payload length mismatch");
        }
        safe::ancestors(destination)?;
        fs::create_dir_all(destination.parent().unwrap())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut reader = r.into_reader().take(entry.size + 1);
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            progress(total)?;
            ensure!(total <= entry.size, "oversized payload");
            file.write_all(&buffer[..n])?;
        }
        file.sync_all()?;
        ensure!(
            total == entry.size && safe::matches(destination, entry)?,
            "truncated/corrupt download"
        );
        Ok(())
    }
}
pub trait Status {
    fn set(&self, key: &str, percent: u32);
    fn cancelled(&self) -> bool;
    /// Phase boundaries/final counts must be visible even inside the UI cadence.
    fn set_final(&self, key: &str, percent: u32) {
        self.set(key, percent);
    }
}

struct DisplayState {
    phase: String,
    last: Option<Instant>,
}
/// Throttle display only. Cancellation is always delegated immediately.
struct ThrottledStatus<'a, T: Status> {
    inner: &'a T,
    state: std::cell::RefCell<DisplayState>,
}
impl<'a, T: Status> ThrottledStatus<'a, T> {
    fn new(inner: &'a T) -> Self {
        Self {
            inner,
            state: std::cell::RefCell::new(DisplayState {
                phase: String::new(),
                last: None,
            }),
        }
    }
    fn send_at(&self, key: &str, percent: u32, boundary: bool, now: Instant) {
        let phase = key.split(':').next().unwrap_or(key);
        let mut state = self.state.borrow_mut();
        if boundary
            || phase != state.phase
            || percent == 100
            || state
                .last
                .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(200))
        {
            self.inner.set(key, percent);
            state.phase = phase.into();
            state.last = Some(now);
        }
    }
}
impl<T: Status> Status for ThrottledStatus<'_, T> {
    fn set(&self, key: &str, percent: u32) {
        self.send_at(key, percent, false, Instant::now());
    }
    fn set_final(&self, key: &str, percent: u32) {
        self.send_at(key, percent, true, Instant::now());
    }
    fn cancelled(&self) -> bool {
        self.inner.cancelled()
    }
}

/// One bounded disk trace per phase, including an unfinished phase on error.
/// This observer never returns an error or reads cancellation during commit.
pub(super) struct LocalPhase<'a, T: Status> {
    root: &'a Path,
    status: &'a T,
    phase: &'static str,
    total: usize,
    completed: usize,
    range: (u32, u32),
    started: Instant,
    last: Instant,
    complete: bool,
    cache_hits: usize,
    written_bytes: u64,
    payload_resolve_ms: u128,
    copy_flush_ms: u128,
}
impl<'a, T: Status> LocalPhase<'a, T> {
    pub(super) fn new(
        root: &'a Path,
        status: &'a T,
        phase: &'static str,
        total: usize,
        range: (u32, u32),
    ) -> Self {
        let now = Instant::now();
        status.set_final(&format!("{phase}:0/{total}"), range.0);
        Self {
            root,
            status,
            phase,
            total,
            completed: 0,
            range,
            started: now,
            last: now,
            complete: false,
            cache_hits: 0,
            written_bytes: 0,
            payload_resolve_ms: 0,
            copy_flush_ms: 0,
        }
    }
    fn key(&self) -> String {
        format!("{}:{}/{}", self.phase, self.completed, self.total)
    }
    fn percent(&self) -> u32 {
        self.range.0
            + ((self.completed as u64) * (self.range.1 - self.range.0) as u64
                / self.total.max(1) as u64) as u32
    }
    pub(super) fn advance(&mut self, completed: usize) {
        self.completed = completed.min(self.total);
        let now = Instant::now();
        if self.completed < self.total
            && now.duration_since(self.last) >= Duration::from_millis(200)
        {
            self.status.set(&self.key(), self.percent());
            self.last = now;
        }
    }
    pub(super) fn finish(&mut self) {
        self.complete = true;
    }
}
impl<T: Status> Drop for LocalPhase<'_, T> {
    fn drop(&mut self) {
        self.status.set_final(&self.key(), self.percent());
        log(
            self.root,
            "local-phase-end",
            serde_json::json!({"phase":self.phase,"completed":self.completed,
            "total":self.total,"elapsedMs":self.started.elapsed().as_millis(),
            "outcome":if self.complete {"complete"} else {"unfinished"},
            "cacheHits":self.cache_hits,"writtenBytes":self.written_bytes,
            "payloadResolveMs":self.payload_resolve_ms,"copyFlushMs":self.copy_flush_ms}),
        );
    }
}

struct TransactionStatus<'a, T: Status> {
    status: &'a T,
}
impl<T: Status> transaction::ProgressObserver for TransactionStatus<'_, T> {
    fn observe(&mut self, phase: &'static str, completed: usize, total: usize, boundary: bool) {
        let (start, end) = match phase {
            "preparing" => (85, 90),
            "installing" => (90, 98),
            _ => (5, 19),
        };
        let percent =
            start + (completed as u64 * (end - start) as u64 / total.max(1) as u64) as u32;
        let key = format!("{phase}:{completed}/{total}");
        if boundary {
            self.status.set_final(&key, percent);
        } else if completed < total {
            self.status.set(&key, percent);
        }
    }
}
fn progress(status: &impl Status, key: &str, percent: u32) -> Result<()> {
    ensure!(!status.cancelled(), "update cancelled");
    status.set(key, percent);
    Ok(())
}
fn cms(bytes: &[u8], sig: &[u8]) -> Result<()> {
    crate::platform::verify_cms(bytes, sig, crate::SIGNING_KEY).map_err(anyhow::Error::msg)
}
fn feed_auth(bytes: &[u8], sig: &[u8], fresh: bool) -> Result<Feed> {
    let feed = Feed::parse(bytes, now(), fresh)?;
    crate::platform::verify_cms_at(bytes, sig, crate::SIGNING_KEY, feed.created_unix)
        .map_err(anyhow::Error::msg)?;
    Ok(feed)
}
fn fresh_discovery(
    root: &Path,
    source: &impl Source,
    status: &impl Status,
    transfers: &mut Transfers,
) -> Result<(Feed, Vec<u8>, Vec<u8>)> {
    for attempt in 1..=3 {
        let mut pair_bytes = 0u64;
        let (bytes, signature) = source.discovery_pair(&mut |additional| {
            pair_bytes = pair_bytes
                .checked_add(additional)
                .context("discovery pair transfer overflow")?;
            ensure!(pair_bytes <= 4 * 32769, "discovery pair transfer bound");
            transfers.wire_bytes = transfers
                .wire_bytes
                .checked_add(additional)
                .context("discovery transfer overflow")?;
            progress(status, "checking", 5)
        })?;
        ensure!(
            bytes.len() <= 32768 && signature.len() <= 32768,
            "discovery pair size bound"
        );
        // Schema, compatibility, time and bounds fail immediately. Only a CMS
        // mismatch can represent promotion between the two alias requests.
        let feed = Feed::parse(&bytes, now(), true)?;
        match crate::platform::verify_cms_at(
            &bytes,
            &signature,
            crate::SIGNING_KEY,
            feed.created_unix,
        ) {
            Ok(()) => return Ok((feed, bytes, signature)),
            Err(error) => {
                log(
                    root,
                    "discovery-pair-auth-failure",
                    serde_json::json!({"attempt":attempt,"reason":error,"wireBytes":transfers.wire_bytes}),
                );
                ensure!(
                    attempt < 3,
                    "discovery CMS authentication failed after three fresh pairs: {error}"
                );
            }
        }
    }
    unreachable!("bounded discovery attempts")
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn engine_at(root: &Path, digest: &str) -> Result<(Engine, PathBuf)> {
    ensure!(valid_hash(digest), "invalid engine pointer");
    let dir = safe::target(root, &format!("updater/engines/{digest}"))?;
    let bytes = safe::read_bounded(&dir.join("ENGINE.json"), 32768)?;
    let signature = safe::read_bounded(&dir.join("ENGINE.p7s"), 32768)?;
    let engine = Engine::parse(&bytes)?;
    crate::platform::verify_cms_at(&bytes, &signature, crate::SIGNING_KEY, engine.built_unix)
        .map_err(anyhow::Error::msg)?;
    ensure!(engine.exe_sha256 == digest, "engine generation mismatch");
    let exe = dir.join("Mir2Updater.exe");
    ensure!(
        safe::matches(
            &exe,
            &FileEntry {
                path: "Mir2Updater.exe".into(),
                size: engine.exe_size,
                sha256: engine.exe_sha256.clone()
            }
        )?,
        "engine executable hash mismatch"
    );
    Ok((engine, exe))
}
pub fn active_engine(root: &Path) -> Result<(Engine, PathBuf)> {
    let pointer = safe::read_bounded(&root.join("updater/active.txt"), 64)?;
    engine_at(root, std::str::from_utf8(&pointer)?)
}
fn local_game(root: &Path) -> Result<(Manifest, BTreeMap<String, Vec<u8>>)> {
    let mut meta = BTreeMap::new();
    for name in META {
        meta.insert(
            name.into(),
            safe::read_bounded(&root.join("game").join(name), MAX_META)?,
        );
    }
    cms(
        &meta["RELEASE-STATEMENT.json"],
        &meta["RELEASE-STATEMENT.p7s"],
    )?;
    let manifest = bind_game(&meta)?;
    Ok((manifest, meta))
}
pub fn verify_launch(root: &Path) -> Result<String> {
    let (manifest, meta) = local_game(root)?;
    ensure!(
        safe::matches(
            &root.join("game/mir2-platform-windows.exe"),
            manifest.map()["mir2-platform-windows.exe"]
        )?,
        "installed game executable failed verification"
    );
    let attest = manifest.map()["BUILD-ATTESTATION.json"];
    ensure!(
        safe::matches(&root.join("game/BUILD-ATTESTATION.json"), attest)?,
        "installed build attestation failed verification"
    );
    let version: serde_json::Value = serde_json::from_slice(&meta["VERSION.json"])?;
    Ok(version["candidate"]
        .as_str()
        .context("missing candidate")?
        .to_owned())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReceiptSource {
    Highest,
    Accepted,
    Seed,
}
struct LocalReceipt {
    source: ReceiptSource,
    feed: Feed,
    bytes: Vec<u8>,
}
/// Each input has already passed CMS/key/schema verification. Installation seeds
/// and retained history jointly define the floor; a new trusted installer must
/// not be downgraded merely because it preserved an older highest pointer.
fn select_previous(
    mut receipts: Vec<LocalReceipt>,
    installed_version: &[u8],
) -> Result<Option<(Feed, Vec<u8>)>> {
    ensure!(
        !receipts.is_empty(),
        "signed installer seed/release history is missing"
    );
    let mut sequences = BTreeMap::new();
    let mut selected = 0;
    let has_history = receipts.iter().any(|r| r.source != ReceiptSource::Seed);
    for (index, receipt) in receipts.iter().enumerate() {
        if let Some(existing) = sequences.insert(receipt.feed.sequence, receipt.bytes.as_slice()) {
            ensure!(
                existing == receipt.bytes,
                "conflicting signed local release sequence"
            );
        }
        if receipt.feed.sequence > receipts[selected].feed.sequence {
            selected = index;
        }
        if !has_history && receipt.source == ReceiptSource::Seed {
            ensure!(
                receipt
                    .feed
                    .game
                    .metadata
                    .iter()
                    .any(|e| { e.path == "VERSION.json" && e.sha256 == hash(installed_version) }),
                "updated installation lost its authenticated release history"
            );
        }
    }
    let chosen = receipts.swap_remove(selected);
    Ok(Some((chosen.feed, chosen.bytes)))
}
fn read_previous(root: &Path) -> Result<Option<(Feed, Vec<u8>)>> {
    let mut receipts = Vec::new();
    let highest = root.join(".update/highest.txt");
    if highest.exists() {
        let pointer = safe::read_bounded(&highest, 64)?;
        let digest = std::str::from_utf8(&pointer)?;
        ensure!(valid_hash(digest), "invalid highest-release pointer");
        let dir = safe::target(root, &format!(".update/receipts/{digest}"))?;
        let bytes = safe::read_bounded(&dir.join("FEED.json"), 32768)?;
        let sig = safe::read_bounded(&dir.join("FEED.p7s"), 32768)?;
        ensure!(hash(&bytes) == digest, "highest-release hash mismatch");
        receipts.push(LocalReceipt {
            source: ReceiptSource::Highest,
            feed: feed_auth(&bytes, &sig, false)?,
            bytes,
        });
    }
    for (source, json, sig) in [
        (
            ReceiptSource::Accepted,
            ".update/accepted-feed.json",
            ".update/accepted-feed.p7s",
        ),
        (
            ReceiptSource::Seed,
            "updater/seed-feed.json",
            "updater/seed-feed.p7s",
        ),
    ] {
        let p = root.join(json);
        if p.exists() {
            let bytes = safe::read_bounded(&p, 32768)?;
            let signature = safe::read_bounded(&root.join(sig), 32768)?;
            receipts.push(LocalReceipt {
                source,
                feed: feed_auth(&bytes, &signature, false)?,
                bytes,
            });
        }
    }
    let version = safe::read_bounded(&root.join("game/VERSION.json"), MAX_META)?;
    select_previous(receipts, &version)
}
fn remember_highest(root: &Path, bytes: &[u8], sig: &[u8]) -> Result<()> {
    let digest = hash(bytes);
    let dir = safe::target(root, &format!(".update/receipts/{digest}"))?;
    safe::atomic_write(&dir.join("FEED.json"), bytes)?;
    safe::atomic_write(&dir.join("FEED.p7s"), sig)?;
    safe::atomic_write(&root.join(".update/highest.txt"), digest.as_bytes())
}
fn remote_meta(
    source: &impl Source,
    component: &Component,
    status: &impl Status,
    transfers: &mut Transfers,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    for entry in &component.metadata {
        let bytes = fetched_bytes(
            source,
            &format!("{}/{}", component.directory, entry.path),
            entry.size,
            status,
            transfers,
        )?;
        ensure!(
            bytes.len() as u64 == entry.size && hash(&bytes) == entry.sha256,
            "metadata hash mismatch"
        );
        files.insert(entry.path.clone(), bytes);
    }
    Ok(files)
}
#[derive(Default)]
pub(super) struct Transfers {
    downloaded_files: usize,
    downloaded_bytes: u64,
    wire_bytes: u64,
}
fn fetched_bytes(
    source: &impl Source,
    path: &str,
    limit: u64,
    status: &impl Status,
    transfers: &mut Transfers,
) -> Result<Vec<u8>> {
    let mut received = 0;
    let result = source.get(path, limit, &mut |bytes| {
        ensure!(
            bytes <= limit.saturating_add(1),
            "source response progress exceeds read bound"
        );
        let advanced = bytes.saturating_sub(received);
        received = received.max(bytes);
        transfers.wire_bytes += advanced;
        ensure!(bytes <= limit, "source response exceeds bound");
        progress(status, "checking", 10)
    });
    if let Ok(bytes) = &result {
        ensure!(bytes.len() as u64 <= limit, "source response exceeds bound");
        transfers.wire_bytes += (bytes.len() as u64).saturating_sub(received);
    }
    result
}
pub(super) fn cached_payload(
    root: &Path,
    source: &impl Source,
    path: &str,
    entry: &FileEntry,
    status: &impl Status,
    download_progress: (u64, u64),
    transfers: &mut Transfers,
) -> Result<(PathBuf, bool)> {
    let (done, total) = download_progress;
    let cache = safe::target(root, &format!(".update/downloads/{}", entry.sha256))?;
    if cache.exists() && safe::matches(&cache, entry)? {
        return Ok((cache, false));
    }
    if cache.exists() {
        safe::regular(&cache)?;
        fs::remove_file(&cache)?;
    }
    let part = cache.with_extension("part");
    if part.exists() {
        safe::regular(&part)?;
        fs::remove_file(&part)?;
    }
    let mut received = 0;
    transfers.downloaded_files += 1;
    let result = source.download(path, entry, &part, &mut |bytes| {
        ensure!(
            bytes <= entry.size.saturating_add(1),
            "source payload progress exceeds read bound"
        );
        let advanced = bytes.saturating_sub(received);
        received = received.max(bytes);
        transfers.downloaded_bytes += advanced;
        transfers.wire_bytes += advanced;
        ensure!(bytes <= entry.size, "source payload exceeds bound");
        progress(
            status,
            "downloading",
            (30 + ((done + bytes) * 45 / total.max(1)) as u32).min(75),
        )
    });
    let result = result.and_then(|()| {
        ensure!(safe::matches(&part, entry)?, "download hash mismatch");
        Ok(())
    });
    if let Err(e) = result {
        if part.exists() {
            safe::regular(&part)?;
            fs::remove_file(&part)?;
        }
        return Err(e);
    }
    // Source adapters should report streaming bytes. A successful bounded,
    // hash-verified adapter that omits the final callback still has exact size.
    let remaining = entry.size.saturating_sub(received);
    transfers.downloaded_bytes += remaining;
    transfers.wire_bytes += remaining;
    crate::platform::replace_file(&part, &cache)?;
    Ok((cache, true))
}
fn optional_delivery(
    root: &Path,
    source: &impl Source,
    component: &Component,
    target: (&BTreeMap<String, Vec<u8>>, &Manifest),
    status: &impl Status,
    transfers: &mut Transfers,
    stats: &mut delivery::Stats,
) -> Result<Option<delivery::Delivery>> {
    let (metadata, manifest) = target;
    let bytes = match fetched_bytes(
        source,
        &format!("{}/DELIVERY.json", component.directory),
        MAX_META,
        status,
        transfers,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            ensure!(!status.cancelled(), "update cancelled");
            log(root, "delivery-unavailable", format!("{error:#}"));
            return Ok(None);
        }
    };
    let result = (|| -> Result<delivery::Delivery> {
        let descriptor = delivery::Delivery::parse(
            &bytes,
            &component.identity,
            &metadata["PACKAGE-MANIFEST.json"],
            &metadata["VERSION.json"],
            manifest,
            now(),
        )?;
        let signature = fetched_bytes(
            source,
            &format!("{}/DELIVERY.p7s", component.directory),
            32768,
            status,
            transfers,
        )?;
        crate::platform::verify_cms_at(
            &bytes,
            &signature,
            crate::SIGNING_KEY,
            descriptor.built_unix,
        )
        .map_err(anyhow::Error::msg)?;
        Ok(descriptor)
    })();
    match result {
        Ok(descriptor) => Ok(Some(descriptor)),
        Err(error) => {
            ensure!(!status.cancelled(), "update cancelled");
            stats.fallbacks += 1;
            log(root, "delivery-descriptor-fallback", format!("{error:#}"));
            Ok(None)
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub candidate: String,
    pub sequence: u64,
    pub downloaded_files: usize,
    pub downloaded_bytes: u64,
    pub wire_bytes: u64,
    pub delivery: delivery::Stats,
    pub changed_game_files: usize,
    pub activated: bool,
}
use serde::Serialize;
fn stage_payloads(
    root: &Path,
    source: &impl Source,
    directory: &str,
    status: &impl Status,
    pending: &[FileEntry],
    total: u64,
    transfers: &mut Transfers,
) -> Result<Vec<Change>> {
    let mut changes = vec![];
    let mut done = 0;
    let mut phase = LocalPhase::new(root, status, "staging", pending.len(), (70, 84));
    for (index, entry) in pending.iter().enumerate() {
        ensure!(!status.cancelled(), "update cancelled");
        let resolve_started = Instant::now();
        let (cache, downloaded) = cached_payload(
            root,
            source,
            &format!("{directory}/{}", entry.path),
            entry,
            status,
            (done, total),
            transfers,
        )?;
        phase.payload_resolve_ms += resolve_started.elapsed().as_millis();
        phase.cache_hits += usize::from(!downloaded);
        let copy_started = Instant::now();
        let target = safe::target(
            &root.join(".update/staging"),
            &format!("game/{}", entry.path),
        )?;
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(cache, &target)?;
        fs::OpenOptions::new()
            .write(true)
            .open(&target)?
            .sync_all()?;
        let mut new = entry.clone();
        new.path = format!("game/{}", entry.path);
        changes.push(Change {
            path: new.path.clone(),
            new: Some(new),
        });
        done += entry.size;
        phase.written_bytes += entry.size;
        phase.copy_flush_ms += copy_started.elapsed().as_millis();
        phase.advance(index + 1);
    }
    phase.finish();
    Ok(changes)
}

pub fn check_update(root: &Path, source: &impl Source, status: &impl Status) -> Result<Outcome> {
    let throttled = ThrottledStatus::new(status);
    let status = &throttled;
    progress(status, "checking", 0)?;
    ensure!(
        !crate::platform::game_is_running(&root.join("game/mir2-platform-windows.exe"))?,
        "game is already running"
    );
    if transaction::recover_observed(root, &mut TransactionStatus { status })? {
        progress(status, "recovering", 5)?;
    }
    let (old, old_meta) = local_game(root)?;
    let old_metadata_bytes = old_meta.values().map(|b| b.len() as u64).sum::<u64>();
    let (old_engine, _) = active_engine(root)?;
    let previous = read_previous(root)?;
    // A check-only run or power loss after commit must still receive first-launch
    // health/rollback. Never overwrite its only backup with a subsequent update.
    if transaction::pending_activation(root)? {
        let candidate = verify_launch(root)?;
        return Ok(Outcome {
            candidate,
            sequence: previous.as_ref().map_or(0, |(f, _)| f.sequence),
            downloaded_files: 0,
            downloaded_bytes: 0,
            wire_bytes: 0,
            delivery: delivery::Stats::default(),
            changed_game_files: 0,
            activated: true,
        });
    }
    let mut transfers = Transfers::default();
    let (feed, bytes, signature) = fresh_discovery(root, source, status, &mut transfers)?;
    feed.check_advance(&bytes, previous.as_ref().map(|(f, b)| (f, b.as_slice())))?;
    let failed = root.join(".update/failed-release.txt");
    if failed.exists() {
        ensure!(
            safe::read_bounded(&failed, 64)? != hash(&bytes).as_bytes(),
            "release quarantined after unsuccessful first launch"
        );
    }
    remember_highest(root, &bytes, &signature)?;
    let game_same = feed.game.metadata.iter().all(|e| {
        old_meta
            .get(&e.path)
            .is_some_and(|b| hash(b) == e.sha256 && b.len() as u64 == e.size)
    });
    let old_version: serde_json::Value = serde_json::from_slice(&old_meta["VERSION.json"])?;
    let mut outcome = Outcome {
        candidate: old_version["candidate"]
            .as_str()
            .context("candidate")?
            .into(),
        sequence: feed.sequence,
        downloaded_files: 0,
        downloaded_bytes: 0,
        wire_bytes: 0,
        delivery: delivery::Stats::default(),
        changed_game_files: 0,
        activated: false,
    };
    let engine_meta = remote_meta(source, &feed.engine, status, &mut transfers)?;
    let engine = Engine::parse(&engine_meta["ENGINE.json"])?;
    crate::platform::verify_cms_at(
        &engine_meta["ENGINE.json"],
        &engine_meta["ENGINE.p7s"],
        crate::SIGNING_KEY,
        engine.built_unix,
    )
    .map_err(anyhow::Error::msg)?;
    ensure!(
        engine.version >= old_engine.version && engine.version.to_string() == feed.engine.identity,
        "engine downgrade/identity mismatch"
    );
    ensure!(
        engine.version <= crate::ENGINE_VERSION || engine.exe_sha256 != old_engine.exe_sha256,
        "engine version mismatch"
    );
    if game_same
        && engine.exe_sha256 == old_engine.exe_sha256
        && previous
            .as_ref()
            .is_some_and(|(_, old_bytes)| old_bytes == &bytes)
        && verify_launch(root).is_ok()
    {
        outcome.wire_bytes = transfers.wire_bytes;
        return Ok(outcome);
    }
    let metadata = if game_same {
        old_meta
    } else {
        remote_meta(source, &feed.game, status, &mut transfers)?
    };
    cms(
        &metadata["RELEASE-STATEMENT.json"],
        &metadata["RELEASE-STATEMENT.p7s"],
    )?;
    let new = bind_game(&metadata)?;
    let version: serde_json::Value = serde_json::from_slice(&metadata["VERSION.json"])?;
    ensure!(
        version["candidate"].as_str() == Some(&feed.game.identity),
        "candidate/feed mismatch"
    );
    outcome.candidate = feed.game.identity.clone();
    progress(status, "verifying", 20)?;
    let mut scan = LocalPhase::new(root, status, "checking-installed", new.file_count, (20, 30));
    let old_files = old.map();
    let new_files = new.map();
    let mut pending = vec![];
    let mut removed = vec![];
    // Hash on update, not on each launch. Missing/corrupted assets are repaired.
    for (index, entry) in new.files.iter().enumerate() {
        if index % 256 == 0 {
            ensure!(!status.cancelled(), "update cancelled");
        }
        if !safe::matches(&safe::target(&root.join("game"), &entry.path)?, entry)? {
            pending.push(entry.clone());
        }
        scan.advance(index + 1);
    }
    scan.finish();
    drop(scan);
    for (path, old_entry) in &old_files {
        if !new_files.contains_key(path) && root.join("game").join(path).exists() {
            ensure!(
                safe::matches(&safe::target(&root.join("game"), path)?, old_entry)?,
                "removed managed file was modified"
            );
            removed.push((*path).to_owned());
        }
    }
    // Existing v1 releases have no accelerator, and zero-payload checks do not
    // request a potentially large optional descriptor.
    let mut delivery = if pending.is_empty() {
        None
    } else {
        optional_delivery(
            root,
            source,
            &feed.game,
            (&metadata, &new),
            status,
            &mut transfers,
            &mut outcome.delivery,
        )?
    };
    let engine_needed = engine.exe_sha256 != old_engine.exe_sha256;
    let total = pending.iter().map(|f| f.size).sum::<u64>()
        + if engine_needed { engine.exe_size } else { 0 };
    let mut backups = old_metadata_bytes;
    let mut largest = 0;
    for entry in &pending {
        let path = safe::target(&root.join("game"), &entry.path)?;
        let old_size = if path.exists() {
            safe::regular(&path)?.len()
        } else {
            0
        };
        backups += old_size;
        largest = largest.max(old_size).max(entry.size);
    }
    for path in &removed {
        backups += safe::regular(&root.join("game").join(path))?.len();
    }
    let metadata_bytes = metadata.values().map(|b| b.len() as u64).sum::<u64>();
    largest = largest.max(metadata_bytes.min(MAX_META));
    let required = total.saturating_mul(2) + metadata_bytes + backups + largest + 64 * 1024 * 1024;
    let available = crate::platform::free_bytes(root)?;
    ensure!(
        total <= MAX_TOTAL && available > required,
        "insufficient update and rollback space"
    );
    if delivery.as_ref().is_some_and(|descriptor| {
        available <= required.saturating_add(descriptor.extra_cache_bytes())
    }) {
        outcome.delivery.fallbacks += 1;
        log(
            root,
            "delivery-space-fallback",
            "insufficient additional accelerator cache space",
        );
        delivery = None;
    }
    safe::clear_scratch(root, "staging")?;
    if let Some(delivery) = &delivery {
        delivery::Acceleration {
            root,
            source,
            component: &feed.game,
            status,
            total,
            transfers: &mut transfers,
            stats: &mut outcome.delivery,
        }
        .run(delivery, &new, &pending)?;
    }
    let mut changes = stage_payloads(
        root,
        source,
        &feed.game.directory,
        status,
        &pending,
        total,
        &mut transfers,
    )?;
    let done = pending.iter().map(|entry| entry.size).sum::<u64>();
    outcome.changed_game_files = pending.len() + removed.len();
    for path in removed {
        changes.push(Change {
            path: format!("game/{path}"),
            new: None,
        });
    }
    // Candidate metadata comes last, then the immutable engine pointer and receipt.
    for name in META {
        changes.push(transaction::stage_bytes(
            root,
            &format!("game/{name}"),
            &metadata[name],
        )?);
    }
    if engine_needed {
        let entry = FileEntry {
            path: "Mir2Updater.exe".into(),
            size: engine.exe_size,
            sha256: engine.exe_sha256.clone(),
        };
        let (cache, _) = cached_payload(
            root,
            source,
            &format!("{}/Mir2Updater.exe", feed.engine.directory),
            &entry,
            status,
            (done, total),
            &mut transfers,
        )?;
        let dir = safe::target(root, &format!("updater/engines/{}", engine.exe_sha256))?;
        fs::create_dir_all(&dir)?;
        for name in ["ENGINE.json", "ENGINE.p7s"] {
            let dest = dir.join(name);
            // This generation is inactive. Atomic writes also repair an incomplete
            // generation left by power loss before its pointer was published.
            safe::atomic_write(&dest, &engine_meta[name])?;
        }
        let dest = dir.join("Mir2Updater.exe");
        if !safe::matches(&dest, &entry)? {
            let temp = dir.join("Mir2Updater.download");
            safe::ancestors(&temp)?;
            if temp.exists() {
                safe::regular(&temp)?;
                fs::remove_file(&temp)?;
            }
            fs::copy(cache, &temp)?;
            fs::OpenOptions::new().write(true).open(&temp)?.sync_all()?;
            ensure!(
                safe::matches(&temp, &entry)?,
                "engine copy failed validation"
            );
            crate::platform::replace_file(&temp, &dest)?;
        }
        engine_at(root, &engine.exe_sha256)?;
    }
    changes.push(transaction::stage_bytes(
        root,
        "updater/active.txt",
        engine.exe_sha256.as_bytes(),
    )?);
    changes.push(transaction::stage_bytes(
        root,
        ".update/accepted-feed.json",
        &bytes,
    )?);
    changes.push(transaction::stage_bytes(
        root,
        ".update/accepted-feed.p7s",
        &signature,
    )?);
    progress(status, "applying", 85)?;
    if let Err(error) =
        transaction::prepare_and_apply_observed(root, &changes, &mut TransactionStatus { status })
    {
        transaction::recover_observed(root, &mut TransactionStatus { status })
            .context("activation failed AND rollback failed; do not launch")?;
        return Err(error);
    }
    if let Err(error) = verify_launch(root) {
        transaction::rollback_observed(root, &mut TransactionStatus { status })?;
        return Err(error);
    }
    progress(status, "launching", 100)?;
    outcome.downloaded_files = transfers.downloaded_files;
    outcome.downloaded_bytes = transfers.downloaded_bytes;
    outcome.wire_bytes = transfers.wire_bytes;
    outcome.activated = true;
    Ok(outcome)
}
pub fn quarantine_and_rollback(root: &Path) -> Result<()> {
    let bytes = safe::read_bounded(&root.join(".update/accepted-feed.json"), 32768)?;
    let sig = safe::read_bounded(&root.join(".update/accepted-feed.p7s"), 32768)?;
    feed_auth(&bytes, &sig, false)?;
    safe::atomic_write(
        &root.join(".update/failed-release.txt"),
        hash(&bytes).as_bytes(),
    )?;
    transaction::rollback(root)
}
pub fn log(root: &Path, event: &str, detail: impl serde::Serialize) {
    let result = (|| -> Result<()> {
        let path = root.join(".update/last-run.jsonl");
        safe::ancestors(&path)?;
        fs::create_dir_all(path.parent().unwrap())?;
        if path.exists() && safe::regular(&path)?.len() > 1024 * 1024 {
            crate::platform::replace_file(&path, &path.with_extension("previous.jsonl"))?;
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        serde_json::to_writer(
            &mut file,
            &serde_json::json!({"time":now(),"event":event,"detail":detail}),
        )?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    })();
    if result.is_err() {} // Logging cannot permit or block an unverified launch.
}

#[cfg(test)]
#[path = "update_receipt_tests.rs"]
mod receipt_tests;

#[cfg(test)]
#[path = "progress_trace_tests.rs"]
mod progress_trace_tests;
