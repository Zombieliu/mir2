// A private ordinary-account world for the R16 browser playtest. No QA assets.
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { createInterface } from "node:readline";
import { createWriteStream } from "node:fs";
import { mkdir, readFile, copyFile, writeFile, symlink, realpath } from "node:fs/promises";
import { resolve, isAbsolute } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "../../ai-player/scripts/companion-trial-fixture.mjs";
import { appRoot, repoRoot, freePort, isolatedEnvironment, closeOwnedGatewayChild } from "../../ai-player/scripts/local-gateway.mjs";

const options = {};
for (let index = 2; index < process.argv.length; index += 2) {
  const key = { "--gateway-exe": "gateway", "--out": "output" }[process.argv[index]], value = process.argv[index + 1];
  if (!key || options[key] || !value || !isAbsolute(value)) throw new Error("Provide --gateway-exe and --out as absolute paths.");
  options[key] = resolve(value);
}
if (!options.gateway || !options.output || process.versions.node !== "22.18.0") throw new Error("Use the bundled Node.js 22.18.0 and explicit local paths.");
const webRoot = resolve(appRoot, "../web");
let fixture, web, logs, closing = false, actor;
const root = resolve(options.output, `web-${randomUUID().slice(0, 8)}`);
await mkdir(root, { recursive: true });
await mkdir(resolve(root, "bin"));
const gateway = resolve(root, "bin/mir2-gateway.exe");
await copyFile(options.gateway, gateway);
let queue = Promise.resolve();
async function closeWeb() {
  const child = web; web = null;
  if (child?.exitCode === null) { child.kill(); await Promise.race([new Promise(done => child.once("exit", done)), sleep(8000)]); }
  logs?.end();
}
async function shutdown() {
  if (closing) return;
  closing = true;
  try { await closeWeb(); }
  finally { await fixture?.close(); }
  console.log(JSON.stringify({ type: "webPlaytestStopped", dataPreserved: true, output: root }));
}
async function startWeb() {
  await closeWeb();
  const port = await freePort(), configName = `tsconfig.r16-${root.split(/[\\/]/).at(-1)}.local.json`;
  const distName = `.next-r16-${root.split(/[\\/]/).at(-1)}`, cache = resolve(root, "next-dev");
  await mkdir(cache, { recursive: true });
  try { await symlink(resolve(webRoot, "node_modules"), resolve(root, "node_modules"), process.platform === "win32" ? "junction" : "dir"); }
  catch (error) { if (error.code !== "EEXIST" || (await realpath(resolve(root, "node_modules"))).toLowerCase() !== (await realpath(resolve(webRoot, "node_modules"))).toLowerCase()) throw error; }
  try { await symlink(cache, resolve(webRoot, distName), process.platform === "win32" ? "junction" : "dir"); }
  catch (error) { if (error.code !== "EEXIST" || (await realpath(resolve(webRoot, distName))).toLowerCase() !== (await realpath(cache)).toLowerCase()) throw error; }
  const originalTsconfig = await readFile(resolve(webRoot, "tsconfig.json"));
  try { await writeFile(resolve(webRoot, configName), originalTsconfig, { flag: "wx" }); } catch (error) { if (error.code !== "EEXIST") throw error; }
  logs = createWriteStream(resolve(root, "next.log"), { flags: "a" });
  web = spawn(process.execPath, [resolve(webRoot, "node_modules/next/dist/bin/next"), "dev", "--webpack", "--hostname", "127.0.0.1", "--port", String(port)], {
    cwd: webRoot, windowsHide: true, stdio: ["ignore", "pipe", "pipe"], env: isolatedEnvironment({
      NODE_ENV: "development", NODE_OPTIONS: "--max-old-space-size=6144", NEXT_TELEMETRY_DISABLED: "1",
      MIR2_NEXT_STANDALONE: "1", MIR2_NEXT_DIST_DIR: distName, MIR2_NEXT_TSCONFIG_PATH: configName,
      NEXT_PUBLIC_MIR2_GATEWAY_WS_URL: fixture.gateway.url, MIR2_GATEWAY_HTTP_URL: fixture.gateway.url.replace(/^ws:/, "http:").replace(/\/ws$/, ""),
      NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: "crystal",
    }),
  });
  web.stdout.pipe(logs, { end: false }); web.stderr.pipe(logs, { end: false });
  const origin = `http://127.0.0.1:${port}`;
  await writeFile(resolve(root, "playtest-private.local.json"), JSON.stringify({ schema: 1, origin, gatewayUrl: fixture.gateway.url, realmId: fixture.gateway.realmId,
    owner: fixture.owner, actor: fixture.actor, stranger: fixture.stranger, fixtureOutput: fixture.output, driverPid: process.pid, webPid: web.pid,
    profileAPath: fixture.profileAPath }, null, 2), { mode: 0o600 });
  await writeFile(resolve(root, "试玩账号.txt"), `Numeron · Web R16 本机试玩\n\n地址：${origin}\n账号：${fixture.owner.accountId}\n密码：${fixture.owner.password}\n角色：${fixture.owner.characterName}\n\n正常登录后选择角色进入比奇。世界与账号保存在本目录；不要公开分享本目录的私密文件。\n`, { mode: 0o600 });
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    if (web.exitCode !== null) throw new Error("The local Web server exited. See the private Next log.");
    try { const response = await fetch(origin, { signal: AbortSignal.timeout(10_000) }); if (response.ok) { console.log(JSON.stringify({ type: "webPlaytestReady", origin, gatewayUrl: fixture.gateway.url, output: root, webPid: web.pid, driverPid: process.pid, commands: ["status", "restartWeb", "actor", "stopActor", "stop"] })); return; } }
    catch { /* wait for the ordinary compiler */ }
    await sleep(1000);
  }
  throw new Error("The local Web server did not become ready within its startup window.");
}
try {
  fixture = await createTrialFixture({ gatewayExecutable: gateway, outputRoot: root, onEvent: event => { if (event.type === "fixtureStage") console.log(JSON.stringify(event)); } });
  await startWeb();
  const lines = createInterface({ input: process.stdin, terminal: false });
  lines.on("line", command => {
    queue = queue.then(async () => {
      switch (command.trim()) {
        case "status": console.log(JSON.stringify({ type: "webPlaytestStatus", root, webAlive: web?.exitCode === null, actorPid: actor?.pid ?? null })); break;
        case "restartWeb": await startWeb(); break;
        case "actor": if (!actor) { actor = await fixture.startActor(fixture.profileA, { run: true }); console.log(JSON.stringify({ type: "webPlaytestActorReady", pid: actor.pid })); } break;
        case "stopActor": await fixture.stopActor(); actor = null; break;
        case "stop": lines.close(); await shutdown(); break;
      }
    }).catch(() => console.log(JSON.stringify({ type: "webPlaytestCommandUnavailable" })));
  });
  lines.on("close", () => { queue.then(shutdown).catch(() => { process.exitCode = 1; }); });
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { lines.close(); queue.then(shutdown).catch(() => { process.exitCode = 1; }); });
} catch {
  console.error("Local Web playtest startup stopped; existing worlds and source changes were retained.");
  await shutdown(); process.exitCode = 1;
}
