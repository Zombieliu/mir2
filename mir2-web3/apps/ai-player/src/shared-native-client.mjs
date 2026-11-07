/** Client-only Native entry. Imports are inert; this module never owns a world or Actor.
 * clientRoot is the directory containing the existing EXE and public TOML.
 * clientSettingsDir isolates preferences/exports, not the installation TOML.
 */
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, open, realpath, stat } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, parse, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { R16_GAMEPLAY_BASELINE } from "./generated-gameplay-baseline.mjs";

const EXE = "mir2-platform-windows.exe", TOML = "mir2-client.toml", MAX_CONFIG_BYTES = 32 * 1024;
export const SHARED_NATIVE_CLASSES = Object.freeze(["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]);
const SYSTEM_ENV = new Set(["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "COMSPEC", "TEMP", "TMP", "USERPROFILE", "LANG", "LC_ALL", "TZ"]);
const CREDENTIAL_KEYS = ["account", "password", "passwd", "token", "passkey", "secret", "email", "authorization", "bearer", "credential", "api_key", "private_key", "character_index"];
const error = (code, credentialsPresent) => Object.assign(new Error(code), { code,
  ...(typeof credentialsPresent === "boolean" ? { credentialsPresent } : {}) });
const requireValue = (ok, code = "PUBLIC_CLIENT_CONFIG_INVALID") => { if (!ok) throw error(code); };
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const contains = (parent, child) => { const r = relative(parent, child); return !r || r !== ".." && !r.startsWith(".." + sep) && !isAbsolute(r); };

export function sharedNativeGatewayUrl(value) {
  try {
    requireValue(typeof value === "string" && value.length <= 2048 && value === value.trim() && !/[\u0000-\u0020\u007f]/u.test(value));
    const url = new URL(value), local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
    requireValue(["ws:", "wss:"].includes(url.protocol) && (local || url.protocol === "wss:")
      && !url.username && !url.password && !url.search && !url.hash && ["/ws", "/playtest/ws"].includes(url.pathname));
    return url.href;
  } catch { throw error("EXPLICIT_GATEWAY_URL_REQUIRED"); }
}

function optionsOf(options) {
  requireValue(options && typeof options === "object" && !Array.isArray(options)
    && Object.keys(options).every(k => ["gatewayWsUrl", "clientRoot", "clientSettingsDir"].includes(k)), "INVALID_CLIENT_ARGUMENTS");
  const gatewayWsUrl = sharedNativeGatewayUrl(options.gatewayWsUrl);
  for (const key of ["clientRoot", "clientSettingsDir"]) requireValue(typeof options[key] === "string"
    && isAbsolute(options[key]) && !options[key].includes("\0"), "EXPLICIT_CLIENT_PATHS_REQUIRED");
  const clientRoot = resolve(options.clientRoot), clientSettingsDir = resolve(options.clientSettingsDir);
  requireValue(clientSettingsDir !== parse(clientSettingsDir).root && !contains(clientRoot, clientSettingsDir)
    && !contains(clientSettingsDir, clientRoot), "CLIENT_SETTINGS_MUST_BE_SEPARATE");
  return { gatewayWsUrl, clientRoot, clientSettingsDir };
}

export function parseSharedNativeClientArguments(args) {
  const names = { "--gateway-ws-url": "gatewayWsUrl", "--client-root": "clientRoot", "--client-settings-dir": "clientSettingsDir" }, values = {};
  for (let i = 0; i < args.length; i++) {
    const key = Object.hasOwn(names, args[i]) ? names[args[i]] : null, value = args[++i];
    requireValue(key && !Object.hasOwn(values, key) && typeof value === "string" && !value.startsWith("--"), "INVALID_CLIENT_ARGUMENTS");
    values[key] = value;
  }
  return Object.freeze(optionsOf(values));
}

// Native's public schema contains only tables, scalar values and a string array.
// Unsupported TOML forms fail closed; no general TOML evaluation or value logging.
function statements(text) {
  requireValue(typeof text === "string" && Buffer.byteLength(text) <= MAX_CONFIG_BYTES && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(text));
  text = text.replace(/^\uFEFF/u, "").replaceAll("\r\n", "\n");
  const rows = []; let row = "", quote = null, escaped = false, depth = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quote) {
      requireValue(ch !== "\n" && ch !== "\r"); row += ch;
      if (escaped) escaped = false;
      else if (quote === '"' && ch === "\\") escaped = true;
      else if (ch === quote) quote = null;
      continue;
    }
    if (ch === "#") { while (i < text.length && text[i] !== "\n") i++; }
    else if (ch === '"' || ch === "'") { quote = ch; row += ch; continue; }
    else if (ch === "[") depth++;
    else if (ch === "]") { depth--; requireValue(depth >= 0); }
    if (ch === "\n" || ch === "#") {
      if (depth) row += " ";
      else { if (row.trim()) rows.push(row.trim()); row = ""; }
    } else row += ch;
  }
  requireValue(!quote && depth === 0);
  if (row.trim()) rows.push(row.trim());
  return rows;
}

function quoted(value) {
  if (/^'[^'\r\n]*'$/u.test(value)) return value.slice(1, -1);
  try { const result = JSON.parse(value); requireValue(typeof result === "string"); return result; }
  catch { throw error("PUBLIC_CLIENT_CONFIG_INVALID"); }
}

function keyParts(value) {
  const parts = value.match(/"(?:\\.|[^"\\])*"|'[^']*'|[A-Za-z0-9_-]+|\./gu) ?? [];
  requireValue(parts.join("") === value.replace(/\s+/gu, "") && parts.length % 2 === 1);
  return parts.filter((part, i) => { if (i % 2) requireValue(part === "."); return i % 2 === 0; })
    .map(part => part.startsWith('"') || part.startsWith("'") ? quoted(part) : part);
}

function assignment(row) {
  let quote = null, escaped = false;
  for (let i = 0; i < row.length; i++) {
    const ch = row[i];
    if (quote) { if (escaped) escaped = false; else if (quote === '"' && ch === "\\") escaped = true; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === "=") return [keyParts(row.slice(0, i).trim()), i + 1];
  }
  throw error("PUBLIC_CLIENT_CONFIG_INVALID");
}

function stringArray(value) {
  requireValue(value.startsWith("[") && value.endsWith("]"));
  const body = value.slice(1, -1).trim(); if (!body) return [];
  const values = []; let start = 0, quote = null, escaped = false;
  for (let i = 0; i <= body.length; i++) {
    const ch = body[i];
    if (quote) { if (escaped) escaped = false; else if (quote === '"' && ch === "\\") escaped = true; else if (ch === quote) quote = null; }
    else if (ch === '"' || ch === "'") quote = ch;
    else if (ch === "," || i === body.length) { const part = body.slice(start, i).trim(); if (part) values.push(quoted(part)); else requireValue(i === body.length && start === body.length); start = i + 1; }
  }
  return values;
}

export function inspectSharedNativeConfig(text) {
  const rows = statements(text), parsed = rows.map(row => row.startsWith("[")
    ? { section: keyParts(row.slice(1, -1)) } : { entry: assignment(row), row });
  const hasCredentials = parsed.some(row => (row.section ?? row.entry[0]).some(key => {
    const normalized = key.toLowerCase().replaceAll("-", "_"); return CREDENTIAL_KEYS.some(part => normalized.includes(part));
  }));
  if (hasCredentials) throw error("PUBLIC_CLIENT_CREDENTIALS_PRESENT", true);
  const allowed = { server: ["gateway_ws_url", "auth_http_url", "billing_http_origin", "allowed_classes"],
    gameplay: ["quest_guidance"], display: ["width", "height", "force_daylight"] };
  const values = new Map(), sections = new Set(); let section = null;
  for (const row of parsed) {
    if (row.section) {
      requireValue(row.section.length === 1 && Object.hasOwn(allowed, row.section[0]) && !sections.has(row.section[0]));
      section = row.section[0]; sections.add(section); continue;
    }
    const [parts, valueOffset] = row.entry;
    const [table, key] = section ? [section, parts.length === 1 ? parts[0] : null] : parts;
    requireValue((section ? parts.length === 1 : parts.length === 2) && Object.hasOwn(allowed, table) && allowed[table].includes(key));
    const id = table + "." + key; requireValue(!values.has(id));
    // Credential names have already failed; only approved public values reach this slice/parser.
    const value = row.row.slice(valueOffset).trim();
    if (key === "allowed_classes") values.set(id, stringArray(value));
    else if (["width", "height"].includes(key)) {
      const number = Number(value.replaceAll("_", ""));
      requireValue(/^[1-9][0-9]*(?:_[0-9]+)*$/u.test(value) && Number.isSafeInteger(number) && number > 0 && number <= 0xffffffff); values.set(id, number);
    } else if (key === "force_daylight") { requireValue(["true", "false"].includes(value)); values.set(id, value === "true"); }
    else {
      const string = quoted(value); requireValue(string.trim().length > 0);
      if (key.endsWith("url") || key.endsWith("origin")) {
        let url; try { url = new URL(string); } catch { throw error("PUBLIC_CLIENT_CONFIG_INVALID"); }
        if (url.username || url.password || url.search || url.hash) throw error("PUBLIC_CLIENT_CREDENTIALS_PRESENT", true);
        requireValue(["ws:", "wss:", "http:", "https:"].includes(url.protocol));
      }
      values.set(id, string);
    }
  }
  if (JSON.stringify(values.get("server.allowed_classes")) !== JSON.stringify(SHARED_NATIVE_CLASSES)
    || values.get("gameplay.quest_guidance") !== R16_GAMEPLAY_BASELINE.questGuidance) throw error("CANONICAL_NATIVE_BASELINE_REQUIRED", false);
  return Object.freeze({ credentialsPresent: false, contentProfile: R16_GAMEPLAY_BASELINE.contentProfile,
    questGuidance: R16_GAMEPLAY_BASELINE.questGuidance, allowedClasses: SHARED_NATIVE_CLASSES });
}

export function sharedNativeClientEnvironment(options, inherited = process.env) {
  const selected = optionsOf(options), environment = {};
  for (const key of Object.keys(inherited)) if (SYSTEM_ENV.has(key.toUpperCase())) {
    const value = inherited[key]; requireValue(typeof value === "string" && !value.includes("\0"), "INVALID_CLIENT_ENVIRONMENT");
    environment[key.toUpperCase()] = value;
  }
  return Object.freeze({ ...environment, APPDATA: selected.clientSettingsDir, LOCALAPPDATA: selected.clientSettingsDir,
    MIR2_GATEWAY_WS_URL: selected.gatewayWsUrl,
    MIR2_COMPANION_EXPORT_DIR: join(selected.clientSettingsDir, "mir2-web3", "companion-exports") });
}

async function ordinaryAncestors(path, missing = false) {
  for (let current = resolve(path);;) {
    try { const info = await lstat(current); requireValue(info.isDirectory() && !info.isSymbolicLink(), "UNSAFE_CLIENT_PATH"); }
    catch (cause) { if (!(missing && cause.code === "ENOENT")) throw error(cause.code === "UNSAFE_CLIENT_PATH" ? cause.code : "CLIENT_PATH_UNAVAILABLE"); }
    const parent = dirname(current); if (parent === current) return; current = parent;
  }
}

async function canonicalDestination(path) {
  const suffix = []; let current = path;
  for (;;) {
    try { return resolve(await realpath(current), ...suffix.reverse()); }
    catch (cause) {
      if (cause.code !== "ENOENT" || dirname(current) === current) throw error("CLIENT_PATH_UNAVAILABLE");
      suffix.push(basename(current)); current = dirname(current);
    }
  }
}

async function readInstallation(clientRoot) {
  await ordinaryAncestors(clientRoot);
  const executable = join(clientRoot, EXE), config = join(clientRoot, TOML), pins = [];
  for (const path of [executable, config]) {
    let before; try { before = await lstat(path); } catch { throw error("CLIENT_FILE_UNAVAILABLE"); }
    requireValue(before.isFile() && !before.isSymbolicLink() && before.nlink === 1, "UNSAFE_CLIENT_FILE");
    requireValue(path !== config || before.size > 0 && before.size <= MAX_CONFIG_BYTES, "PUBLIC_CLIENT_CONFIG_INVALID");
    pins.push({ path, size: before.size, mtimeMs: before.mtimeMs, ino: before.ino, dev: before.dev });
  }
  let bytes, header, configFile, exeFile;
  try {
    configFile = await open(config, "r"); exeFile = await open(executable, "r");
    bytes = await configFile.readFile(); header = Buffer.alloc(2); await exeFile.read(header, 0, 2, 0);
  } finally { await Promise.all([configFile?.close(), exeFile?.close()]); }
  requireValue(header.toString("ascii") === "MZ", "NATIVE_EXECUTABLE_REQUIRED");
  let text; try { text = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch { throw error("PUBLIC_CLIENT_CONFIG_INVALID"); }
  const baseline = inspectSharedNativeConfig(text);
  for (const pin of pins) {
    const info = await lstat(pin.path);
    requireValue(info.isFile() && !info.isSymbolicLink() && info.nlink === 1
      && ["size", "mtimeMs", "ino", "dev"].every(key => info[key] === pin[key]), "CLIENT_INSTALLATION_CHANGED");
  }
  const candidates = [join(clientRoot, "mir2-assets"), join(clientRoot, "assets"), join(dirname(clientRoot), "Resources", "mir2-assets")];
  let assets = null;
  for (const candidate of candidates) {
    try { if ((await stat(candidate)).isDirectory()) { assets = candidate; break; } } catch { /* Existing installed assets only; never copy or create. */ }
  }
  requireValue(assets, "NATIVE_ASSETS_UNAVAILABLE");
  return { executable, config, publicConfigSha256: hash(bytes), baseline, assets, pins };
}

/** Read-only planning: no settings directory, server, account or process is created. */
export async function planSharedNativeClient(options, { environment = process.env } = {}) {
  let selected = optionsOf(options);
  await ordinaryAncestors(selected.clientRoot); await ordinaryAncestors(selected.clientSettingsDir, true);
  selected = optionsOf({ ...selected, clientRoot: await realpath(selected.clientRoot), clientSettingsDir: await canonicalDestination(selected.clientSettingsDir) });
  const installation = await readInstallation(selected.clientRoot);
  return Object.freeze({ ...selected, ...installation, cwd: selected.clientRoot,
    environment: sharedNativeClientEnvironment(selected, environment) });
}

/** Only read the unauthenticated realm announcement. Never send a login or game command. */
export function probeSharedNativeGateway(gatewayWsUrl, { WebSocketImpl = globalThis.WebSocket, timeoutMs = 5000 } = {}) {
  const url = sharedNativeGatewayUrl(gatewayWsUrl);
  return new Promise((accept, reject) => {
    let socket, done = false;
    const finish = (failure, value) => {
      if (done) return; done = true; clearTimeout(timer);
      try { socket?.close(); } catch { /* Failed probes never own a service. */ }
      if (failure) reject(error(failure)); else accept(value);
    };
    const timer = setTimeout(() => finish("SHARED_GATEWAY_UNAVAILABLE"), timeoutMs);
    try {
      socket = new WebSocketImpl(url);
      socket.addEventListener("error", () => finish("SHARED_GATEWAY_UNAVAILABLE"));
      socket.addEventListener("close", () => finish("SHARED_GATEWAY_UNAVAILABLE"));
      socket.addEventListener("message", event => {
        try {
          requireValue(typeof event.data === "string" && Buffer.byteLength(event.data) <= 64 * 1024);
          const frame = JSON.parse(event.data); if (frame?.type !== "realmInfo") return;
          const realm = frame.payload;
          if (realm?.profileId !== R16_GAMEPLAY_BASELINE.contentProfile || realm?.profileVersion !== 1
            || ![realm?.realmId, realm?.worldEpoch].every(v => typeof v === "string" && v.length > 0 && v.length <= 256 && !/[\u0000-\u0020\u007f]/u.test(v))) {
            finish("SHARED_GATEWAY_BASELINE_MISMATCH"); return;
          }
          finish(null, Object.freeze({ ready: true, contentProfile: realm.profileId, profileVersion: realm.profileVersion }));
        } catch { finish("SHARED_GATEWAY_UNAVAILABLE"); }
      });
    } catch { finish("SHARED_GATEWAY_UNAVAILABLE"); }
  });
}

/** Wait for this Native child only. Its exit never stops the shared service. */
export async function runSharedNativeClient(options, { environment = process.env, platform = process.platform,
  probe = probeSharedNativeGateway, spawnNative = spawn, report = () => {} } = {}) {
  requireValue(platform === "win32", "WINDOWS_NATIVE_CLIENT_REQUIRED");
  const plan = await planSharedNativeClient(options, { environment });
  let ready;
  try { ready = await probe(plan.gatewayWsUrl); }
  catch (cause) { throw error(cause?.code === "SHARED_GATEWAY_BASELINE_MISMATCH" ? cause.code : "SHARED_GATEWAY_UNAVAILABLE"); }
  requireValue(ready?.ready === true && ready.contentProfile === R16_GAMEPLAY_BASELINE.contentProfile && ready.profileVersion === 1, "SHARED_GATEWAY_BASELINE_MISMATCH");
  const current = await readInstallation(plan.clientRoot);
  requireValue(current.publicConfigSha256 === plan.publicConfigSha256
    && JSON.stringify(current.pins) === JSON.stringify(plan.pins), "CLIENT_INSTALLATION_CHANGED");
  await ordinaryAncestors(plan.clientSettingsDir, true); await mkdir(plan.clientSettingsDir, { recursive: true });
  await ordinaryAncestors(plan.clientSettingsDir);
  requireValue(await realpath(plan.clientSettingsDir) === plan.clientSettingsDir, "UNSAFE_CLIENT_PATH");
  return new Promise((accept, reject) => {
    let child;
    try { child = spawnNative(plan.executable, [], { cwd: plan.cwd, env: plan.environment, shell: false, windowsHide: false, stdio: "ignore" }); }
    catch { reject(error("NATIVE_CLIENT_START_FAILED")); return; }
    child.once("error", () => reject(error("NATIVE_CLIENT_START_FAILED")));
    child.once("spawn", () => { try { report(Object.freeze({ type: "sharedNativeClientStarted", pid: child.pid, credentialsPresent: false })); } catch { /* Reporting cannot take ownership of the child or service. */ } });
    child.once("exit", code => {
      if (code !== 0) { reject(error("NATIVE_CLIENT_EXIT_FAILED")); return; }
      accept(Object.freeze({ type: "sharedNativeClientClosed", exitCode: 0, credentialsPresent: false }));
    });
  });
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  (async () => {
    const result = await runSharedNativeClient(parseSharedNativeClientArguments(process.argv.slice(2)), {
      report: value => process.stdout.write(JSON.stringify(value) + "\n"),
    });
    process.stdout.write(JSON.stringify(result) + "\n");
  })().catch(cause => {
    process.exitCode = 1;
    process.stderr.write(JSON.stringify({ type: "sharedNativeClientFailed",
      code: /^[A-Z_]+$/u.test(cause?.code ?? "") ? cause.code : "SHARED_NATIVE_CLIENT_UNAVAILABLE",
      ...(typeof cause?.credentialsPresent === "boolean" ? { credentialsPresent: cause.credentialsPresent } : {}) }) + "\n");
  });
}
