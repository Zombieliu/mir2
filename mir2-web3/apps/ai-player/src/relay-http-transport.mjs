import tls from "node:tls";
import { EnvHttpProxyAgent, Request as UndiciRequest, fetch as undiciFetch } from "undici";

const ORIGIN = "https://dubhe-connect-relay.zombiesliu.workers.dev";
const HOST = "dubhe-connect-relay.zombiesliu.workers.dev";
const MESSAGE = "The relay HTTPS transport is unavailable.";

function failure(reason) {
  const error = new Error(MESSAGE);
  error.code = "RELAY_TRANSPORT_UNAVAILABLE";
  error.reason = reason;
  return error;
}

function variable(environment, lower, upper) {
  const value = environment[lower] ?? environment[upper];
  if (value !== undefined && typeof value !== "string") throw failure("configuration");
  return value;
}

function proxy(value) {
  if (!value.trim()) return "";
  if (value.length > 4096 || /[\s\p{Cc}\p{Cf}]/u.test(value.trim())) throw failure("configuration");
  const url = new URL(value.trim());
  if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.pathname !== "/" || url.search || url.hash) throw failure("configuration");
  return url.href;
}

function proxyOptions(environment) {
  const http = variable(environment, "http_proxy", "HTTP_PROXY");
  const https = variable(environment, "https_proxy", "HTTPS_PROXY");
  const all = variable(environment, "all_proxy", "ALL_PROXY");
  const noProxy = variable(environment, "no_proxy", "NO_PROXY") ?? "";
  if (noProxy.length > 8192 || /[\p{Cc}\p{Cf}]/u.test(noProxy)) throw failure("configuration");
  // Empty values are intentional overrides. Never let the lazy agent read a
  // later process.env value or acquire a different proxy/no-proxy policy.
  return Object.freeze({ httpProxy: proxy(http ?? all ?? ""), httpsProxy: proxy(https ?? http ?? all ?? ""), noProxy });
}

function reasonOf(error) {
  // Inspect only allowlisted categories, never retain the exception or cause.
  try {
    const codes = [error?.code, error?.cause?.code];
    if (codes.some(code => ["UNABLE_TO_GET_ISSUER_CERT_LOCALLY", "UNABLE_TO_VERIFY_LEAF_SIGNATURE", "SELF_SIGNED_CERT_IN_CHAIN", "DEPTH_ZERO_SELF_SIGNED_CERT", "CERT_HAS_EXPIRED", "ERR_TLS_CERT_ALTNAME_INVALID"].includes(code))) return "tls-trust";
    if (codes.some(code => ["ETIMEDOUT", "UND_ERR_CONNECT_TIMEOUT", "UND_ERR_HEADERS_TIMEOUT", "UND_ERR_BODY_TIMEOUT"].includes(code))) return "timeout";
    return error?.name === "AbortError" || codes.some(code => ["ABORT_ERR", "UND_ERR_ABORTED"].includes(code)) ? "aborted" : "network";
  } catch { return "network"; }
}

function relayTarget(input) {
  try {
    let request = null, value;
    if (typeof input === "string") value = input;
    else if (input instanceof URL) value = Object.getOwnPropertyDescriptor(URL.prototype, "href").get.call(input);
    else {
      const RequestClass = input instanceof UndiciRequest ? UndiciRequest
        : typeof globalThis.Request === "function" && input instanceof globalThis.Request ? globalThis.Request : null;
      if (!RequestClass) return null;
      request = input; value = Object.getOwnPropertyDescriptor(RequestClass.prototype, "url").get.call(input);
    }
    const url = new URL(value);
    return url.origin === ORIGIN ? { url, request } : null;
  } catch { return null; }
}

function relayOptions(request, init) {
  if (init !== undefined && (!init || typeof init !== "object" || Array.isArray(init))) throw failure("configuration");
  // Node's built-in Request and package Undici have distinct brands. Copy only
  // after recognizing this relay, then submit its URL and effective fetch init;
  // unrelated Request objects must never be reconstructed or consume a body.
  const copied = { ...init };
  const effective = request ? new (request instanceof UndiciRequest ? UndiciRequest : globalThis.Request)(request, copied) : null;
  const base = effective ? {
    method: effective.method, headers: effective.headers, body: effective.body, signal: effective.signal,
    cache: effective.cache, credentials: effective.credentials, integrity: effective.integrity,
    keepalive: effective.keepalive, mode: effective.mode, referrer: effective.referrer,
    referrerPolicy: effective.referrerPolicy, duplex: effective.duplex,
  } : {};
  const options = { ...copied, ...base }, method = String(options.method ?? "GET").toUpperCase();
  const headers = new Headers(options.headers);
  if (!["GET", "POST"].includes(method) || headers.has("host") || headers.has("proxy-authorization")) throw failure("configuration");
  // Forward the validated copy, not an untrusted mutable/getter-backed input.
  return { ...options, method, headers };
}

