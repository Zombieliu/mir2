//! Actual loopback HTTP/body/file regressions. HTTPS/CMS and public CDN
//! acceptance remain separate; production origins and redirect rules stay pinned.
use super::*;
use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

struct Reply {
    source: HttpsSource,
    request: Arc<Mutex<Vec<String>>>,
    server: thread::JoinHandle<()>,
}
impl Reply {
    fn new(status: &str, headers: &str, body: Vec<u8>) -> Self {
        Self::many(vec![(status.to_owned(), headers.to_owned(), body)])
    }
    fn many(replies: Vec<(String, String, Vec<u8>)>) -> Self {
        Self::raw_many(
            replies
                .into_iter()
                .map(|(status, headers, body)| (status, headers.into_bytes(), body))
                .collect(),
        )
    }
    fn raw_many(replies: Vec<(String, Vec<u8>, Vec<u8>)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = url::Url::parse(&format!(
            "http://{}/updates/",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let request = Arc::new(Mutex::new(Vec::new()));
        let captured = request.clone();
        let server = thread::spawn(move || {
            for (status, headers, body) in replies {
                let mut response =
                    format!("HTTP/1.1 {status}\r\nConnection: close\r\n").into_bytes();
                response.extend_from_slice(&headers);
                response.extend_from_slice(b"\r\n");
                let start = Instant::now();
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                start.elapsed() < Duration::from_secs(5),
                                "HTTP test request missing"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(e) => panic!("HTTP test accept: {e}"),
                    }
                };
                // Windows may inherit the nonblocking listener mode. This
                // response fixture intentionally uses bounded blocking I/O;
                // a read timeout alone does not clear nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0u8; 1024];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    assert!(n != 0 && bytes.len() + n <= 16384);
                    bytes.extend_from_slice(&buf[..n]);
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(bytes).unwrap());
                // Rejection/cancellation may close the reader before the response ends.
                let _ = stream.write_all(&response);
                let _ = stream.write_all(&body);
            }
        });
        let source = HttpsSource {
            base: base.clone(),
            legacy_base: base,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_connect(Duration::from_secs(2))
                .timeout_read(Duration::from_secs(2))
                .build(),
        };
        Self {
            source,
            request,
            server,
        }
    }
    fn finish(self) -> Vec<String> {
        self.server.join().unwrap();
        Arc::try_unwrap(self.request).unwrap().into_inner().unwrap()
    }
}

struct Quiet;
impl Status for Quiet {
    fn set(&self, _: &str, _: u32) {}
    fn cancelled(&self) -> bool {
        false
    }
}
fn entry(data: &[u8]) -> FileEntry {
    FileEntry {
        path: "delivery/resource.m2b.gz".into(),
        size: data.len() as u64,
        sha256: hash(data),
    }
}
fn part(root: &Path, entry: &FileEntry) -> PathBuf {
    root.join(format!(".update/downloads/{}.part", entry.sha256))
}

#[test]
fn red_interrupted_http_preserves_prefix_then_reopen_requests_only_remaining_bytes() {
    let root = tempfile::tempdir().unwrap();
    let data: Vec<u8> = (0..262144).map(|i| (i % 251) as u8).collect();
    let file = entry(&data);
    let interrupted = Reply::new(
        "200 OK",
        &format!("Content-Length: {}\r\n", data.len()),
        data[..131072].to_vec(),
    );
    let mut first = Transfers::default();
    assert!(cached_payload(
        root.path(),
        &interrupted.source,
        "releases/test/resource.m2b.gz",
        &file,
        &Quiet,
        (0, file.size),
        &mut first
    )
    .is_err());
    let request = interrupted.finish();
    assert_eq!(request.len(), 1);
    assert!(!request[0].to_ascii_lowercase().contains("range:"));
    let prefix = safe::read_bounded(&part(root.path(), &file), file.size)
        .expect("interruption must retain the partial resource");
    assert!(!prefix.is_empty() && prefix.len() < data.len());
    assert_eq!(prefix, data[..prefix.len()]);
    let offset = prefix.len();
    let remaining = Reply::new(
        "206 Partial Content",
        &format!(
            "Content-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n",
            data.len() - offset,
            offset,
            data.len() - 1,
            data.len()
        ),
        data[offset..].to_vec(),
    );
    let mut second = Transfers::default();
    let (cache, downloaded) = cached_payload(
        root.path(),
        &remaining.source,
        "releases/test/resource.m2b.gz",
        &file,
        &Quiet,
        (0, file.size),
        &mut second,
    )
    .unwrap();
    let requests = remaining.finish();
    assert_eq!(requests.len(), 1);
    let request = requests[0].to_ascii_lowercase();
    assert!(request.contains(&format!("range: bytes={offset}-\r\n")));
    assert!(request.contains("accept-encoding: identity\r\n"));
    assert!(downloaded && safe::matches(&cache, &file).unwrap());
    assert_eq!(second.wire_bytes, (data.len() - offset) as u64);
    assert_eq!(second.downloaded_bytes, second.wire_bytes);
    assert!(!part(root.path(), &file).exists());
    assert!(
        !root.path().join("game").exists(),
        "download does not install or launch"
    );
}

