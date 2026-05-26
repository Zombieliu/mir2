export interface Env {
  MIR2_ASSETS: R2Bucket;
  MIR2_R2_UPLOAD_SECRET: string;
}

const MAX_OBJECT_BYTES = 300 * 1024 * 1024;

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method !== "PUT") {
      return json({ ok: false, error: "method_not_allowed" }, 405);
    }

    const auth = request.headers.get("authorization") ?? "";
    if (!env.MIR2_R2_UPLOAD_SECRET || auth !== `Bearer ${env.MIR2_R2_UPLOAD_SECRET}`) {
      return json({ ok: false, error: "unauthorized" }, 401);
    }

    const url = new URL(request.url);
    if (url.pathname !== "/upload") {
      return json({ ok: false, error: "not_found" }, 404);
    }

    const key = normalizeObjectKey(url.searchParams.get("key") ?? "");
    if (!key) {
      return json({ ok: false, error: "invalid_key" }, 400);
    }

    const contentLength = Number(request.headers.get("content-length") ?? 0);
    if (Number.isFinite(contentLength) && contentLength > MAX_OBJECT_BYTES) {
      return json({ ok: false, error: "object_too_large" }, 413);
    }

    await env.MIR2_ASSETS.put(key, request.body, {
      httpMetadata: {
        contentType: request.headers.get("content-type") ?? undefined,
        cacheControl: request.headers.get("cache-control") ?? undefined,
      },
    });

    return json({ ok: true, key });
  },
};

function normalizeObjectKey(value: string): string {
  const key = value.trim().replace(/^\/+/, "");
  if (!key || key.includes("..") || key.includes("\\")) return "";
  if (!key.startsWith("mir2/")) return "";
  return key;
}

function json(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}