/**
 * Importing this module is inert. The synchronous factory snapshots only its
 * explicit proxy environment; the owned dispatcher and CA union are lazy.
 * Non-relay fetch calls retain their native input/init/promise unchanged.
 *
 * Node 22.18: https://nodejs.org/download/release/v22.18.0/docs/api/tls.html#tlsgetcacertificatestype
 * Pinned API: https://github.com/nodejs/undici/blob/v7.30.1/docs/docs/api/EnvHttpProxyAgent.md
 * Dependency injection is trusted test code, never environment/profile data.
 */
export function createRelayHttpTransport(options = {}, trustedTestDeps = {}) {
  let proxies, nativeFetch, fetchImpl, agentFactory, getCACertificates;
  try {
    if (!options || typeof options !== "object" || Array.isArray(options)
      || !trustedTestDeps || typeof trustedTestDeps !== "object" || Array.isArray(trustedTestDeps)) throw failure("configuration");
    const { environment = process.env, nativeFetch: native = globalThis.fetch } = options;
    nativeFetch = native;
    ({ fetchImpl = undiciFetch, agentFactory = agentOptions => new EnvHttpProxyAgent(agentOptions),
      getCACertificates = type => tls.getCACertificates(type) } = trustedTestDeps);
    if (!environment || typeof environment !== "object" || environment.NODE_TLS_REJECT_UNAUTHORIZED === "0"
      || typeof nativeFetch !== "function" || typeof fetchImpl !== "function"
      || typeof agentFactory !== "function" || typeof getCACertificates !== "function") throw failure("configuration");
    proxies = proxyOptions(environment);
  } catch { throw failure("configuration"); }

  let dispatcher, initializationFailed = false, closed = false, closing;
  function ownDispatcher() {
    if (closed) throw failure("closed");
    if (initializationFailed) throw failure("configuration");
    if (!dispatcher) {
      try {
        const defaults = getCACertificates("default"), system = getCACertificates("system");
        if (!Array.isArray(defaults) || !Array.isArray(system)) throw failure("configuration");
        const ca = [...new Set([...defaults, ...system])];
        if (!ca.length || ca.some(certificate => typeof certificate !== "string" || !certificate)) throw failure("configuration");
        const trust = { ca, rejectUnauthorized: true, checkServerIdentity: tls.checkServerIdentity, minVersion: "TLSv1.2", timeout: 10_000 };
        const candidate = agentFactory({ ...proxies, connections: 1, headersTimeout: 15_000, bodyTimeout: 15_000,
          connect: { ...trust, servername: HOST }, requestTls: { ...trust, servername: HOST }, proxyTls: { ...trust } });
        if (!candidate || typeof candidate.destroy !== "function") throw failure("configuration");
        dispatcher = candidate;
      } catch { initializationFailed = true; throw failure("configuration"); }
    }
    return dispatcher;
  }

  async function fetchRelay(target, init) {
    if (closed) throw failure("closed");
    let phase = "configuration";
    try {
      const { url, request } = target;
      if (url.username || url.password || url.search || url.hash || url.href !== `${ORIGIN}${url.pathname}`
        || !/^\/sessions(?:\/[A-Za-z0-9_-]{1,256})?$/.test(url.pathname)) throw failure("configuration");
      const options = relayOptions(request, init);
      if (options.signal?.aborted) { phase = "aborted"; throw failure("aborted"); }
      const owned = ownDispatcher();
      if (closed) throw failure("closed");
      phase = "network";
      return await fetchImpl(url.href, { ...options, dispatcher: owned, redirect: "error" });
    } catch (error) {
      throw failure(closed ? "closed" : phase === "network" ? reasonOf(error) : phase);
    }
  }

  return {
    fetch(input, init) {
      const target = relayTarget(input);
      if (!target) return nativeFetch(input, init);
      return fetchRelay(target, init);
    },
    close() {
      if (!closing) {
        closed = true;
        // Destroy cancels only this dispatcher's sockets. Do not gracefully
        // wait for a stuck relay, change global agents, or close sibling users.
        closing = Promise.resolve().then(() => dispatcher?.destroy()).catch(() => { throw failure("close"); });
        closing.catch(() => {});
      }
      return closing;
    },
  };
}