fn write_prefix(root: &Path, file: &FileEntry, bytes: &[u8]) {
    safe::write_new(&part(root, file), bytes).unwrap();
}
fn fetch(
    root: &Path,
    source: &HttpsSource,
    file: &FileEntry,
    status: &impl Status,
) -> (Result<(PathBuf, bool)>, Transfers) {
    let mut transfers = Transfers::default();
    let result = cached_payload(
        root,
        source,
        "releases/test/resource.m2b.gz",
        file,
        status,
        (0, file.size),
        &mut transfers,
    );
    (result, transfers)
}
fn full_headers(size: usize) -> String {
    format!("Content-Length: {size}\r\n")
}
fn range_headers(offset: usize, size: usize) -> String {
    format!(
        "Content-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n",
        size - offset,
        offset,
        size - 1,
        size
    )
}
fn no_http_source() -> (HttpsSource, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = url::Url::parse(&format!(
        "http://{}/updates/",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    (
        HttpsSource {
            base: base.clone(),
            legacy_base: base,
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_read(Duration::from_secs(1))
                .build(),
        },
        listener,
    )
}
fn no_request(listener: &TcpListener) {
    assert!(
        matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "unexpected HTTP request"
    );
}

#[test]
fn cancel_before_write_uses_actual_prefix_length_and_preserves_personal_files() {
    use std::cell::Cell;
    struct Cancel {
        snapshots: Cell<usize>,
    }
    impl Status for Cancel {
        fn set(&self, key: &str, _: u32) {
            if key == "downloading" {
                self.snapshots.set(self.snapshots.get() + 1);
            }
        }
        fn cancelled(&self) -> bool {
            self.snapshots.get() >= 3
        }
    }
    let root = tempfile::tempdir().unwrap();
    let personal = root.path().join("game/Config/owned-personal.json");
    safe::write_new(&personal, b"preserve my preferences").unwrap();
    let data: Vec<u8> = (0..262144).map(|i| (i % 251) as u8).collect();
    let file = entry(&data);
    let first = Reply::new("200 OK", &full_headers(data.len()), data.clone());
    let (result, transfers) = fetch(
        root.path(),
        &first.source,
        &file,
        &Cancel {
            snapshots: Cell::new(0),
        },
    );
    assert!(result.is_err());
    first.finish();
    let prefix = safe::read_bounded(&part(root.path(), &file), file.size).unwrap();
    assert!(!prefix.is_empty() && prefix.len() < data.len());
    assert_eq!(prefix, data[..prefix.len()]);
    assert!(
        transfers.wire_bytes > prefix.len() as u64,
        "cancelled read was not written"
    );
    let next = Reply::new(
        "206 Partial Content",
        &range_headers(prefix.len(), data.len()),
        data[prefix.len()..].to_vec(),
    );
    let (result, resumed) = fetch(root.path(), &next.source, &file, &Quiet);
    assert!(safe::matches(&result.unwrap().0, &file).unwrap());
    assert!(next.finish()[0]
        .to_ascii_lowercase()
        .contains(&format!("range: bytes={}-\r\n", prefix.len())));
    assert_eq!(resumed.wire_bytes, file.size - prefix.len() as u64);
    assert_eq!(fs::read(personal).unwrap(), b"preserve my preferences");
}

#[test]
fn range_ignored_200_resets_prefix_instead_of_appending() {
    let root = tempfile::tempdir().unwrap();
    let data = b"server returns the whole immutable object";
    let file = entry(data);
    write_prefix(root.path(), &file, &data[..7]);
    let reply = Reply::new("200 OK", &full_headers(data.len()), data.to_vec());
    let (result, transfers) = fetch(root.path(), &reply.source, &file, &Quiet);
    assert!(safe::matches(&result.unwrap().0, &file).unwrap());
    assert!(reply.finish()[0]
        .to_ascii_lowercase()
        .contains("range: bytes=7-\r\n"));
    assert_eq!(transfers.wire_bytes, file.size);
}

#[test]
fn complete_valid_prefix_recovers_with_zero_requests_and_bad_or_oversize_restarts() {
    let data = b"complete immutable payload";
    let file = entry(data);
    let root = tempfile::tempdir().unwrap();
    write_prefix(root.path(), &file, data);
    let (source, listener) = no_http_source();
    let (result, transfers) = fetch(root.path(), &source, &file, &Quiet);
    let (cache, downloaded) = result.unwrap();
    assert!(!downloaded && safe::matches(&cache, &file).unwrap());
    assert_eq!(transfers.wire_bytes, 0);
    assert_eq!(transfers.downloaded_files, 0);
    no_request(&listener);
    for stale in [vec![b'x'; data.len()], vec![b'x'; data.len() + 1], vec![]] {
        let root = tempfile::tempdir().unwrap();
        write_prefix(root.path(), &file, &stale);
        let reply = Reply::new("200 OK", &full_headers(data.len()), data.to_vec());
        let (result, transfers) = fetch(root.path(), &reply.source, &file, &Quiet);
        assert!(safe::matches(&result.unwrap().0, &file).unwrap());
        assert!(!reply.finish()[0].to_ascii_lowercase().contains("range:"));
        assert_eq!(transfers.wire_bytes, file.size);
    }
}

#[test]
fn exact_416_total_allows_one_same_origin_full_restart() {
    let root = tempfile::tempdir().unwrap();
    let data = b"immutable object has a fixed size";
    let file = entry(data);
    write_prefix(root.path(), &file, &data[..4]);
    let reply = Reply::many(vec![
        (
            "416 Range Not Satisfiable".into(),
            format!(
                "Content-Length: 0\r\nContent-Range: bytes */{}\r\n",
                data.len()
            ),
            vec![],
        ),
        ("200 OK".into(), full_headers(data.len()), data.to_vec()),
    ]);
    let (result, transfers) = fetch(root.path(), &reply.source, &file, &Quiet);
    assert!(safe::matches(&result.unwrap().0, &file).unwrap());
    let requests = reply.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("range: bytes=4-\r\n"));
    assert!(!requests[1].to_ascii_lowercase().contains("range:"));
    assert_eq!(transfers.wire_bytes, file.size);
}

#[test]
fn malformed_range_encoding_lengths_multipart_and_redirect_fail_closed() {
    let data = b"0123456789abcdef";
    let file = entry(data);
    let mut cases: Vec<(String, String)> = [
        "",
        "Content-Range: bytes 3-15/16\r\n",
        "Content-Range: bytes 4-14/16\r\n",
        "Content-Range: bytes 4-15/17\r\n",
        "Content-Range: bytes 4-15/*\r\n",
        "Content-Range: items 4-15/16\r\n",
        "Content-Range: bytes +4-15/16\r\n",
        "Content-Range: bytes 4-15/18446744073709551616\r\n",
        "Content-Range: bytes 4-15/16\r\nContent-Range: bytes 4-15/16\r\n",
        "Content-Range: bytes 4-15/16\r\nContent-Encoding: gzip\r\n",
        "Content-Range: bytes 4-15/16\r\nContent-Encoding: identity\r\nContent-Encoding: gzip\r\n",
        "Content-Range: bytes 4-15/16\r\nContent-Type: multipart/byteranges; boundary=evil\r\n",
        "Content-Range: bytes 4-15/16\r\nContent-Length: 11\r\n",
    ]
    .into_iter()
    .map(|h| ("206 Partial Content".into(), h.into()))
    .collect();
    cases.push(("200 OK".into(), "Content-Range: bytes 4-15/16\r\n".into()));
    cases.push((
        "302 Found".into(),
        "Location: http://127.0.0.1:9/unsafe\r\n".into(),
    ));
    cases.push((
        "416 Range Not Satisfiable".into(),
        "Content-Range: bytes */17\r\n".into(),
    ));
    for (status, headers) in cases {
        let root = tempfile::tempdir().unwrap();
        write_prefix(root.path(), &file, &data[..4]);
        let reply = Reply::new(&status, &headers, data[4..].to_vec());
        let (result, _) = fetch(root.path(), &reply.source, &file, &Quiet);
        assert!(result.is_err(), "accepted {status}/{headers}");
        assert_eq!(reply.finish().len(), 1);
        assert!(
            !part(root.path(), &file).exists(),
            "integrity failure retained {status}/{headers}"
        );
        assert!(!root
            .path()
            .join(format!(".update/downloads/{}", file.sha256))
            .exists());
    }
}

#[test]
fn bad_prefix_sha_and_oversize_responses_discard_untrusted_parts() {
    let data = b"0123456789abcdef";
    let file = entry(data);
    for oversize in [false, true] {
        let root = tempfile::tempdir().unwrap();
        write_prefix(
            root.path(),
            &file,
            if oversize { &data[..4] } else { b"evil" },
        );
        let mut body = data[4..].to_vec();
        let headers = if oversize {
            body.push(0);
            "Content-Range: bytes 4-15/16\r\n".to_owned()
        } else {
            range_headers(4, data.len())
        };
        let reply = Reply::new("206 Partial Content", &headers, body);
        let (result, _) = fetch(root.path(), &reply.source, &file, &Quiet);
        assert!(result.is_err());
        reply.finish();
        assert!(!part(root.path(), &file).exists());
        assert!(!root
            .path()
            .join(format!(".update/downloads/{}", file.sha256))
            .exists());
    }
}

#[test]
fn unsolicited_206_and_full_bad_sha_do_not_publish_or_retain() {
    let data = b"0123456789abcdef";
    let file = entry(data);
    for (status, headers, body) in [
        (
            "206 Partial Content",
            range_headers(0, data.len()),
            data.to_vec(),
        ),
        ("200 OK", full_headers(data.len()), vec![b'x'; data.len()]),
    ] {
        let root = tempfile::tempdir().unwrap();
        let reply = Reply::new(status, &headers, body);
        let (result, _) = fetch(root.path(), &reply.source, &file, &Quiet);
        assert!(result.is_err());
        reply.finish();
        assert!(!part(root.path(), &file).exists());
        assert!(!root
            .path()
            .join(format!(".update/downloads/{}", file.sha256))
            .exists());
    }
}

#[test]
fn partial_hardlink_is_rejected_before_http_and_external_target_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let witness = outside.path().join("owned-witness");
    safe::write_new(&witness, b"external prefix").unwrap();
    let file = entry(b"external prefix and complete body");
    let partial = part(root.path(), &file);
    fs::create_dir_all(partial.parent().unwrap()).unwrap();
    fs::hard_link(&witness, &partial).unwrap();
    let (source, listener) = no_http_source();
    let (result, transfers) = fetch(root.path(), &source, &file, &Quiet);
    assert!(result.is_err());
    assert_eq!(transfers.wire_bytes, 0);
    assert_eq!(fs::read(&witness).unwrap(), b"external prefix");
    assert!(partial.exists(), "refuse rather than mutate unsafe scratch");
    no_request(&listener);
    assert!(safe::open_download(&partial).is_err());
}

