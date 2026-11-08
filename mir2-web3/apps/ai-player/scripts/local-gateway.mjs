import { spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createReadStream } from "node:fs";
import { access, mkdir, realpath, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { relative, resolve, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";

export const appRoot = fileURLToPath(new URL("../", import.meta.url));
export const repoRoot = resolve(appRoot, "../..");

export async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

export function isolatedEnvironment(overrides) {
  const keys = new Set(["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "COMSPEC", "LANG", "LC_ALL", "TZ"]);
  return { ...Object.fromEntries(Object.entries(process.env).filter(([key]) => keys.has(key.toUpperCase()))), ...overrides };
}

export async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

/** A private local account store and our own child only; never targets a running world. */
export async function startLocalGateway({ executable, prefix = "companion", reuse, companionPolicy } = {}) {
  executable ??= resolve(repoRoot, `target-ai-player/debug/mir2-gateway${process.platform === "win32" ? ".exe" : ""}`);
  await access(executable);
  const runId = randomUUID();
  const realmId = reuse?.realmId ?? `ai-${prefix}-${runId}`;
  const output = resolve(appRoot, ".data", `${prefix}-${runId}`);
  await mkdir(output, { recursive: true });
  let accountStorePath = resolve(output, "accounts.json");
  if (reuse) {
    const source = await realpath(resolve(reuse.output, "accounts.json"));
    const inside = relative(await realpath(resolve(appRoot, ".data")), source);
    if (!inside || inside.startsWith("..") || isAbsolute(inside) || typeof realmId !== "string" || !/^ai-[A-Za-z0-9-]+$/.test(realmId)) throw new Error("Only this helper's private local account store may be reused.");
    accountStorePath = source;
  }
  let policyPath;
  if (companionPolicy) {
    policyPath = resolve(output, "companion-policy.json");
    await writeFile(policyPath, JSON.stringify({ ...companionPolicy, realmId }), { flag: "wx", mode: 0o600 });
  }
  const webPort = await freePort();
  const child = spawn(executable, [], { cwd: repoRoot, windowsHide: true, stdio: ["ignore", "pipe", "pipe"], env: isolatedEnvironment({
    MIR2_GATEWAY_TCP_ADDR: "127.0.0.1:0", MIR2_GATEWAY_WEB_ADDR: `127.0.0.1:${webPort}`,
    MIR2_ACCOUNT_STORE_BACKEND: "file", MIR2_ACCOUNT_STORE_PATH: accountStorePath,
    MIR2_SAVE_RECOVERY_DIR: resolve(output, "save-recovery"), MIR2_SAVE_RECOVERY_MAC_KEY: randomBytes(32).toString("hex"),
    MIR2_IDENTITY_SESSION_SECRET: randomBytes(32).toString("hex"), MIR2_IDENTITY_RECOVERY_PEPPER: randomBytes(32).toString("hex"),
    MIR2_REALM_ID: realmId, MIR2_CONTENT_PROFILE: "crystal_full", MIR2_RUNTIME_ENV: "test", MIR2_ZONE_ROUTING_MODE: "single",
    MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "0", MIR2_AI_LIVE_ENABLED: "0",
    MIR2_SHARED_PARTY_ENABLED: "1", MIR2_GATEWAY_TOKIO_WORKER_THREADS: "2",
    ...(policyPath ? { MIR2_COMPANION_POLICY_PATH: policyPath } : {}),
  }) });
  let diagnostics = "";
  let spawnError = false;
  child.on("error", () => { spawnError = true; });
  const capture = chunk => { diagnostics = (diagnostics + chunk.toString()).slice(-64_000); };
  child.stdout.on("data", capture); child.stderr.on("data", capture);
  const close = async () => {
    if (child.exitCode === null) {
      const exited = new Promise(resolve => child.once("exit", resolve));
      child.kill();
      await Promise.race([exited, sleep(5_000)]);
    }
  };
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
    return { executable, runId, realmId, output, url: `ws://127.0.0.1:${webPort}/ws`, close,
      saveDiagnostics: () => writeFile(resolve(output, "gateway.log"), diagnostics) };
  } catch (error) { await close(); throw error; }
}
