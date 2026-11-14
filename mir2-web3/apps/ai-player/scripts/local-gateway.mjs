import { spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, mkdir, realpath, stat, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { relative, resolve, isAbsolute, parse } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { localSupplyServerEnvironment, withinPrivateRoot } from "../src/local-supply-server-config.mjs";
import { R16_GAMEPLAY_BASELINE as SHARED_GAMEPLAY_BASELINE } from "../src/generated-gameplay-baseline.mjs";

export const appRoot = fileURLToPath(new URL("../", import.meta.url));
export const repoRoot = resolve(appRoot, "../..");
// Read from and hash-checked against the signed public R16 GENERATION.json.
// Resource release identity and local server gameplay configuration are separate facts.
export const R16_GAMEPLAY_BASELINE = Object.freeze({
  ...SHARED_GAMEPLAY_BASELINE, profileVersion: 1,
  generationManifestSha256: "56ddd5417a09a2515fece9a36be2607282084ec89058c412d2edd7a7dcb21bc8",
});

/** Both named and legacy positional paths are accepted; duplicates never win. */
export function gatewayExecutableArgument(args = []) {
  if (!Array.isArray(args) || args.some(value => typeof value !== "string" || !value.trim())) throw new Error("Use --gateway-exe <path>.");
  if (args.length === 0) return undefined;
  if (args.length === 1 && !args[0].startsWith("-")) return resolve(args[0]);
  if (args.length === 2 && args[0] === "--gateway-exe" && !args[1].startsWith("-")) return resolve(args[1]);
  throw new Error("Use --gateway-exe <path>, or one gateway executable path.");
}

export async function resolveGatewayExecutable(executable, { root = repoRoot } = {}) {
  const suffix = process.platform === "win32" ? ".exe" : "";
  const candidates = executable ? [resolve(executable)] : [
    resolve(root, `target/debug/mir2-gateway${suffix}`),
    resolve(root, `target-ai-player/debug/mir2-gateway${suffix}`),
  ];
  for (const candidate of candidates) {
    try { if ((await stat(candidate)).isFile()) return await realpath(candidate); }
    catch (error) { if (!["ENOENT", "ENOTDIR"].includes(error.code)) throw error; }
  }
  throw new Error("Build the canonical gateway with Rust 1.95.0, or pass --gateway-exe <path>.");
}

export async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

export function isolatedEnvironment(overrides, environment = process.env) {
  const keys = new Set(["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "COMSPEC", "LANG", "LC_ALL", "TZ"]);
  return { ...Object.fromEntries(Object.entries(environment).filter(([key]) => keys.has(key.toUpperCase()))), ...overrides };
}

/** An explicit independent test root does not broaden the default app/.data
 * reuse boundary. Reused stores must remain under the currently selected root. */
export async function resolveLocalOutputRoot(outputRoot) {
  if (outputRoot !== undefined && (typeof outputRoot !== "string" || !isAbsolute(outputRoot))) throw new Error("Use an explicit absolute private output root.");
  const root = resolve(outputRoot ?? resolve(appRoot, ".data"));
  if (root === parse(root).root || outputRoot !== undefined && withinPrivateRoot(root, resolve(appRoot), { allowRoot: true })) throw new Error("Use a dedicated private output directory.");
  await mkdir(root, { recursive: true });
  const canonicalRoot = await realpath(root);
  if (canonicalRoot === parse(canonicalRoot).root || withinPrivateRoot(canonicalRoot, resolve(appRoot), { allowRoot: true })) throw new Error("Use a dedicated private output directory.");
  return canonicalRoot;
}

export async function prepareLocalGatewayPaths({ prefix, runId, reuse, outputRoot } = {}) {
  if (typeof prefix !== "string" || !/^[A-Za-z][A-Za-z0-9-]{0,63}$/.test(prefix) || typeof runId !== "string" || !/^[a-f0-9-]{36}$/.test(runId)) throw new Error("Invalid private gateway output identity.");
  const root = await resolveLocalOutputRoot(outputRoot);
  let source, reuseOutput;
  if (reuse) {
    if (typeof reuse !== "object" || Array.isArray(reuse) || Object.keys(reuse).some(key => !["output", "realmId"].includes(key)) || typeof reuse.output !== "string" || !isAbsolute(reuse.output) || typeof reuse.realmId !== "string" || !/^ai-[A-Za-z0-9-]+$/.test(reuse.realmId)) throw new Error("Only this helper's private local account store may be reused.");
    reuseOutput = await realpath(reuse.output);
    const candidate = resolve(reuseOutput, "accounts.json"), metadata = await lstat(candidate);
    source = await realpath(candidate);
    if (!metadata.isFile() || metadata.isSymbolicLink() || !withinPrivateRoot(root, reuseOutput) || !withinPrivateRoot(root, source) || relative(reuseOutput, source) !== "accounts.json") throw new Error("Only this helper's private local account store may be reused.");
  }
  const output = resolve(root, `${prefix}-${runId}`);
  if (!withinPrivateRoot(root, output)) throw new Error("Invalid private gateway output identity.");
  await mkdir(output, { recursive: false });
  const canonicalOutput = await realpath(output);
  if (!withinPrivateRoot(root, canonicalOutput)) throw new Error("Private gateway output escaped its selected root.");
  return { outputRoot: root, output: canonicalOutput, accountStorePath: source ?? resolve(canonicalOutput, "accounts.json"), reuseOutput };
}

const ownedChildClosures = new WeakMap();
const childExited = child => child.exitCode !== null || child.signalCode !== null;
function waitForChildExit(child, timeoutMs) {
  if (childExited(child)) return Promise.resolve(true);
  return new Promise(resolveWait => {
    let timer;
    const done = exited => { clearTimeout(timer); child.removeListener("exit", exit); child.removeListener("close", exit); resolveWait(exited); };
    const exit = () => done(true);
    child.once("exit", exit); child.once("close", exit);
    timer = setTimeout(() => done(false), timeoutMs);
    if (childExited(child)) done(true);
  });
}

/** Only for children this helper/caller spawned. Never accepts a PID. */
export function closeOwnedGatewayChild(child, { timeoutMs = 7500, killWaitMs = 2000 } = {}) {
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 8000 || !Number.isInteger(killWaitMs) || killWaitMs < 1 || killWaitMs > 2000) return Promise.reject(new Error("Invalid bounded local shutdown window."));
  if (!child || !Number.isSafeInteger(child.pid) || childExited(child)) return Promise.resolve();
  const previous = ownedChildClosures.get(child);
  if (previous) return previous;
  const closing = (async () => {
    if (child.stdin && !child.stdin.destroyed && child.stdin.writable) {
      const ignorePipeClosure = () => {};
      child.stdin.on("error", ignorePipeClosure);
      child.stdin.once("close", () => child.stdin.removeListener("error", ignorePipeClosure));
      try { child.stdin.end("shutdown\n"); } catch { /* it may have exited between the check and write */ }
      if (await waitForChildExit(child, timeoutMs)) return;
    }
    // Normal guild/conquest clock settlement gets the full bounded window.
    // Only a broken/no-stdin or timed-out owned child reaches TerminateProcess.
    if (!childExited(child)) child.kill();
    if (!await waitForChildExit(child, killWaitMs)) throw new Error("The owned local gateway did not exit within its shutdown window.");
  })();
  ownedChildClosures.set(child, closing);
  return closing;
}

export async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

/** A private local account store and our own child only; never targets a running world. */
export async function startLocalGateway({ executable, prefix = "companion", reuse, companionPolicy, contentProfile = R16_GAMEPLAY_BASELINE.contentProfile, outputRoot, supplyConfigDirectory } = {}) {
  if (!["platinum_fiveclass", "platinum_176", "crystal_full"].includes(contentProfile)) throw new Error("Choose an explicit supported local content profile.");
  executable = await resolveGatewayExecutable(executable);
  const runId = randomUUID();
  const realmId = reuse?.realmId ?? `ai-${prefix}-${runId}`;
  const paths = await prepareLocalGatewayPaths({ prefix, runId, reuse, outputRoot });
  const { output, accountStorePath } = paths;
  let policyPath;
  if (companionPolicy) {
    policyPath = resolve(output, "companion-policy.json");
    await writeFile(policyPath, JSON.stringify({ ...companionPolicy, realmId }), { flag: "wx", mode: 0o600 });
  }
  const supplyEnvironment = companionPolicy ? await localSupplyServerEnvironment({
    directory: supplyConfigDirectory ?? paths.reuseOutput ?? output,
    allowedRoot: paths.outputRoot, realmId, worldEpoch: companionPolicy.worldEpoch,
  }) : {};
  const webPort = await freePort();
  const child = spawn(executable, [], { cwd: repoRoot, windowsHide: true, stdio: ["pipe", "pipe", "pipe"], env: isolatedEnvironment({
    MIR2_GATEWAY_TCP_ADDR: "127.0.0.1:0", MIR2_GATEWAY_WEB_ADDR: `127.0.0.1:${webPort}`,
    MIR2_ACCOUNT_STORE_BACKEND: "file", MIR2_ACCOUNT_STORE_PATH: accountStorePath,
    MIR2_SAVE_RECOVERY_DIR: resolve(output, "save-recovery"), MIR2_SAVE_RECOVERY_MAC_KEY: randomBytes(32).toString("hex"),
    MIR2_IDENTITY_SESSION_SECRET: randomBytes(32).toString("hex"), MIR2_IDENTITY_RECOVERY_PEPPER: randomBytes(32).toString("hex"),
    MIR2_REALM_ID: realmId, MIR2_CONTENT_PROFILE: contentProfile, MIR2_QUEST_CADENCE: R16_GAMEPLAY_BASELINE.questCadence,
    MIR2_RUNTIME_ENV: "test", MIR2_ZONE_ROUTING_MODE: "single",
    MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "0", MIR2_GATEWAY_STDIN_SHUTDOWN: "1", MIR2_AI_LIVE_ENABLED: "0",
    // Canonical R16 has typed shared groups; no legacy feature switch is required.
    MIR2_GATEWAY_TOKIO_WORKER_THREADS: "2",
    ...(policyPath ? { MIR2_COMPANION_POLICY_PATH: policyPath } : {}),
    ...supplyEnvironment,
  }) });
  let diagnostics = "";
  let spawnError = false;
  child.on("error", () => { spawnError = true; });
  const capture = chunk => { diagnostics = (diagnostics + chunk.toString()).slice(-64_000); };
  child.stdout.on("data", capture); child.stderr.on("data", capture);
  const close = () => closeOwnedGatewayChild(child);
  try {
    const deadline = Date.now() + 60_000;
    let healthy = false;
    while (Date.now() < deadline) {
      if (spawnError || child.exitCode !== null) throw new Error("Isolated gateway did not start.");
      try { healthy = (await fetch(`http://127.0.0.1:${webPort}/health`, { signal: AbortSignal.timeout(1_000) })).ok; } catch { /* wait */ }
      if (healthy) break;
      await sleep(200);
    }
    if (!healthy) throw new Error("Isolated gateway health check timed out.");
    return { executable, runId, realmId, output, outputRoot: paths.outputRoot, contentProfile, supplyServerConfigured: Boolean(companionPolicy),
      gameplayBaseline: { ...R16_GAMEPLAY_BASELINE, matchesR16Profile: contentProfile === R16_GAMEPLAY_BASELINE.contentProfile },
      url: `ws://127.0.0.1:${webPort}/ws`, close,
      saveDiagnostics: () => writeFile(resolve(output, "gateway.log"), diagnostics) };
  } catch (error) { await close(); throw error; }
}