#[cfg(windows)]
#[test]
fn actual_download_handle_denies_concurrent_mutation_and_delete() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("owned-part");
    safe::write_new(&file, b"existing prefix").unwrap();
    let handle = safe::open_download(&file).unwrap();
    assert!(fs::OpenOptions::new().append(true).open(&file).is_err());
    assert!(fs::rename(&file, file.with_extension("swapped")).is_err());
    assert!(fs::remove_file(&file).is_err());
    drop(handle);
    assert_eq!(fs::read(file).unwrap(), b"existing prefix");
}

#[cfg(windows)]
#[test]
fn download_handle_validates_unicode_and_long_local_paths_without_lossy_conversion() {
    let root = tempfile::tempdir().unwrap();
    for directory in ["owned-繁體 العربية 😀".to_owned(), "segment/".repeat(75)] {
        let file = root.path().join(directory).join("owned-part");
        safe::write_new(&file, b"existing prefix").unwrap();
        let handle = safe::open_download(&file).unwrap();
        assert_eq!(handle.metadata().unwrap().len(), 15);
        assert!(fs::OpenOptions::new().append(true).open(&file).is_err());
        drop(handle);
        assert_eq!(fs::read(file).unwrap(), b"existing prefix");
    }
}

#[test]
fn bundle_interruption_stops_raw_file_fallback_then_retry_resumes_the_archive() {
    use flate2::{write::GzEncoder, Compression};
    let root = tempfile::tempdir().unwrap();
    let targets = [
        ("mir2-assets/a.txt", &b"alpha"[..]),
        ("mir2-assets/b.txt", &b"beta"[..]),
    ];
    let files: Vec<_> = targets
        .iter()
        .map(|(path, bytes)| FileEntry {
            path: (*path).into(),
            size: bytes.len() as u64,
            sha256: hash(bytes),
        })
        .collect();
    let mut raw = delivery::BUNDLE_MAGIC.to_vec();
    raw.extend_from_slice(&(targets.len() as u32).to_le_bytes());
    for (path, bytes) in targets {
        raw.extend_from_slice(&(path.len() as u16).to_le_bytes());
        raw.extend_from_slice(path.as_bytes());
        raw.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        raw.extend_from_slice(bytes);
    }
    let mut zip = GzEncoder::new(Vec::new(), Compression::fast());
    zip.write_all(&raw).unwrap();
    let bytes = zip.finish().unwrap();
    let archive = entry(&bytes);
    let descriptor = delivery::Delivery {
        schema: "mir2.windows.delivery.v1".into(),
        candidate: "test".into(),
        package_manifest_sha256: hash(b"manifest"),
        version_sha256: hash(b"version"),
        built_unix: now(),
        patches: vec![],
        bundles: vec![delivery::Bundle {
            archive: archive.clone(),
            files: files.iter().map(|f| f.path.clone()).collect(),
        }],
    };
    let manifest = Manifest {
        schema: "mir2.windows.package-manifest.v4".into(),
        coverage: Coverage {
            excludes: META.iter().map(|p| p.to_string()).collect(),
            rule: "test".into(),
        },
        file_count: files.len(),
        total_bytes: files.iter().map(|f| f.size).sum(),
        aggregate_sha256: hash(b"unused lower-level fixture"),
        files: files.clone(),
    };
    let component = Component {
        directory: "releases/test".into(),
        identity: "test".into(),
        metadata: vec![],
    };
    let interrupted = Reply::new(
        "200 OK",
        &full_headers(bytes.len()),
        bytes[..bytes.len() / 2].to_vec(),
    );
    let mut first_transfers = Transfers::default();
    let mut first_stats = delivery::Stats::default();
    let result = delivery::Acceleration {
        root: root.path(),
        source: &interrupted.source,
        component: &component,
        status: &Quiet,
        total: manifest.total_bytes,
        transfers: &mut first_transfers,
        stats: &mut first_stats,
    }
    .run(&descriptor, &manifest, &files);
    interrupted.finish();
    assert!(
        result.is_err(),
        "network interruption must stop instead of switching to raw file downloads"
    );
    assert_eq!(first_stats.fallbacks, 0);
    let prefix = safe::read_bounded(&part(root.path(), &archive), archive.size).unwrap();
    assert!(!prefix.is_empty() && prefix.len() < bytes.len());
    for file in &files {
        assert!(!root
            .path()
            .join(format!(".update/downloads/{}", file.sha256))
            .exists());
    }
    let resumed = Reply::new(
        "206 Partial Content",
        &range_headers(prefix.len(), bytes.len()),
        bytes[prefix.len()..].to_vec(),
    );
    let mut transfers = Transfers::default();
    let mut stats = delivery::Stats::default();
    delivery::Acceleration {
        root: root.path(),
        source: &resumed.source,
        component: &component,
        status: &Quiet,
        total: manifest.total_bytes,
        transfers: &mut transfers,
        stats: &mut stats,
    }
    .run(&descriptor, &manifest, &files)
    .unwrap();
    assert!(resumed.finish()[0]
        .to_ascii_lowercase()
        .contains(&format!("range: bytes={}-\r\n", prefix.len())));
    assert_eq!(stats.bundles, 1);
    assert_eq!(stats.fallbacks, 0);
    assert_eq!(transfers.wire_bytes, archive.size - prefix.len() as u64);
    for file in &files {
        assert!(safe::matches(
            &root
                .path()
                .join(format!(".update/downloads/{}", file.sha256)),
            file
        )
        .unwrap());
    }
    assert!(!root.path().join("game").exists());
}

