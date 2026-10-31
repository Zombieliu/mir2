/** Temporary, authenticated import of one exact immutable installer release. */
const ORIGIN_HOST = "165.154.65.136.sslip.io";
const IMPORT_HOST = "mir2-full-installer-import.zombiesliu.workers.dev";
const EXPIRES = Date.parse("2026-11-01T00:00:00Z");

function hex(value) {
  if (!(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) return null;
  const bytes = value instanceof ArrayBuffer ? new Uint8Array(value) : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  return [...bytes].map(byte => byte.toString(16).padStart(2, "0")).join("");
}

function metadataMatches(object, entry, release) {
  return object && object.size === entry.bytes && hex(object.checksums?.sha256) === entry.sha256 &&
    object.customMetadata?.sha256 === entry.sha256 && object.customMetadata?.sourceInstaller === release.sha256 &&
    (!object.httpMetadata?.contentEncoding || object.httpMetadata.contentEncoding === "identity");
}

function json(value, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store", "x-content-type-options": "nosniff" } });
}

async function authorized(request, expected) {
  if (typeof expected !== "string" || expected.length !== 64) return false;
  const supplied = request.headers.get("authorization") ?? "";
  const expectedBytes = new TextEncoder().encode("Bearer " + expected);
  const suppliedBytes = new TextEncoder().encode(supplied);
  if (suppliedBytes.byteLength !== expectedBytes.byteLength) return false;
  return crypto.subtle.timingSafeEqual(expectedBytes, suppliedBytes);
}

async function nativeContracts(bucket, release) {
  const prefix = "mir2/native/windows-full-installers/" + release.sha256 + "/probes/" + crypto.randomUUID() + "/";
  const keys = ["wrong-sha", "short-stream", "race", "existing-wrong"].map(name => prefix + name);
  const good = new TextEncoder().encode("mir2 R2 checksum contract\n");
  const bad = new TextEncoder().encode("mir2 R2 checksum contracX\n");
  const sha = hex(await crypto.subtle.digest("SHA-256", good));
  const badSha = hex(await crypto.subtle.digest("SHA-256", bad));
  const checks = {};
  try {
    let rejected = false;
    try { await bucket.put(keys[0], good, { sha256: "0".repeat(64), onlyIf: new Headers({ "if-none-match": "*" }) }); }
    catch { rejected = true; }
    checks.wrongShaRejectedAtomically = rejected && await bucket.head(keys[0]) === null;
    const fixed = new FixedLengthStream(good.byteLength + 1);
    const writer = fixed.writable.getWriter();
    const write = (async () => { await writer.write(good); await writer.close(); })();
    const put = bucket.put(keys[1], fixed.readable, { sha256: sha, onlyIf: new Headers({ "if-none-match": "*" }) });
    const short = await Promise.allSettled([write, put]);
    checks.shortStreamRejectedAtomically = short.every(result => result.status === "rejected") && await bucket.head(keys[1]) === null;
    const options = { sha256: sha, onlyIf: new Headers({ "if-none-match": "*" }) };
    const race = await Promise.all([bucket.put(keys[2], good, options), bucket.put(keys[2], good, options)]);
    checks.createOnceRaceHasOneWinner = race.filter(Boolean).length === 1 && hex((await bucket.head(keys[2]))?.checksums?.sha256) === sha;
    const expected = { bytes: good.byteLength, sha256: sha };
    await bucket.put(keys[3], bad, { sha256: badSha, onlyIf: new Headers({ "if-none-match": "*" }), customMetadata: { sha256: sha, sourceInstaller: release.sha256 } });
    checks.nativeChecksumRejectsForgedMetadata = !metadataMatches(await bucket.head(keys[3]), expected, release);
    const collision = await bucket.put(keys[3], good, { sha256: sha, onlyIf: new Headers({ "if-none-match": "*" }) });
    checks.existingWrongObjectNotOverwritten = collision === null && hex((await bucket.head(keys[3]))?.checksums?.sha256) === badSha;
    if (!Object.values(checks).every(Boolean)) return json({ passed: false, checks }, 502);
    return json({ passed: true, checks, probeObjectsCleaned: true });
  } finally {
    await bucket.delete(keys);
  }
}

export function createImporter(release, transport = fetch) {
  if (release.schema !== 1 || !/^[a-f0-9]{64}$/.test(release.sha256) || !Number.isSafeInteger(release.bytes) || release.bytes < 1 || release.bytes > 1536 * 1024 * 1024 || release.parts.length !== Math.ceil(release.bytes / (64 * 1024 * 1024)) || release.files.length !== 4) throw new Error("invalid_release");
  const prefix = "mir2/native/windows-full-installers/" + release.sha256 + "/";
  const entries = [...release.parts, ...release.files];
  let offset = 0;
  for (const [index, part] of release.parts.entries()) {
    if (part.offset !== offset || !Number.isSafeInteger(part.bytes) || part.bytes < 1 || part.bytes > 64 * 1024 * 1024 ||
      (index < release.parts.length - 1 && part.bytes !== 64 * 1024 * 1024) || !/^[a-f0-9]{64}$/.test(part.sha256) ||
      part.key !== prefix + "parts/" + part.sha256 + ".bin") throw new Error("invalid_release_part");
    offset += part.bytes;
  }
  if (offset !== release.bytes) throw new Error("invalid_release_size");
  if (release.files.map(file => file.name).sort().join(",") !== ["README.zh-CN.txt", "SHA256SUMS.txt", "release.json", "release.p7s"].sort().join(",")) throw new Error("invalid_release_files");
  for (const file of release.files) {
    if (!Number.isSafeInteger(file.bytes) || file.bytes < 1 || file.bytes > 32768 || !/^[a-f0-9]{64}$/.test(file.sha256) || file.key !== prefix + "files/" + file.name) throw new Error("invalid_release_file");
  }
  return async function handle(request, env) {
    const url = new URL(request.url);
    const probe = url.pathname === "/probe/contracts";
    if (url.protocol !== "https:" || url.host !== IMPORT_HOST || url.search || url.hash || request.method !== "POST" ||
      (!probe && !/^\/import\/(?:0|[1-9][0-9]*)$/.test(url.pathname))) return json({ error: "not_found" }, 404);
    if (!await authorized(request, env.IMPORT_TOKEN)) return json({ error: "unauthorized" }, 401);
    if (Date.now() >= EXPIRES) return json({ error: "import_expired" }, 410);
    if ((request.headers.get("content-length") ?? "0") !== "0" || request.body !== null) return json({ error: "body_not_allowed" }, 400);
    if (probe) {
      try { return await nativeContracts(env.MIR2_ASSETS, release); }
      catch { return json({ error: "native_probe_failed" }, 502); }
    }
    const index = Number(url.pathname.slice("/import/".length));
    const entry = entries[index];
    if (!entry) return json({ error: "not_found" }, 404);
    let originBody = null;
    try {
      const existing = await env.MIR2_ASSETS.head(entry.key);
      if (existing) {
        if (!metadataMatches(existing, entry, release)) return json({ error: "immutable_collision" }, 409);
        return json({ passed: true, index, bytes: entry.bytes, sha256: entry.sha256, alreadyPresent: true });
      }
      const name = index < release.parts.length ? "Mir2Setup.exe" : entry.name;
      const source = "https://" + ORIGIN_HOST + "/client-updates/installers/" + release.sha256 + "/" + name;
      const headers = new Headers({ "accept-encoding": "identity", "cache-control": "no-cache" });
      if (index < release.parts.length) headers.set("range", "bytes=" + entry.offset + "-" + (entry.offset + entry.bytes - 1));
      const response = await transport(source, { method: "GET", headers, redirect: "error", signal: AbortSignal.timeout(120000) });
      originBody = response.body;
      const encoding = response.headers.get("content-encoding");
      const contentRange = index < release.parts.length ? "bytes " + entry.offset + "-" + (entry.offset + entry.bytes - 1) + "/" + release.bytes : null;
      if (response.status !== (index < release.parts.length ? 206 : 200) || response.headers.get("content-length") !== String(entry.bytes) ||
        response.headers.get("content-range") !== contentRange || (encoding && encoding !== "identity") || !response.body ||
        (response.url && response.url !== source)) {
        if (response.body) await response.body.cancel().catch(() => {});
        return json({ error: "origin_contract_mismatch" }, 502);
      }
      // R2 checks this native SHA256 before committing. Preserve fetch's known-
      // length stream; buffering or transforming would lose its length metadata.
      const stored = await env.MIR2_ASSETS.put(entry.key, response.body, {
        onlyIf: new Headers({ "if-none-match": "*" }),
        sha256: entry.sha256,
        httpMetadata: { contentType: entry.contentType ?? "application/octet-stream", cacheControl: "public, max-age=31536000, immutable, no-transform" },
        customMetadata: { sha256: entry.sha256, sourceInstaller: release.sha256 },
      });
      if (!stored) {
        const winner = await env.MIR2_ASSETS.head(entry.key);
        if (!metadataMatches(winner, entry, release)) return json({ error: "immutable_collision" }, 409);
      } else if (!metadataMatches(stored, entry, release)) {
        return json({ error: "storage_contract_mismatch" }, 502);
      }
      return json({ passed: true, index, bytes: entry.bytes, sha256: entry.sha256, alreadyPresent: !stored });
    } catch {
      return json({ error: "import_failed", index }, 502);
    } finally {
      if (originBody) await originBody.cancel().catch(() => {});
    }
  };
}
