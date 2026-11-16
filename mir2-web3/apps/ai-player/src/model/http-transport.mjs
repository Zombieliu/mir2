import tls from "node:tls";
import { EnvHttpProxyAgent, fetch as undiciFetch } from "undici";

const ENDPOINT = "https://api.openai.com/v1/responses";
const HOST = "api.openai.com";
const MESSAGE = "The model HTTPS transport is unavailable.";

function failure(reason = "network") {
  const error = new Error(MESSAGE);
  error.code = "MODEL_TRANSPORT_UNAVAILABLE";
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
  // Pass all three explicitly, even when empty. Undici must not silently read
  // different process.env values after this private environment snapshot.
  return { httpProxy: proxy(http ?? all ?? ""), httpsProxy: proxy(https ?? http ?? all ?? ""), noProxy };
}

function reasonOf(error) {
  // Never retain the original exception: it can contain a proxy URL/password.
  try {
    const codes = [error?.code, error?.cause?.code];
    if (codes.some(code => ["UNABLE_TO_GET_ISSUER_CERT_LOCALLY", "UNABLE_TO_VERIFY_LEAF_SIGNATURE", "SELF_SIGNED_CERT_IN_CHAIN", "DEPTH_ZERO_SELF_SIGNED_CERT", "CERT_HAS_EXPIRED", "ERR_TLS_CERT_ALTNAME_INVALID"].includes(code))) return "tls-trust";
    if (codes.some(code => ["ETIMEDOUT", "UND_ERR_CONNECT_TIMEOUT", "UND_ERR_HEADERS_TIMEOUT", "UND_ERR_BODY_TIMEOUT"].includes(code))) return "timeout";
    return error?.name === "AbortError" ? "aborted" : "network";
  } catch { return "network"; }
}

/**
 * Synchronous factory; fetch() and close() return promises. Uses only its own
 * dispatcher, never a global agent. The second argument is trusted test-only
 * dependency injection, not configuration/profile/environment data.
 *
 * Node 22.18 supports getCACertificates('system') without a process-wide flag:
 * https://nodejs.org/download/release/v22.18.0/docs/api/tls.html#tlsgetcacertificatestype
 * Proxy/no-proxy semantics are supplied by pinned official Undici 7.30.1:
 * https://github.com/nodejs/undici/blob/v7.30.1/docs/docs/api/EnvHttpProxyAgent.md
 */
export function createModelFetch({ environment = process.env } = {}, {
  fetchImpl = undiciFetch,
  agentFactory = options => new EnvHttpProxyAgent(options),
  getCACertificates = type => tls.getCACertificates(type),
} = {}) {
  let dispatcher;
  try {
    if (!environment || typeof environment !== "object" || environment.NODE_TLS_REJECT_UNAUTHORIZED === "0" ||
      typeof fetchImpl !== "function" || typeof agentFactory !== "function" || typeof getCACertificates !== "function") throw failure("configuration");
    const proxies = proxyOptions(environment);
    const ca = [...new Set([...getCACertificates("default"), ...getCACertificates("system")])];
    if (!ca.length || ca.some(certificate => typeof certificate !== "string" || !certificate)) throw failure("configuration");
    const trust = { ca, rejectUnauthorized: true, checkServerIdentity: tls.checkServerIdentity, minVersion: "TLSv1.2", timeout: 10_000 };
    dispatcher = agentFactory({ ...proxies, connections: 1, headersTimeout: 15_000, bodyTimeout: 15_000,
      connect: { ...trust, servername: HOST }, requestTls: { ...trust, servername: HOST }, proxyTls: { ...trust } });
    if (!dispatcher || typeof dispatcher.destroy !== "function") throw failure("configuration");
  } catch { throw failure("configuration"); }

  let closed = false, closing;
  return {
    async fetch(input, init = {}) {
      if (closed) throw failure("closed");
      try {
        if (!(typeof input === "string" || input instanceof URL) || new URL(input).href !== ENDPOINT ||
          !init || typeof init !== "object" || Array.isArray(init) || String(init.method ?? "GET").toUpperCase() !== "POST") throw failure("configuration");
        const headers = new Headers(init.headers);
        if (headers.has("host") || headers.has("proxy-authorization")) throw failure("configuration");
        return await fetchImpl(ENDPOINT, { ...init, dispatcher, redirect: "error" });
      } catch (error) {
        let configuration = false;
        try { configuration = error?.code === "MODEL_TRANSPORT_UNAVAILABLE" && error?.reason === "configuration"; } catch { /* untrusted error getters */ }
        throw failure(configuration ? "configuration" : reasonOf(error));
      }
    },
    close() {
      if (!closing) {
        closed = true;
        // Destroy cancels owned sockets promptly; graceful close could wait for
        // a stalled response indefinitely. No process, global agent or sibling
        // model runtime is affected.
        closing = Promise.resolve().then(() => dispatcher.destroy()).catch(() => { throw failure("close"); });
        closing.catch(() => {});
      }
      return closing;
    },
  };
}