#[test]
fn malformed_raw_headers_status_and_chunk_framing_discard_prefix_without_fallback() {
    let data = b"0123456789abcdef";
    let file = entry(data);
    let valid = b"Content-Range: bytes 4-15/16\r\n";
    let cases = [
        (
            "206 Partial Content",
            b"Content-Encoding: \xff\r\n".to_vec(),
            data[4..].to_vec(),
        ),
        (
            "206 Partial Content",
            b"Content-Encoding: identity\r\nContent-Encoding: \xff\r\n".to_vec(),
            data[4..].to_vec(),
        ),
        (
            "206 Partial Content",
            b"Content-Range: \xff\r\n".to_vec(),
            data[4..].to_vec(),
        ),
        (
            "206 Partial Content",
            b"Content-Length: \xff\r\n".to_vec(),
            data[4..].to_vec(),
        ),
        (
            "206 Partial Content",
            (0..101)
                .map(|i| format!("X-Fixture-{i}: value\r\n"))
                .collect::<String>()
                .into_bytes(),
            data[4..].to_vec(),
        ),
        ("not-a-status", vec![], data[4..].to_vec()),
        (
            "206 Partial Content",
            b"Transfer-Encoding: chunked\r\n".to_vec(),
            b"not-hex\r\ninvalid-frame\r\n0\r\n\r\n".to_vec(),
        ),
    ];
    for (status, extra, body) in cases {
        let root = tempfile::tempdir().unwrap();
        write_prefix(root.path(), &file, &data[..4]);
        let mut headers = valid.to_vec();
        headers.extend_from_slice(&extra);
        let reply = Reply::raw_many(vec![(status.into(), headers, body)]);
        let (result, _) = fetch(root.path(), &reply.source, &file, &Quiet);
        assert!(result.is_err(), "accepted malformed {status}/{extra:?}");
        assert_eq!(reply.finish().len(), 1);
        assert!(
            !part(root.path(), &file).exists(),
            "protocol error retained {status}/{extra:?}"
        );
        assert!(!root
            .path()
            .join(format!(".update/downloads/{}", file.sha256))
            .exists());
    }
}

