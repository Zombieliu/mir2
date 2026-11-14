// A read-only, release-scoped facade over independently authenticated R2 parts.
const PART_LIMIT = 64 * 1024 * 1024;
const SMALL_LIMIT = 32 * 1024;
const SHA256 = /^[0-9a-f]{64}$/;
const FILE_NAMES = ["release.json", "release.p7s", "SHA256SUMS.txt", "README.zh-CN.txt"];
const IMMUTABLE = "public, max-age=31536000, immutable, no-transform";
const CACHE_MARKER = "mir2-full-installer-v1";
const CACHE_QUEUE_LIMIT = 1024 * 1024;

function invariant(condition, message) {
  if (!condition) throw new TypeError(message);
}

function validKey(key) {
  return typeof key === "string" && /^[a-zA-Z0-9][a-zA-Z0-9._/-]{0,1023}$/.test(key)
    && !key.split("/").some((segment) => !segment || segment === "." || segment === "..");
}

function prepareRelease(input) {
  invariant(input?.schema === 1 && SHA256.test(input.sha256), "Invalid installer release identity");
  invariant(input.sourceInstaller === input.sha256, "Invalid installer source binding");
  invariant(Number.isSafeInteger(input.bytes) && input.bytes > 0, "Invalid installer size");
  const publicBase = new URL(input.publicBase);
  invariant(input.publicBase === publicBase.href
    && publicBase.origin === "https://assets.mir2.obelisk.build"
    && publicBase.pathname === `/client-updates/installers/${input.sha256}/`
    && !publicBase.search && !publicBase.hash, "Invalid installer public base");
  invariant(Array.isArray(input.allowedHosts) && input.allowedHosts.length > 0
    && input.allowedHosts.every((host) => typeof host === "string"
      && /^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/.test(host)), "Invalid installer hosts");
  const hosts = new Set(input.allowedHosts);
  invariant(hosts.has(publicBase.host), "Public base host must be allowed");
  const modified = new Date(input.lastModified);
  invariant(Number.isFinite(modified.getTime()), "Invalid installer modification time");
  invariant(Array.isArray(input.parts) && input.parts.length > 0 && input.parts.length <= 64,
    "Invalid installer part count");
  let offset = 0;
  const keys = new Set();
  const parts = input.parts.map((part) => {
    invariant(part.offset === offset && Number.isSafeInteger(part.bytes)
      && part.bytes > 0 && part.bytes <= PART_LIMIT && SHA256.test(part.sha256)
      && validKey(part.key) && !keys.has(part.key), "Invalid installer part layout");
    keys.add(part.key);
    offset += part.bytes;
    invariant(Number.isSafeInteger(offset) && offset <= input.bytes, "Installer part overflow");
    return Object.freeze({ offset: part.offset, bytes: part.bytes, sha256: part.sha256, key: part.key });
  });
  invariant(offset === input.bytes, "Installer parts do not cover the complete file");
  invariant(Array.isArray(input.files) && input.files.length === FILE_NAMES.length,
    "Invalid installer companion count");
  const files = new Map();
  for (const file of input.files) {
    invariant(FILE_NAMES.includes(file.name) && !files.has(file.name)
      && Number.isSafeInteger(file.bytes) && file.bytes > 0 && file.bytes <= SMALL_LIMIT
      && SHA256.test(file.sha256) && validKey(file.key) && !keys.has(file.key)
      && typeof file.contentType === "string"
      && /^[a-zA-Z0-9.+-]+\/[a-zA-Z0-9.+-]+(?:; charset=utf-8)?$/.test(file.contentType),
    "Invalid installer companion");
    keys.add(file.key);
    files.set(file.name, Object.freeze({ ...file }));
  }
  return Object.freeze({
    sha256: input.sha256, bytes: input.bytes, sourceInstaller: input.sha256,
    publicBase: publicBase.href, pathname: publicBase.pathname,
    lastModified: modified.toUTCString(), hosts, parts: Object.freeze(parts), files,
  });
}

function corsHeaders() {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, HEAD",
    "Access-Control-Expose-Headers": "Content-Length, Content-Range, ETag, Last-Modified, Accept-Ranges, X-Mir2-First-Chunk-Cache",
    "X-Content-Type-Options": "nosniff",
  };
}

