import childProcess from "node:child_process";
import { open, realpath } from "node:fs/promises";
import { resolve } from "node:path";

const MAX_BYTES = 8192;
const DEFAULT_MODEL = "gpt-6-luna";
const ERROR = "Local OpenAI settings are invalid or unavailable.";
const MODEL = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,99}$/;
const TARGET = /^NumeronAlo\/OpenAI\/v1\/[a-f0-9]{64}$/;
const KEY = /^sk-[A-Za-z0-9_-]{13,2045}$/;
// JavaScript's $ may match before a final newline; credentials/configuration
// require the entire value, including its actual final character, to be valid.
const fullMatch = (pattern, value) => typeof value === "string" && value.trim() === value && pattern.test(value);
const rules = () => ({ provider: "rules", model: "rules", credentialRef: null });
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const exactKeys = (value, keys) => record(value) && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));

async function readBounded(path) {
  const file = await open(path, "r");
  try {
    if (!(await file.stat()).isFile()) throw new Error(ERROR);
    const buffer = Buffer.alloc(MAX_BYTES + 1);
    let length = 0;
    while (length < buffer.length) {
      const { bytesRead } = await file.read(buffer, length, buffer.length - length, null);
      if (!bytesRead) break;
      length += bytesRead;
    }
    if (length > MAX_BYTES) throw new Error(ERROR);
    return buffer.toString("utf8", 0, length);
  } finally { await file.close(); }
}

function readCredential(helper, root, env) {
  // The helper receives neither API credentials nor Codex/AI Gateway variables.
  const childEnv = {};
  for (const key of ["SystemRoot", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"]) {
    if (typeof env[key] === "string") childEnv[key] = env[key];
  }
  return new Promise((resolveResult, reject) => {
    const child = childProcess.execFile(helper, ["--read-key"], {
      cwd: root, env: childEnv, windowsHide: true, encoding: "utf8", timeout: 5000, maxBuffer: MAX_BYTES,
    }, (error, stdout) => error ? reject(new Error(ERROR)) : resolveResult(stdout));
    child.stdin?.end();
  });
}

/** Returns an API key only in memory. Never persist or log the returned object. */
export async function loadLocalModelSettings({ bundleDir, env = process.env }) {
  try {
    if (typeof bundleDir !== "string" || !bundleDir) throw new Error(ERROR);
    const root = await realpath(resolve(bundleDir));
    let encoded;
    try { encoded = await readBounded(resolve(root, "apps/ai-player/.data/local-playtest/openai.local.json")); }
    catch (error) {
      if (error.code !== "ENOENT") throw error;
      // Only this explicit variable is eligible; Codex and AI Gateway keys are
      // never inspected, copied, guessed or substituted.
      const apiKey = typeof env.OPENAI_API_KEY === "string" ? env.OPENAI_API_KEY.trim() : "";
      if (!apiKey) return rules();
      if (!fullMatch(KEY, apiKey)) throw new Error(ERROR);
      return { provider: "openai", model: DEFAULT_MODEL, credentialRef: "OPENAI_API_KEY", apiKey, maxCallsPerHour: 20 };
    }
    const settings = JSON.parse(encoded);
    if (!exactKeys(settings, ["schemaVersion", "provider", "enabled", "model", "credentialTarget", "maxCallsPerHour"]) ||
      settings.schemaVersion !== 1 || settings.provider !== "openai" || settings.enabled !== true ||
      !fullMatch(MODEL, settings.model) || !fullMatch(TARGET, settings.credentialTarget) || settings.maxCallsPerHour !== 20) throw new Error(ERROR);
    const stdout = await readCredential(resolve(root, "Numeron-Alo-Settings.exe"), root, env);
    if (typeof stdout !== "string" || Buffer.byteLength(stdout) > MAX_BYTES) throw new Error(ERROR);
    const credential = JSON.parse(stdout);
    if (!exactKeys(credential, ["credentialTarget", "apiKey"]) || credential.credentialTarget !== settings.credentialTarget ||
      !fullMatch(KEY, credential.apiKey)) throw new Error(ERROR);
    return { provider: "openai", model: settings.model, credentialRef: "OPENAI_API_KEY", apiKey: credential.apiKey, maxCallsPerHour: 20 };
  } catch {
    const error = new Error(ERROR);
    error.code = "LOCAL_MODEL_SETTINGS_UNAVAILABLE";
    throw error;
  }
}
