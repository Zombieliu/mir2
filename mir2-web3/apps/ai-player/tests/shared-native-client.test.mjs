import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { execFileSync } from "node:child_process";
import { access, link, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { SHARED_NATIVE_CLASSES, inspectSharedNativeConfig, parseSharedNativeClientArguments,
  planSharedNativeClient, probeSharedNativeGateway, runSharedNativeClient, sharedNativeClientEnvironment,
  sharedNativeGatewayUrl } from "../src/shared-native-client.mjs";

const canonical = `[server]\ngateway_ws_url = "ws://127.0.0.1:19040/ws"\nallowed_classes = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]\n[display]\nwidth = 1024\nheight = 768\nforce_daylight = true\n[gameplay]\nquest_guidance = "newcomer-v2"\n`;
const ready = () => ({ ready: true, contentProfile: "platinum_fiveclass", profileVersion: 1 });
const failCode = code => e => e.code === code;
async function fixture(t, text = canonical) {
  const root = await mkdtemp(join(tmpdir(), "mir2-shared-native-"));
  t.after(async () => {
    assert.equal(dirname(root), resolve(tmpdir())); assert(basename(root).startsWith("mir2-shared-native-"));
    await rm(root, { recursive: true, force: true });
  });
  const clientRoot = join(root, "existing Native client"), clientSettingsDir = join(root, "independent preferences", "中文");
  await mkdir(join(clientRoot, "mir2-assets"), { recursive: true });
  const exe = join(clientRoot, "mir2-platform-windows.exe"), config = join(clientRoot, "mir2-client.toml");
  await writeFile(exe, "MZsynthetic-native-never-executed"); await writeFile(config, text);
  return { root, exe, config, options: { gatewayWsUrl: "ws://127.0.0.1:19040/ws", clientRoot, clientSettingsDir } };
}
function fakeSpawn(calls, { exitCode = 0, startupError = false } = {}) {
  return (...args) => {
    calls.push(args); const child = new EventEmitter(); child.pid = 1234;
    child.kill = () => assert.fail("A client-only connector must not kill any process");
    queueMicrotask(() => { if (startupError) child.emit("error", new Error("synthetic-sensitive-spawn-detail"));
      else { child.emit("spawn"); queueMicrotask(() => child.emit("exit", exitCode)); } });
    return child;
  };
}
function socketFixture(events) {
  const calls = [];
  class Socket extends EventTarget {
    constructor(url) {
      super(); this.url = url; this.closed = 0; calls.push(this);
      queueMicrotask(() => { for (const [type, data] of events) {
        this.dispatchEvent(type === "message" ? new MessageEvent(type, { data }) : new Event(type));
      } });
    }
    close() { this.closed++; this.dispatchEvent(new Event("close")); }
    send() { assert.fail("Preflight cannot send a login, heartbeat, game or model command"); }
  }
  return { calls, Socket };
}
const realm = (update = {}) => JSON.stringify({ type: "realmInfo", payload: { realmId: "ai-shared-fixture", worldEpoch: "epoch-fixture",
  profileId: "platinum_fiveclass", profileVersion: 1, ...update } });

test("an explicit endpoint and separate absolute paths are mandatory; there is no local-world fallback", () => {
  const root = resolve(tmpdir(), "explicit-native"), options = { gatewayWsUrl: "ws://127.0.0.1:19040/ws", clientRoot: join(root, "client"), clientSettingsDir: join(root, "preferences") };
  const args = ["--gateway-ws-url", options.gatewayWsUrl, "--client-root", options.clientRoot, "--client-settings-dir", options.clientSettingsDir];
  assert.deepEqual(parseSharedNativeClientArguments(args), options);
  for (const bad of [[], args.slice(2), [...args, "--start-local-world"], [...args, "--client-root", options.clientRoot],
    args.map(v => v === options.clientRoot ? "relative" : v), args.map(v => v === options.clientSettingsDir ? options.clientRoot : v),
    args.map(v => v === options.clientSettingsDir ? dirname(options.clientRoot) : v)]) assert.throws(() => parseSharedNativeClientArguments(bad));
  for (const url of [undefined, "", " ws://127.0.0.1:19040/ws", "ws://remote.example/ws", "https://remote.example/ws",
    "wss://user:never-print-this@remote.example/ws", "ws://127.0.0.1:19040/ws?token=never-print-this", "ws://127.0.0.1:19040/ws#fragment", "ws://127.0.0.1:19040/"])
    assert.throws(() => sharedNativeGatewayUrl(url), failCode("EXPLICIT_GATEWAY_URL_REQUIRED"));
  assert.equal(sharedNativeGatewayUrl("wss://gateway.example/playtest/ws"), "wss://gateway.example/playtest/ws");
  assert.equal(sharedNativeGatewayUrl("ws://[::1]:19040/ws"), "ws://[::1]:19040/ws");
});

test("canonical public TOML supports ordinary comments, literal strings and multiline class arrays", () => {
  const text = canonical.replace('allowed_classes = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]',
    "allowed_classes = [\n 'Warrior', # public class name\n 'Wizard', 'Taoist',\n 'Assassin', 'Archer',\n]")
    .replace('quest_guidance = "newcomer-v2"', "quest_guidance = 'newcomer-v2' # account/password are not stored here");
  assert.deepEqual(inspectSharedNativeConfig(text), { credentialsPresent: false, contentProfile: "platinum_fiveclass",
    questGuidance: "newcomer-v2", allowedClasses: SHARED_NATIVE_CLASSES });
});

test("three-class, old guidance, missing fields and reordered/duplicated class policies fail closed", () => {
  for (const text of [canonical.replace(', "Assassin", "Archer"', ""), canonical.replace("newcomer-v2", "newcomer-v1"),
    canonical.replace(/^quest_guidance.*$/m, ""), canonical.replace("Archer", "Warrior"), canonical.replace('"Warrior", "Wizard"', '"Wizard", "Warrior"')]) {
    assert.throws(() => inspectSharedNativeConfig(text), e => e.code === "CANONICAL_NATIVE_BASELINE_REQUIRED" && e.credentialsPresent === false);
  }
});

test("credential keys, escaped key names, hidden sections and URL credentials expose only a presence boolean", () => {
  const secret = "SYNTHETIC_SECRET_DO_NOT_EMIT";
  for (const extra of [`account_id = "${secret}"`, `password = "${secret}"`, `"pass\\u0077ord" = "${secret}"`,
    `server.token = "${secret}"`, `[credentials]\nvalue = "${secret}"`, `authorization = "Bearer ${secret}"`]) {
    assert.throws(() => inspectSharedNativeConfig(canonical + "\n" + extra + "\n"), e => {
      assert.equal(e.code, "PUBLIC_CLIENT_CREDENTIALS_PRESENT"); assert.equal(e.credentialsPresent, true);
      assert(!JSON.stringify(e).includes(secret)); assert(!e.message.includes(secret)); assert(!e.stack.includes(secret)); return true;
    });
  }
  for (const value of ["ws://user:" + secret + "@127.0.0.1:19040/ws", "ws://127.0.0.1:19040/ws?password=" + secret]) {
    assert.throws(() => inspectSharedNativeConfig(canonical.replace("ws://127.0.0.1:19040/ws", value)), e => e.code === "PUBLIC_CLIENT_CREDENTIALS_PRESENT" && e.credentialsPresent === true);
  }
});

test("unsupported or ambiguous public TOML cannot silently become a valid baseline", () => {
  for (const text of [canonical + '[server]\nallowed_classes = []\n', canonical + 'unknown = "ignored"\n',
    canonical.replace("width = 1024", "width = 0"), canonical.replace("width = 1024", "width = 1.5"),
    canonical.replace('"Warrior", "Wizard"', '"Warrior",, "Wizard"'), canonical + '\u0000', canonical.replace("[server]", "[[server]]")])
    assert.throws(() => inspectSharedNativeConfig(text), failCode("PUBLIC_CLIENT_CONFIG_INVALID"));
});

test("the child environment allowlist does not even read inherited credential values", async t => {
  const { options } = await fixture(t), inherited = { Path: "fixture-system-bin", SystemRoot: "fixture-system", LANG: "zh-CN" };
  for (const key of ["MIR2_NATIVE_ACCOUNT", "MIR2_NATIVE_PASSWORD", "MIR2_NATIVE_CHARACTER_INDEX", "OPENAI_API_KEY", "anthropic_api_key",
    "MIR2_COMPANION_ARCHIVE_KEY", "MIR2_SAVE_RECOVERY_MAC_KEY", "MIR2_IDENTITY_SESSION_SECRET", "AWS_SECRET_ACCESS_KEY", "HTTPS_PROXY",
    "NODE_OPTIONS", "MIR2_ASSET_ROOT", "MIR2_GATEWAY_WS_URL", "APPDATA", "LOCALAPPDATA"])
    Object.defineProperty(inherited, key, { enumerable: true, get() { assert.fail("Credential or redirection environment value was read"); } });
  const env = sharedNativeClientEnvironment(options, inherited);
  assert.deepEqual(Object.keys(env).sort(), ["APPDATA", "LANG", "LOCALAPPDATA", "MIR2_COMPANION_EXPORT_DIR", "MIR2_GATEWAY_WS_URL", "PATH", "SYSTEMROOT"].sort());
  assert.equal(env.APPDATA, options.clientSettingsDir); assert.equal(env.LOCALAPPDATA, options.clientSettingsDir);
  assert.equal(env.MIR2_GATEWAY_WS_URL, options.gatewayWsUrl); assert.equal(env.PATH, "fixture-system-bin");
  assert.equal(env.MIR2_COMPANION_EXPORT_DIR, join(options.clientSettingsDir, "mir2-web3", "companion-exports"));
});

test("planning only reads the installed EXE/public TOML and reuses existing resources without creating preferences", async t => {
  const f = await fixture(t), beforeConfig = await readFile(f.config), beforeExe = await readFile(f.exe);
  const plan = await planSharedNativeClient(f.options, { environment: {} });
  assert.equal(plan.executable, f.exe); assert.equal(plan.cwd, f.options.clientRoot);
  assert.equal(plan.config, f.config); assert.equal(plan.assets, join(f.options.clientRoot, "mir2-assets"));
  assert.equal(plan.baseline.credentialsPresent, false); assert.equal(plan.baseline.questGuidance, "newcomer-v2");
  await assert.rejects(access(f.options.clientSettingsDir), { code: "ENOENT" });
  assert.deepEqual(await readFile(f.config), beforeConfig); assert.deepEqual(await readFile(f.exe), beforeExe);
  assert.deepEqual((await readdir(f.options.clientRoot)).sort(), ["mir2-assets", "mir2-client.toml", "mir2-platform-windows.exe"]);
});

test("missing EXE, public configuration or installed resource directory never probes or starts anything", async t => {
  for (const missing of ["mir2-platform-windows.exe", "mir2-client.toml", "mir2-assets"]) {
    const f = await fixture(t), target = resolve(f.options.clientRoot, missing);
    assert.equal(dirname(target), f.options.clientRoot); await rm(target, { recursive: true });
    await assert.rejects(runSharedNativeClient(f.options, { platform: "win32", environment: {},
      probe() { assert.fail("A missing installation cannot probe a service"); }, spawnNative() { assert.fail("Cannot spawn"); } }));
    await assert.rejects(access(f.options.clientSettingsDir), { code: "ENOENT" });
  }
});

test("old baseline or public credentials fail before preflight and leave the installation untouched", async t => {
  for (const text of [canonical.replace(', "Assassin", "Archer"', ""), canonical + 'password = "synthetic-never-log"\n']) {
    const f = await fixture(t, text);
    await assert.rejects(runSharedNativeClient(f.options, { platform: "win32", environment: {},
      probe() { assert.fail("Cannot probe an invalid config"); }, spawnNative() { assert.fail("Cannot spawn"); } }));
    assert.equal(await readFile(f.config, "utf8"), text);
    await assert.rejects(access(f.options.clientSettingsDir), { code: "ENOENT" });
  }
});

test("linked installations/settings and hardlinked executable files are rejected", async t => {
  const f = await fixture(t), redirected = join(f.root, "linked client");
  await symlink(f.options.clientRoot, redirected, "junction");
  await assert.rejects(planSharedNativeClient({ ...f.options, clientRoot: redirected }), failCode("UNSAFE_CLIENT_PATH"));
  await link(f.exe, join(f.root, "second-exe-link"));
  await assert.rejects(planSharedNativeClient(f.options), failCode("UNSAFE_CLIENT_FILE"));
  const g = await fixture(t), target = join(g.root, "other-preferences"); await mkdir(target);
  await mkdir(dirname(g.options.clientSettingsDir), { recursive: true });
  await symlink(target, g.options.clientSettingsDir, "junction");
  await assert.rejects(planSharedNativeClient(g.options), failCode("UNSAFE_CLIENT_PATH"));
});

test("unavailable services are reported safely without creating settings or substituting a local world", async t => {
  const f = await fixture(t), calls = [];
  await assert.rejects(runSharedNativeClient(f.options, { platform: "win32", environment: {},
    async probe(url) { calls.push(url); throw new Error("synthetic-sensitive-network-detail"); }, spawnNative() { assert.fail("Unreachable service cannot start Native"); } }),
  e => e.code === "SHARED_GATEWAY_UNAVAILABLE" && !e.message.includes("synthetic-sensitive"));
  assert.deepEqual(calls, [f.options.gatewayWsUrl]); await assert.rejects(access(f.options.clientSettingsDir), { code: "ENOENT" });
});

test("preflight reads only the public R16 realm announcement and closes its own unauthenticated socket", async () => {
  const f = socketFixture([["message", JSON.stringify({ type: "packet", packet: "Connected" })], ["message", realm()]]);
  assert.deepEqual(await probeSharedNativeGateway("ws://127.0.0.1:19040/ws", { WebSocketImpl: f.Socket }), ready());
  assert.equal(f.calls.length, 1); assert.equal(f.calls[0].url, "ws://127.0.0.1:19040/ws"); assert.equal(f.calls[0].closed, 1);
});

test("preflight rejects wrong profiles, missing world identity, malformed frames, close, error and timeout", async () => {
  for (const payload of [realm({ profileId: "platinum_176" }), realm({ profileVersion: 2 }), realm({ worldEpoch: "" }), realm({ realmId: null })]) {
    const f = socketFixture([["message", payload]]);
    await assert.rejects(probeSharedNativeGateway("ws://127.0.0.1:19040/ws", { WebSocketImpl: f.Socket }), failCode("SHARED_GATEWAY_BASELINE_MISMATCH"));
    assert.equal(f.calls[0].closed, 1);
  }
  for (const events of [[["message", "{malformed"]], [["error"]], [["close"]], []]) {
    const f = socketFixture(events);
    await assert.rejects(probeSharedNativeGateway("ws://127.0.0.1:19040/ws", { WebSocketImpl: f.Socket, timeoutMs: 5 }), failCode("SHARED_GATEWAY_UNAVAILABLE"));
    assert.equal(f.calls[0].closed, 1);
  }
});

test("the explicit client plan starts one original Native child and its exit never stops the service", async t => {
  const f = await fixture(t), calls = [], reports = [], before = await readFile(f.config);
  const result = await runSharedNativeClient(f.options, { platform: "win32", environment: { OPENAI_API_KEY: "synthetic-secret" },
    probe: async url => { assert.equal(url, f.options.gatewayWsUrl); return ready(); }, spawnNative: fakeSpawn(calls), report: r => reports.push(r) });
  assert.deepEqual(result, { type: "sharedNativeClientClosed", exitCode: 0, credentialsPresent: false });
  assert.deepEqual(reports, [{ type: "sharedNativeClientStarted", pid: 1234, credentialsPresent: false }]);
  assert.equal(calls.length, 1); assert.equal(calls[0][0], f.exe); assert.deepEqual(calls[0][1], []);
  assert.deepEqual(Object.fromEntries(Object.entries(calls[0][2]).filter(([k]) => k !== "env")), { cwd: f.options.clientRoot, shell: false, windowsHide: false, stdio: "ignore" });
  assert.equal(calls[0][2].env.OPENAI_API_KEY, undefined); assert.equal(calls[0][2].env.MIR2_NATIVE_ACCOUNT, undefined);
  assert.equal((await lstat(f.options.clientSettingsDir)).isDirectory(), true); assert.deepEqual(await readFile(f.config), before);
});

test("configuration replacement or newly added credentials during preflight block the Native launch", async t => {
  for (const update of [canonical + "# changed during probe\n", canonical + 'password = "synthetic-late-credential"\n']) {
    const f = await fixture(t);
    await assert.rejects(runSharedNativeClient(f.options, { platform: "win32", environment: {},
      async probe() { await writeFile(f.config, update); return ready(); }, spawnNative() { assert.fail("Changed installation cannot spawn"); } }));
    await assert.rejects(access(f.options.clientSettingsDir), { code: "ENOENT" });
  }
});

test("spawn and nonzero Native exit failures are sanitized and never retried", async t => {
  for (const kind of [{ startupError: true }, { exitCode: 9 }]) {
    const f = await fixture(t), calls = [];
    await assert.rejects(runSharedNativeClient(f.options, { platform: "win32", environment: {}, probe: async () => ready(), spawnNative: fakeSpawn(calls, kind) }),
      e => ["NATIVE_CLIENT_START_FAILED", "NATIVE_CLIENT_EXIT_FAILED"].includes(e.code) && !e.message.includes("synthetic-sensitive"));
    assert.equal(calls.length, 1);
  }
});

test("importing the connector is inert and requires no model, credentials, local fixture or service", () => {
  const moduleUrl = new URL("../src/shared-native-client.mjs", import.meta.url).href;
  const output = execFileSync(process.execPath, ["--input-type=module", "-e",
    `globalThis.WebSocket = class { constructor() { throw Error("unexpected preflight"); } }; await import(${JSON.stringify(moduleUrl)}); console.log("inert");`], { encoding: "utf8" });
  assert.equal(output.trim(), "inert");
  assert.equal(dirname(fileURLToPath(new URL("../src/shared-native-client.mjs", import.meta.url))).endsWith("src"), true);
});
