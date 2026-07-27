const ORIGIN_HOST = "mir2-telemetry.vercel.app";
const PUBLIC_HOST = "telemetry.obelisk.build";

export default {
  async fetch(request) {
    const origin = new URL(request.url);
    origin.protocol = "https:";
    origin.hostname = ORIGIN_HOST;
    origin.port = "";

    const headers = new Headers(request.headers);
    headers.delete("host");
    headers.delete("x-forwarded-host");
    headers.set("x-forwarded-proto", "https");
    if (headers.has("origin")) {
      headers.set("origin", `https://${ORIGIN_HOST}`);
    }
    if (headers.has("referer")) {
      headers.set(
        "referer",
        headers.get("referer").replace(`https://${PUBLIC_HOST}`, `https://${ORIGIN_HOST}`)
      );
    }

    const upstream = await fetch(
      new Request(origin.toString(), {
        method: request.method,
        headers,
        body: request.body,
        redirect: "manual"
      })
    );
    const responseHeaders = new Headers(upstream.headers);
    const location = responseHeaders.get("location");
    if (location) {
      responseHeaders.set(
        "location",
        location.replace(`https://${ORIGIN_HOST}`, `https://${PUBLIC_HOST}`)
      );
    }
    responseHeaders.set("x-frame-options", "DENY");
    responseHeaders.set("content-security-policy", "frame-ancestors 'none'");
    responseHeaders.set("referrer-policy", "same-origin");
    responseHeaders.set("cache-control", "private, no-store, max-age=0");
    return new Response(upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: responseHeaders
    });
  }
};