function unavailable(method, status = 503, extra = {}) {
  const body = status === 404 ? "Not found.\n" : status === 405 ? "Method not allowed.\n" : "Download unavailable.\n";
  return new Response(method === "HEAD" ? null : body, {
    status,
    headers: { ...corsHeaders(), "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store", ...extra },
  });
}

function publicFile(request, release) {
  const raw = request.url;
  if (typeof raw !== "string" || /[%?#\\\u0000-\u0020\u007f]/.test(raw)) return null;
  let url;
  try { url = new URL(raw); } catch { return null; }
  if (url.protocol !== "https:" || !release.hosts.has(url.host)
    || url.username || url.password || url.port || url.search || url.hash
    || raw !== `${url.origin}${url.pathname}` || !url.pathname.startsWith(release.pathname)) return null;
  const name = url.pathname.slice(release.pathname.length);
  if (name === "Mir2Setup.exe") {
    return { name, bytes: release.bytes, sha256: release.sha256, contentType: "application/octet-stream" };
  }
  return release.files.get(name) ?? null;
}

function responseHeaders(file, release) {
  return new Headers({
    ...corsHeaders(), "Content-Type": file.contentType, "Content-Length": String(file.bytes),
    "Content-Disposition": `attachment; filename="${file.name}"`,
    "Accept-Ranges": "bytes", ETag: `"${file.sha256}"`,
    "Last-Modified": release.lastModified, "Cache-Control": IMMUTABLE,
  });
}

function notModified(value, etag) {
  return value !== null && value.split(",").some((tag) => {
    tag = tag.trim();
    return tag === "*" || tag === etag || tag === `W/${etag}`;
  });
}

function byteRange(value, size) {
  const match = typeof value === "string" && value.length <= 200 && /^bytes=(\d*)-(\d*)$/i.exec(value.trim());
  if (!match || (!match[1] && !match[2])) return null;
  const total = BigInt(size);
  if (!match[1]) {
    const suffix = BigInt(match[2]);
    if (suffix === 0n) return null;
    return { start: Number(suffix < total ? total - suffix : 0n), end: size - 1 };
  }
  const start = BigInt(match[1]);
  const end = match[2] ? BigInt(match[2]) : total - 1n;
  if (start >= total || end < start) return null;
  return { start: Number(start), end: Number(end < total ? end : total - 1n) };
}

function checksumHex(value) {
  let bytes;
  if (value instanceof ArrayBuffer) bytes = new Uint8Array(value);
  else if (ArrayBuffer.isView(value)) bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  else return null;
  return bytes.byteLength === 32 ? Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("") : null;
}

function identityEncoding(value) {
  return value === undefined || value === null || (typeof value === "string" && value.trim().toLowerCase() === "identity");
}

function exactRange(range, span) {
  if (!range || typeof range !== "object" || Array.isArray(range)) return false;
  const keys = Reflect.ownKeys(range);
  return (keys.length === 2 || (keys.length === 3 && keys.includes("suffix") && range.suffix === undefined))
    && keys.includes("offset") && keys.includes("length")
    && range.offset === span.offset && range.length === span.length;
}

function verifyObject(object, expected, release, span = null) {
  // R2's stored native checksum authenticates the WHOLE object, including when
  // get(range) restricts its body. It is not a checksum of the returned slice.
  invariant(object && object.key === expected.key && object.size === expected.bytes
    && (span ? exactRange(object.range, span) : !object.range)
    && typeof object.version === "string" && object.version.length > 0
    && typeof object.httpEtag === "string" && /^"[^"\u0000-\u0020\u007f]+"$/.test(object.httpEtag)
    && object.customMetadata?.sha256 === expected.sha256
    && object.customMetadata?.sourceInstaller === release.sha256
    && checksumHex(object.checksums?.sha256) === expected.sha256
    && identityEncoding(object.httpMetadata?.contentEncoding)
    && typeof object.body?.getReader === "function", "Untrusted installer object");
}

function cacheHeaders(expected, release) {
  return new Headers({
    "Content-Type": "application/octet-stream", "Content-Length": String(expected.bytes),
    "Cache-Control": IMMUTABLE, ETag: `"${expected.sha256}"`,
    "x-mir2-download-cache": CACHE_MARKER, "x-mir2-sha256": expected.sha256,
    "x-mir2-source-installer": release.sha256,
  });
}

function verifyCached(response, expected, release, span = null) {
  invariant(response.status === (span ? 206 : 200) && response.body
    && response.headers.get("Content-Length") === String(span ? span.length : expected.bytes)
    && response.headers.get("Content-Type") === "application/octet-stream"
    && response.headers.get("ETag") === `"${expected.sha256}"`
    && response.headers.get("x-mir2-download-cache") === CACHE_MARKER
    && response.headers.get("x-mir2-sha256") === expected.sha256
    && response.headers.get("x-mir2-source-installer") === release.sha256
    && identityEncoding(response.headers.get("Content-Encoding"))
    && response.headers.get("Content-Range") === (span
      ? `bytes ${span.offset}-${span.offset + span.length - 1}/${expected.bytes}` : null)
    && !response.headers.has("Set-Cookie"),
  "Untrusted installer cache entry");
}

async function discard(body, reason) {
  try { await body?.cancel(reason); } catch { /* Cancellation must not mask the original failure. */ }
}

async function stopPart(part, reason) {
  if (!part) return;
  if (!part.stopping) {
    dropCache(part.fill, reason);
    part.stopping = Promise.resolve().then(() => part.reader.cancel(reason)).catch(() => {}).then(() => {
      try { part.reader.releaseLock(); } catch { /* An in-flight pull releases it after cancellation. */ }
    });
  }
  await part.stopping;
}

function dropCache(fill, reason) {
  if (!fill || !fill.active) return;
  fill.active = false;
  // Do not accumulate another cache pipeline if a failed cache consumer leaves
  // its fixed-length stream locked. One abandoned bounded fill is enough.
  fill.operation.cacheDisabled = true;
  try { fill.controller.error(reason); } catch { /* The cache may already be closed. */ }
}

function lengthBridge(length) {
  // Workers ignores a manually supplied Content-Length for an ordinary stream.
  // Keep the fixed-length native source both for clients and for Cache API puts;
  // the standards-only fallback makes the same reader directly testable in Node.
  return typeof globalThis.FixedLengthStream === "function"
    ? new globalThis.FixedLengthStream(length) : new TransformStream();
}

function knownLengthBody(body, length, ctx) {
  if (typeof globalThis.FixedLengthStream !== "function") return body;
  const bridge = lengthBridge(length);
  const pumping = body.pipeTo(bridge.writable);
  ctx.waitUntil(pumping.catch(() => {}));
  return bridge.readable;
}

function cacheFill(key, expected, release, ctx, cache, operation) {
  // At most one bounded fill per request. A slow cache never backpressures the
  // authenticated source/client or accumulates a queue for each later part.
  if (operation.cacheFill || operation.cacheDisabled) return null;
  const fill = { active: true, controller: null, operation };
  operation.cacheFill = fill;
  const input = new ReadableStream({
    start(controller) { fill.controller = controller; },
    cancel() { fill.active = false; },
  }, { highWaterMark: CACHE_QUEUE_LIMIT, size(chunk) { return chunk.byteLength; } });
  const bridge = lengthBridge(expected.bytes);
  const pumping = input.pipeTo(bridge.writable);
  const putting = Promise.resolve().then(() => cache.put(key,
    new Response(bridge.readable, { headers: cacheHeaders(expected, release) })));
  // Both promises have rejection handlers through Promise.all. A cache that
  // rejects while leaving its response reader locked cannot delay this task or
  // create further fills: dropCache disables caching for the rest of the request.
  fill.task = Promise.all([pumping, putting]).catch((error) => {
    dropCache(fill, error);
  }).finally(() => {
    fill.active = false;
    if (operation.cacheFill === fill) operation.cacheFill = null;
  });
  try { ctx.waitUntil(fill.task); }
  catch (error) { dropCache(fill, error); }
  return fill;
}

function copyToCache(fill, bytes) {
  if (!fill?.active) return;
  if (fill.controller.desiredSize < bytes.byteLength || bytes.byteLength > CACHE_QUEUE_LIMIT) {
    dropCache(fill, new Error("Installer cache exceeded its bounded queue"));
    return;
  }
  // The client may receive a view of the source chunk. The cache owns a bounded
  // independent copy, rather than sharing bytes with a downstream consumer.
  fill.controller.enqueue(bytes.slice());
}

async function openPart(expected, span, release, env, ctx, cache, operation) {
  const partial = span.offset !== 0 || span.length !== expected.bytes;
  const key = new Request(`${release.publicBase}__chunks/${expected.sha256}.bin`, {
    method: "GET", headers: partial ? { Range: `bytes=${span.offset}-${span.offset + span.length - 1}` } : {},
  });
  const cached = await cache.match(key);
  if (operation.cancelled) throw new Error("Installer download cancelled");
  if (cached) {
    try { verifyCached(cached, expected, release, partial ? span : null); }
    catch (error) { await discard(cached.body, error); throw error; }
    return { expected, length: span.length, hit: true, reader: cached.body.getReader(), fill: null, seen: 0 };
  }
  // Never fetch/drain the other 56 MiB for an 8 MiB range. The same native R2
  // whole-object SHA/source/size identity remains mandatory on this ranged GET.
  const object = await env.MIR2_ASSETS.get(expected.key, partial ? { range: span } : undefined);
  try { verifyObject(object, expected, release, partial ? span : null); }
  catch (error) { await discard(object?.body, error); throw error; }
  return { expected, length: span.length, hit: false, reader: object.body.getReader(), seen: 0,
    fill: partial ? null : cacheFill(key, expected, release, ctx, cache, operation) };
}

async function installerBody(request, range, release, env, ctx, cache) {
  const first = release.parts.findIndex((part) => part.offset + part.bytes > range.start);
  const last = release.parts.findIndex((part) => part.offset + part.bytes > range.end);
  const operation = { cancelled: false, closed: false, current: null, controller: null, cacheFill: null };
  const open = (part) => {
    const start = Math.max(range.start, part.offset);
    const end = Math.min(range.end + 1, part.offset + part.bytes);
    return openPart(part, { offset: start - part.offset, length: end - start }, release, env, ctx, cache, operation);
  };
  operation.current = await open(release.parts[first]);
  if (request.signal?.aborted) {
    await stopPart(operation.current, "Cancelled");
    throw new Error("Installer download cancelled");
  }
  const firstHit = operation.current.hit;
  let index = first;
  let sent = 0;
  const wanted = range.end - range.start + 1;
  const detach = () => request.signal?.removeEventListener("abort", abort);
  const abort = () => {
    operation.cancelled = true;
    dropCache(operation.cacheFill, request.signal.reason);
    operation.controller?.error(new Error("Installer download cancelled"));
    ctx.waitUntil(stopPart(operation.current, request.signal.reason));
    detach();
  };
  request.signal?.addEventListener("abort", abort, { once: true });
  const body = new ReadableStream({
    start(controller) { operation.controller = controller; },
    async pull(controller) {
      try {
        while (!operation.cancelled && !operation.closed) {
          if (!operation.current) {
            if (index > last) {
              invariant(sent === wanted, "Installer response length mismatch");
              operation.closed = true;
              detach();
              controller.close();
              return;
            }
            operation.current = await open(release.parts[index]);
            if (operation.cancelled) { await stopPart(operation.current, "Cancelled"); return; }
          }
          const part = operation.current;
          const result = await part.reader.read();
          if (operation.cancelled) return;
          if (result.done) {
            invariant(part.seen === part.length, "Installer chunk length mismatch");
            if (part.fill?.active) part.fill.controller.close();
            part.reader.releaseLock();
            operation.current = null;
            index += 1;
            continue;
          }
          invariant(result.value instanceof Uint8Array, "Installer chunk is not bytes");
          part.seen += result.value.byteLength;
          invariant(part.seen <= part.length, "Installer chunk exceeds its size");
          if (part.seen === part.length) {
            // A ranged R2/cache stream must end at its exact bound. This checks
            // only EOF, never drains an unrequested remainder of a whole part.
            const end = await part.reader.read();
            if (operation.cancelled) return;
            invariant(end.done, "Installer chunk exceeds its size");
            if (part.fill?.active) {
              copyToCache(part.fill, result.value);
              if (part.fill.active) part.fill.controller.close();
            }
            part.reader.releaseLock();
            operation.current = null;
            index += 1;
          } else {
            copyToCache(part.fill, result.value);
          }
          sent += result.value.byteLength;
          controller.enqueue(result.value);
          if (sent === wanted) {
            operation.closed = true;
            detach();
            controller.close();
          }
          return;
        }
      } catch (error) {
        operation.closed = true;
        dropCache(operation.cacheFill, error);
        detach();
        await stopPart(operation.current, error);
        controller.error(error);
      }
    },
    async cancel(reason) {
      operation.cancelled = true;
      dropCache(operation.cacheFill, reason);
      detach();
      await stopPart(operation.current, reason);
    },
  }, { highWaterMark: 0 });
  return { body, firstHit };
}

async function readSmall(body, expectedBytes) {
  const reader = body.getReader();
  const output = new Uint8Array(expectedBytes);
  let size = 0;
  try {
    while (true) {
      const result = await reader.read();
      if (result.done) break;
      invariant(result.value instanceof Uint8Array && size + result.value.byteLength <= expectedBytes,
        "Installer companion exceeds its size");
      output.set(result.value, size);
      size += result.value.byteLength;
    }
    invariant(size === expectedBytes, "Installer companion length mismatch");
    return output;
  } catch (error) {
    try { await reader.cancel(error); } catch { /* Preserve the integrity failure. */ }
    throw error;
  } finally { reader.releaseLock(); }
}

async function companion(file, release, env, ctx, cache) {
  const key = new Request(`${release.publicBase}__files/${file.sha256}/${file.name}`, { method: "GET" });
  const cached = await cache.match(key);
  let bytes;
  if (cached) {
    try { verifyCached(cached, file, release); }
    catch (error) { await discard(cached.body, error); throw error; }
    bytes = await readSmall(cached.body, file.bytes);
  } else {
    const object = await env.MIR2_ASSETS.get(file.key);
    try { verifyObject(object, file, release); }
    catch (error) { await discard(object?.body, error); throw error; }
    bytes = await readSmall(object.body, file.bytes);
  }
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  invariant(checksumHex(digest) === file.sha256, "Installer companion hash mismatch");
  if (!cached) {
    const putting = Promise.resolve().then(() => cache.put(key,
      new Response(bytes, { headers: cacheHeaders(file, release) })));
    ctx.waitUntil(putting.catch(() => {}));
  }
  return bytes;
}

export function createDownloadHandler(release) {
  const manifest = prepareRelease(release);
  return async function handleDownload(request, env, ctx, cache = globalThis.caches?.default) {
    const file = publicFile(request, manifest);
    if (!file) return unavailable(request.method, 404);
    if (request.method !== "GET" && request.method !== "HEAD") {
      return unavailable(request.method, 405, { Allow: "GET, HEAD" });
    }
    const headers = responseHeaders(file, manifest);
    if (notModified(request.headers.get("If-None-Match"), headers.get("ETag"))) {
      return new Response(null, { status: 304, headers });
    }
    if (request.method === "HEAD") return new Response(null, { headers });
    let range = { start: 0, end: file.bytes - 1 };
    let status = 200;
    const rangeHeader = request.headers.get("Range");
    const ifRange = request.headers.get("If-Range");
    if (rangeHeader !== null && (ifRange === null || ifRange === headers.get("ETag")
      || ifRange === manifest.lastModified)) {
      range = byteRange(rangeHeader, file.bytes);
      if (!range) return unavailable(request.method, 416, { "Content-Range": `bytes */${file.bytes}` });
      status = 206;
      headers.set("Content-Range", `bytes ${range.start}-${range.end}/${file.bytes}`);
      headers.set("Content-Length", String(range.end - range.start + 1));
    }
    try {
      invariant(env?.MIR2_ASSETS && typeof env.MIR2_ASSETS.get === "function"
        && cache && typeof cache.match === "function" && typeof cache.put === "function"
        && ctx && typeof ctx.waitUntil === "function", "Installer bindings unavailable");
      if (file.name === "Mir2Setup.exe") {
        const { body, firstHit } = await installerBody(request, range, manifest, env, ctx, cache);
        headers.set("x-mir2-first-chunk-cache", firstHit ? "HIT" : "MISS");
        return new Response(knownLengthBody(body, range.end - range.start + 1, ctx), { status, headers });
      }
      const bytes = await companion(file, manifest, env, ctx, cache);
      return new Response(bytes.subarray(range.start, range.end + 1), { status, headers });
    } catch { return unavailable(request.method); }
  };
}
