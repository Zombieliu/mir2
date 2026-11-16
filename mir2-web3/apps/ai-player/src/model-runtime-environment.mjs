import { open } from "node:fs/promises";
import { resolve } from "node:path";

export const MODEL_PROXY_KEYS = Object.freeze(["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY", "no_proxy", "NO_PROXY"]);
const error = () => Object.assign(new Error("The local model network configuration is unavailable."), { code: "LOCAL_MODEL_NETWORK_UNAVAILABLE" });
const record = value => value && typeof value === "object" && !Array.isArray(value);

/** Process-only proxy credentials may be inherited, but are never logged or persisted. */
export function modelProcessEnvironment(apiKey, environment = process.env) {
  const selected = {};
  if (typeof apiKey === "string") selected.OPENAI_API_KEY = apiKey;
  for (const key of MODEL_PROXY_KEYS) {
    const value = environment[key];
    if (value === undefined) continue;
    if (typeof value !== "string" || value.length > 8192 || /[\p{Cc}\p{Cf}\p{Cs}]/u.test(value)) throw error();
    selected[key] = value;
  }
  return selected;
}

/** Optional machine-local network settings contain no API or proxy password. */
export async function loadLocalModelNetwork({ bundleDir, environment = process.env }) {
  let file;
  try {
    file = await open(resolve(bundleDir, "apps/ai-player/.data/local-playtest/openai-network.local.json"), "r");
  } catch (e) { if (e.code === "ENOENT") return modelProcessEnvironment(undefined, environment); throw error(); }
  try {
    const bytes = Buffer.alloc(16 * 1024 + 1);
    let total = 0;
    while (total < bytes.length) { const { bytesRead } = await file.read(bytes, total, bytes.length - total, null); if (!bytesRead) break; total += bytesRead; }
    if (total > 16 * 1024) throw error();
    const settings = JSON.parse(bytes.toString("utf8", 0, total));
    if (!record(settings) || Object.keys(settings).length !== 2 || settings.schemaVersion !== 1 || !record(settings.proxies) || Object.keys(settings.proxies).some(k => !MODEL_PROXY_KEYS.includes(k))) throw error();
    const local = modelProcessEnvironment(undefined, settings.proxies);
    for (const [key, value] of Object.entries(local)) {
      if (key.toLowerCase() === "no_proxy" || value === "") continue;
      const url = new URL(value);
      if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password || url.search || url.hash || !["", "/"].includes(url.pathname)) throw error();
    }
    return { ...local, ...modelProcessEnvironment(undefined, environment) };
  } catch { throw error(); }
  finally { await file.close(); }
}
