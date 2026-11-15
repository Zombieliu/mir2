/** Read immutable CacheStorage bodies without fetching, repairing or seeding. */
export async function auditStaticCacheBodies(page, expected, { timeoutMs = 8000, cacheSchema = "sw8" } = {}) {
  if (!Array.isArray(expected) || expected.length < 1 || expected.length > 128
    || new Set(expected.map(row => row.path)).size !== expected.length
    || expected.some(row => !/^\/original-ui\/(?:fonts\/[A-Za-z0-9_.-]+\.(?:otf|ttf)|[A-Za-z0-9_]+\/\d+\.png)$/.test(row.path)
      || !Number.isSafeInteger(row.bytes) || row.bytes < 1 || row.bytes > 32 * 1024 * 1024
      || !/^[a-f0-9]{64}$/.test(row.sha256))
    || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 10000
    || !/^sw[1-9]\d*$/.test(cacheSchema)) throw Error("Invalid static cache audit pins");
  let timer;
  try {
    return await Promise.race([
      page.evaluate(async ({ pins, cacheSchema }) => {
        const namespace = window.__mir2AssetCache?.cacheNamespace;
        if (typeof namespace !== "string" || !namespace.length) return { observed: false, reason: "cache-namespace-unavailable" };
        const names = (await caches.keys()).filter(name => name.startsWith("mir2-asset-cache-" + cacheSchema + "-static-") && name.endsWith("-" + namespace));
        if (!names.length) return { observed: false, reason: "current-static-cache-unavailable", namespace, cacheSchema, cacheCount: 0 };
        const stores = await Promise.all(names.map(name => caches.open(name)));
        const rows = [];
        for (const pin of pins) {
          let response;
          for (const store of stores) { response = await store.match(new URL(pin.path, location.origin).href); if (response) break; }
          if (!response) { rows.push({ path: pin.path, present: false, matches: false }); continue; }
          const chunks = [], reader = response.body?.getReader(); let bytes = 0, oversized = false;
          try {
            while (reader) { const chunk = await reader.read(); if (chunk.done) break;
              bytes += chunk.value.byteLength;
              if (bytes > pin.bytes) { oversized = true; await reader.cancel(); break; }
              chunks.push(chunk.value);
            }
          } finally { reader?.releaseLock(); }
          let sha256 = null;
          if (!oversized) {
            const body = new Uint8Array(bytes); let offset = 0;
            for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
            sha256 = [...new Uint8Array(await crypto.subtle.digest("SHA-256", body))].map(value => value.toString(16).padStart(2, "0")).join("");
          }
          rows.push({ path: pin.path, present: true, status: response.status, bytes, sha256,
            matches: response.status === 200 && bytes === pin.bytes && sha256 === pin.sha256 });
        }
        return { observed: true, namespace, cacheCount: names.length, expectedPaths: pins.length,
          matched: rows.filter(row => row.matches).length, missing: rows.filter(row => !row.present).length,
          mismatched: rows.filter(row => row.present && !row.matches).length, rows };
      }, { pins: expected, cacheSchema }),
      new Promise(resolve => { timer = setTimeout(() => resolve({ observed: false, reason: "audit-timeout" }), timeoutMs); }),
    ]);
  } catch (error) {
    return { observed: false, reason: "audit-unavailable", errorName: error.name };
  } finally { clearTimeout(timer); }
}
