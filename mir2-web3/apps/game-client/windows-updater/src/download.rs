//! Resume only immutable, authenticated payloads at the two compiled origins.
//! Prefixes stay untrusted scratch until the complete signed size/SHA matches.
use super::*;
use std::io::{Seek, SeekFrom};

#[derive(Clone, Copy, Debug)]
pub struct DownloadProgress {
    /// Cumulative new response bytes, excluding all bytes reused from disk.
    pub received: u64,
    /// Existing bytes accepted by this particular 206 response (zero for 200).
    pub reused: u64,
}

#[derive(Debug)]
struct Interrupted;
impl std::fmt::Display for Interrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("payload interrupted; synchronized prefix may be resumed")
    }
}
impl std::error::Error for Interrupted {}
pub(super) fn interrupted(error: &anyhow::Error) -> bool {
    error.is::<Interrupted>()
}

fn header<'a>(response: &'a ureq::Response, name: &str) -> Result<Option<&'a str>> {
    let values = response.all(name);
    let named = response
        .headers_names()
        .iter()
        .filter(|header| header.eq_ignore_ascii_case(name))
        .count();
    ensure!(
        values.len() == named,
        "uninterpretable payload {name} header"
    );
    ensure!(values.len() <= 1, "duplicate payload {name} header");
    Ok(values.first().copied())
}
fn decimal(value: &str) -> Result<u64> {
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "invalid range decimal"
    );
    Ok(value.parse()?)
}
fn content_range(response: &ureq::Response, offset: u64, size: u64) -> Result<()> {
    let value = header(response, "Content-Range")?.context("missing payload Content-Range")?;
    let (range, total) = value
        .strip_prefix("bytes ")
        .context("invalid range unit")?
        .split_once('/')
        .context("invalid payload Content-Range")?;
    let (first, last) = range
        .split_once('-')
        .context("invalid payload Content-Range")?;
    ensure!(
        size > offset
            && decimal(first)? == offset
            && decimal(last)? == size - 1
            && decimal(total)? == size,
        "payload range mismatch"
    );
    Ok(())
}
struct PayloadFailure {
    error: anyhow::Error,
    unavailable: bool,
}
fn protocol(error: anyhow::Error) -> PayloadFailure {
    PayloadFailure {
        error,
        unavailable: false,
    }
}
fn open_at(
    source: &HttpsSource,
    base: &url::Url,
    path: &str,
    offset: u64,
    size: u64,
) -> std::result::Result<(ureq::Response, u64), PayloadFailure> {
    let url = HttpsSource::object_url(base, path).map_err(protocol)?;
    let mut request = source
        .agent
        .get(url.as_str())
        .set("Accept-Encoding", "identity");
    if offset > 0 {
        request = request.set("Range", &format!("bytes={offset}-"));
    }
    let response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::Status(416, response)) if offset > 0 => {
            // A 416 is never completion. Only an exact signed total permits one
            // ordinary GET on the same origin; malformed totals are integrity failures.
            let expected = format!("bytes */{size}");
            let actual = header(&response, "Content-Range").map_err(protocol)?;
            if actual != Some(expected.as_str()) {
                return Err(protocol(anyhow::anyhow!("payload 416 total mismatch")));
            }
            return open_at(source, base, path, 0, size);
        }
        Err(error) => {
            let transport = matches!(
                error.kind(),
                ureq::ErrorKind::Dns
                    | ureq::ErrorKind::ConnectionFailed
                    | ureq::ErrorKind::Io
                    | ureq::ErrorKind::ProxyConnect
            );
            let temporary_status = matches!(
                &error,
                ureq::Error::Status(408 | 429 | 500 | 502 | 503 | 504, _)
            );
            let unavailable = transport
                || matches!(&error, ureq::Error::Status(status, _) if !(300..400).contains(status));
            let error = anyhow::Error::new(error);
            return Err(PayloadFailure {
                error: if transport || temporary_status {
                    error.context(Interrupted)
                } else {
                    error
                },
                unavailable,
            });
        }
    };
    let reused = match response.status() {
        200 => {
            if header(&response, "Content-Range")
                .map_err(protocol)?
                .is_some()
            {
                return Err(protocol(anyhow::anyhow!(
                    "unexpected range on full payload"
                )));
            }
            0 // A server may ignore Range. Reset instead of appending its full body.
        }
        206 if offset > 0 => {
            content_range(&response, offset, size).map_err(protocol)?;
            offset
        }
        206 => return Err(protocol(anyhow::anyhow!("unsolicited partial payload"))),
        status => {
            return Err(PayloadFailure {
                error: anyhow::anyhow!("invalid payload HTTP status {status}"),
                unavailable: !(300..400).contains(&status),
            })
        }
    };
    if header(&response, "Content-Encoding")
        .map_err(protocol)?
        .unwrap_or("identity")
        != "identity"
    {
        return Err(protocol(anyhow::anyhow!("invalid payload encoding")));
    }
    if header(&response, "Content-Type")
        .map_err(protocol)?
        .is_some_and(|v| v.to_ascii_lowercase().starts_with("multipart/"))
    {
        return Err(protocol(anyhow::anyhow!("multipart payload denied")));
    }
    if let Some(length) = header(&response, "Content-Length").map_err(protocol)? {
        if decimal(length).map_err(protocol)? != size.saturating_sub(reused) {
            return Err(protocol(anyhow::anyhow!("payload length mismatch")));
        }
    }
    Ok((response, reused))
}