#[test]
fn temporary_failure_of_both_pinned_origins_retains_offset_for_next_206() {
    let root = tempfile::tempdir().unwrap();
    let data = b"immutable payload with a reusable prefix";
    let file = entry(data);
    write_prefix(root.path(), &file, &data[..8]);
    let preferred = Reply::new("503 Service Unavailable", "Content-Length: 0\r\n", vec![]);
    let legacy = Reply::new("503 Service Unavailable", "Content-Length: 0\r\n", vec![]);
    let source = HttpsSource {
        base: preferred.source.base.clone(),
        legacy_base: legacy.source.base.clone(),
        agent: ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_read(Duration::from_secs(2))
            .build(),
    };
    let (result, transfers) = fetch(root.path(), &source, &file, &Quiet);
    assert!(download::interrupted(&result.unwrap_err()));
    assert_eq!(transfers.wire_bytes, 0);
    assert_eq!(
        safe::read_bounded(&part(root.path(), &file), file.size).unwrap(),
        data[..8]
    );
    for requests in [preferred.finish(), legacy.finish()] {
        assert_eq!(requests.len(), 1);
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("range: bytes=8-\r\n"));
    }
    let next = Reply::new(
        "206 Partial Content",
        &range_headers(8, data.len()),
        data[8..].to_vec(),
    );
    let (result, transfers) = fetch(root.path(), &next.source, &file, &Quiet);
    assert!(safe::matches(&result.unwrap().0, &file).unwrap());
    assert!(next.finish()[0]
        .to_ascii_lowercase()
        .contains("range: bytes=8-\r\n"));
    assert_eq!(transfers.wire_bytes, file.size - 8);
}
