export const KEY = "mir2/downloads/android/playtest/v40-20261107-cbee2d6a316e/Mir2-Android-Playtest-20261107-arm64.apk";
export const SHA256 = "cbee2d6a316e37043395517c1c6ad082abad41f8e74bf399743aab476ac2417e";
export const SIZE = 412322437;

const reply = (value, status = 200) => Response.json(value, { status, headers: { "cache-control": "no-store" } });
const hex = (value) => value ? Array.from(new Uint8Array(value), (v) => v.toString(16).padStart(2, "0")).join("") : "";

export default {
  async fetch(request, env) {
    if (!env.UPLOAD_SECRET || request.headers.get("authorization") !== `Bearer ${env.UPLOAD_SECRET}`) return reply({ error: "unauthorized" }, 401);
    if (Date.now() > Number(env.EXPIRES_AT)) return reply({ error: "expired" }, 403);
    if (request.method !== "POST" || new URL(request.url).pathname !== "/publish") return reply({ error: "not_found" }, 404);
    if (Number(request.headers.get("content-length")) > 16384) return reply({ error: "invalid_request" }, 400);
    try {
      const { sourceUrl } = await request.json();
      const source = new URL(sourceUrl);
      if (source.protocol !== "https:" || source.hostname !== "release-assets.githubusercontent.com" || source.username || source.password || source.port) return reply({ error: "invalid_source" }, 400);
      const existing = await env.APK.head(KEY);
      if (existing) {
        if (existing.size !== SIZE || hex(existing.checksums?.sha256) !== SHA256) return reply({ error: "existing_object_mismatch" }, 409);
        return reply({ ok: true, existing: true, key: KEY, size: SIZE, sha256: SHA256 });
      }
      const download = await fetch(source, { redirect: "error", headers: { "accept-encoding": "identity" } });
      if (download.status !== 200 || Number(download.headers.get("content-length")) !== SIZE || download.headers.has("content-encoding")) return reply({ error: "source_metadata_mismatch" }, 502);
      const object = await env.APK.put(KEY, download.body, {
        sha256: SHA256,
        onlyIf: new Headers({ "If-None-Match": "*" }),
        httpMetadata: {
          contentType: "application/vnd.android.package-archive",
          contentDisposition: 'attachment; filename="Mir2-Android-Playtest-20261107-arm64.apk"',
          cacheControl: "public, max-age=31536000, immutable",
        },
        customMetadata: { sha256: SHA256, versionCode: "40", stage: "basic-playtest-not-three-class-level30-accepted" },
      });
      if (!object) return reply({ error: "object_already_exists" }, 409);
      if (object.size !== SIZE || hex(object.checksums?.sha256) !== SHA256) return reply({ error: "stored_checksum_mismatch" }, 502);
      return reply({ ok: true, existing: false, key: KEY, size: object.size, sha256: SHA256 });
    } catch {
      return reply({ error: "transfer_failed" }, 502);
    }
  },
};