pub(super) fn resume(
    source: &HttpsSource,
    path: &str,
    entry: &FileEntry,
    destination: &Path,
    progress: &mut dyn FnMut(DownloadProgress) -> Result<()>,
) -> Result<u64> {
    let mut file = safe::open_download(destination)?;
    let offset = file.metadata()?.len();
    ensure!(
        offset == 0 || offset < entry.size,
        "invalid partial payload length"
    );
    let (response, reused) = match open_at(source, &source.base, path, offset, entry.size) {
        Ok(result) => result,
        Err(failure) if failure.unavailable => {
            open_at(source, &source.legacy_base, path, offset, entry.size)
                .map_err(|failure| failure.error)?
        }
        Err(failure) => return Err(failure.error),
    };
    if reused == 0 {
        file.set_len(0)?;
    }
    file.seek(SeekFrom::Start(reused))?;
    let remaining = entry.size - reused;
    let mut reader = response.into_reader().take(remaining.saturating_add(1));
    let mut received = 0u64;
    let mut buffer = [0u8; 65536];
    if let Err(error) = progress(DownloadProgress { received, reused }) {
        file.sync_all()?;
        return Err(error.context(Interrupted));
    }
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(n) => n,
            Err(error) => {
                file.sync_all()?;
                let retain = matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                        | std::io::ErrorKind::BrokenPipe
                );
                let error = anyhow::Error::new(error);
                return Err(if retain {
                    error.context(Interrupted)
                } else {
                    error
                });
            }
        };
        if n == 0 {
            break;
        }
        received += n as u64;
        if received > remaining {
            // Count an oversize body, but never classify its failed bound check
            // as resumable cancellation or leave its old prefix for reuse.
            progress(DownloadProgress { received, reused })?;
            anyhow::bail!("oversized payload");
        }
        // Cancellation precedes mutation, so the next offset is the actual file
        // length, never a UI snapshot or a count of bytes read but not written.
        if let Err(error) = progress(DownloadProgress { received, reused }) {
            file.sync_all()?;
            return Err(error.context(Interrupted));
        }
        file.write_all(&buffer[..n])?;
    }
    file.sync_all()?;
    if received < remaining {
        return Err(anyhow::anyhow!("truncated payload").context(Interrupted));
    }
    drop(file);
    ensure!(safe::matches(destination, entry)?, "corrupt payload SHA");
    Ok(reused)
}
