/** Persistent loopback-only R16 Web delivery. Importing this module is inert.
 * First initialization uses ordinary accounts; incomplete state is never reset.
 * Model credentials are read only by the isolated Actor worker, never by Next
 * or Gateway. No HTTP administration endpoint is installed.
 */
import { spawn } from "node:child_process";
import { randomUUID, timingSafeEqual } from "node:crypto";
import { createWriteStream } from "node:fs";
import { lstat, mkdir, open, readFile, readdir, realpath, rename } from "node:fs/promises";
import { createConnection, createServer } from "node:net";
import { basename, dirname, parse, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { newPlayableState, parsePlayableArguments, playableActorProfile, playableActorWorkerEnvironment, playableBaseEnvironment, playableControlAddress,
  playableGatewayEnvironment, playableNextEnvironment, playablePaths, playablePolicy, SERVICE_PORTS, validatePlayableState } from "./r16-playable-service-config.mjs";

const self = fileURLToPath(import.meta.url);
const projectRoot = fileURLToPath(new URL("../../../", import.meta.url));
const alive = child => Boolean(child && child.exitCode === null && child.signalCode === null);
const error = code => Object.assign(new Error("Playable service stopped without resetting private state."), { code });
const print = value => process.stdout.write(JSON.stringify(value) + "\n");

async function regularFile(path, { nonempty = true } = {}) {
  const metadata = await lstat(path);
  if (!metadata.isFile() || metadata.isSymbolicLink() || nonempty && !metadata.size) throw error("PRIVATE_FILE_UNAVAILABLE");
  return metadata;
}
async function privateDirectory(path) {
  const metadata = await lstat(path);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) throw error("PRIVATE_DIRECTORY_UNAVAILABLE");
}
async function privateJson(path) {
  const metadata = await regularFile(path);
  if (metadata.size > 64 * 1024) throw error("PRIVATE_STATE_UNAVAILABLE");
  return JSON.parse(await readFile(path, "utf8"));
}
async function writeExclusiveJson(path, value) {
  const file = await open(path, "wx", 0o600);
  try { await file.writeFile(JSON.stringify(value, null, 2) + "\n"); await file.sync(); }
  finally { await file.close(); }
}

/** Tested with temporary synthetic directories only; no process/network work. */
export async function preparePlayableState(dataDir) {
  const paths = playablePaths(dataDir), names = await readdir(dataDir);
  if (!names.includes("state.local.json")) {
    if (names.length) throw error("PRIVATE_STATE_MISSING");
    const state = validatePlayableState(newPlayableState(), { allowInitializing: true }); await writeExclusiveJson(paths.state, state); return { state, firstRun: true };
  }
  if (names.some(name => name.startsWith("state.local.json.pending-"))) throw error("PRIVATE_STATE_PUBLICATION_INCOMPLETE");
  const state = validatePlayableState(await privateJson(paths.state));
  await regularFile(paths.accounts); await privateDirectory(paths.recovery); await privateDirectory(paths.supply);
  await regularFile(resolve(paths.supply, "supply-server.local.json"));
  if (JSON.stringify(await privateJson(paths.policy)) !== JSON.stringify(playablePolicy(state))) throw error("PRIVATE_POLICY_MISMATCH");
  await verifyActorStores(state, dataDir);
  return { state, firstRun: false };
}

async function verifyActorStores(state, dataDir) {
  const paths = playablePaths(dataDir);
  const files = [paths.actorMemory, paths.actorMemory + ".supply.sqlite", paths.actorMemory + ".life.sqlite", paths.actorMemory + ".gifts.sqlite", paths.experience];
  let present = 0;
  for (const path of files) {
    try { await regularFile(path); present++; }
    catch (cause) { if (cause.code !== "ENOENT") throw cause; }
  }
  if (state.actorStoreInitialized && present !== files.length || present !== 0 && present !== files.length) throw error("PRIVATE_ACTOR_STORE_INCOMPLETE");
}

async function updateState(dataDir, previous, next) {
  validatePlayableState(next);
  const path = playablePaths(dataDir).state;
  if (JSON.stringify(await privateJson(path)) !== JSON.stringify(previous)) throw error("PRIVATE_STATE_CHANGED");
  const pending = path + ".pending-" + randomUUID(); await writeExclusiveJson(pending, next); await rename(pending, path);
  return next;
}

function reservePort(port) {
  return new Promise((resolvePort, reject) => {
    const server = createServer(socket => socket.destroy());
    server.once("error", () => reject(error("LOCAL_PORT_IN_USE")));
    server.listen({ host: "127.0.0.1", port, exclusive: true }, () => resolvePort(() => new Promise(done => server.close(done))));
  });
}
function waitExit(child, timeoutMs) {
  if (!alive(child)) return Promise.resolve(true);
  return new Promise(done => {
    const finish = value => { clearTimeout(timer); child.removeListener("exit", exited); done(value); };
    const exited = () => finish(true), timer = setTimeout(() => finish(false), timeoutMs);
    child.once("exit", exited); if (!alive(child)) finish(true);
  });
}
async function stopOwnedChild(child, gateway = false) {
  if (!alive(child)) return;
  if (gateway) { child.stdin.on("error", () => {}); child.stdin.end("shutdown\n"); }
  else child.kill();
  if (await waitExit(child, gateway ? 10_000 : 8000)) return;
  child.kill(); await waitExit(child, 2000); throw error("OWNED_CHILD_SHUTDOWN_UNCONFIRMED");
}
async function waitHttp(child, url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!alive(child)) throw error("OWNED_CHILD_STARTUP_FAILED");
    try { const response = await fetch(url, { signal: AbortSignal.timeout(1000), redirect: "error" }); if (response.ok && alive(child)) return; }
    catch { /* Only our already-spawned child's reserved loopback endpoint. */ }
    await sleep(200);
  }
  throw error("OWNED_CHILD_STARTUP_TIMEOUT");
}
function authenticatedKey(value, expected) {
  return typeof value === "string" && typeof expected === "string" && value.length === 64 && expected.length === 64
    && timingSafeEqual(Buffer.from(value), Buffer.from(expected));
}

export async function sendPlayableControl(options) {
  const dataDir = await realpath(options.dataDir), state = validatePlayableState(await privateJson(playablePaths(dataDir).state), { allowInitializing: true });
  return new Promise((done, reject) => {
    const socket = createConnection(playableControlAddress(dataDir)); let buffer = "";
    socket.setTimeout(options.command === "status" ? 5000 : 65_000);
    socket.on("connect", () => socket.write(JSON.stringify({ key: state.keys.controlKey, command: options.command }) + "\n"));
    socket.on("data", chunk => { buffer += chunk; if (buffer.length > 4096) socket.destroy(error("CONTROL_UNAVAILABLE")); });
    socket.on("timeout", () => socket.destroy(error("CONTROL_TIMEOUT")));
    socket.once("error", () => reject(error("CONTROL_UNAVAILABLE")));
    socket.once("end", () => { try { const result = JSON.parse(buffer); if (!result.ok) throw error("CONTROL_UNAVAILABLE"); done(result.status); } catch { reject(error("CONTROL_UNAVAILABLE")); } });
  });
}

/** Only the Actor worker imports the credential reader and model network code. */
async function runActorWorker() {
  if (!process.connected) throw error("ACTOR_IPC_REQUIRED");
  const { ResidentActor } = await import("../../ai-player/src/resident-actor.mjs");
  const { loadLocalModelSettings } = await import("../../ai-player/src/local-model-settings.mjs");
  const { loadLocalModelNetwork, modelProcessEnvironment } = await import("../../ai-player/src/model-runtime-environment.mjs");
  let actor, running, stopping, starting = Promise.resolve(), started = false, stopRequested = false;
  const stop = () => stopping ??= (async () => {
    stopRequested = true; await starting.catch(() => {});
    const saved = await actor?.stop({ save: true }); await running?.catch(() => {});
    if (process.connected) process.send({ type: "actorStopped", saved: saved === true }, () => process.disconnect());
  })();
  process.on("message", message => {
    if (message?.type === "stop") { stop().catch(() => process.exitCode = 1); return; }
    if (message?.type !== "start" || started) return; started = true;
    starting = (async () => {
      const profile = message.profile, environment = { ...playableBaseEnvironment(), MIR2_COMPANION_ARCHIVE_KEY: message.archiveKey };
      let local = { provider: "rules" };
      if (message.modelSettingsBundle) local = await loadLocalModelSettings({ bundleDir: message.modelSettingsBundle, env: playableBaseEnvironment() });
      if (local.provider === "openai") {
        Object.assign(profile.runtimeService, { provider: "openai", model: local.model, credentialRef: "OPENAI_API_KEY", promptVersion: "alo-objective-v2", maxCallsPerHour: local.maxCallsPerHour });
        Object.assign(environment, modelProcessEnvironment(local.apiKey, await loadLocalModelNetwork({ bundleDir: message.modelSettingsBundle })));
        local.apiKey = null;
      }
      actor = new ResidentActor(profile, { environment, report() {} });
      await actor.start(message.identity);
      if (stopRequested) return;
      process.send({ type: "actorReady", modelEnabled: profile.runtimeService.provider === "openai" });
      running = (async () => { while (!stopping) await actor.run({ durationMs: 86_400_000 }); })();
      running.catch(async () => { await stop(); process.exitCode = 1; });
    })();
    starting.catch(async () => {
      if (process.connected) process.send({ type: "actorFailed" });
      await stop().catch(() => {}); process.exitCode = 1;
    });
  });
  process.once("disconnect", () => { stop().catch(() => { process.exitCode = 1; }); });
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => stop().catch(() => { process.exitCode = 1; }));
}

export async function runPlayableService(options) {
  if (process.versions.node !== "22.18.0") throw error("PINNED_NODE_REQUIRED");
  const [gatewayExe, serverEntry, webRoot, nodeExe] = await Promise.all([options.gatewayExe, options.serverEntry, options.webRoot, options.nodeExe].map(path => realpath(path)));
  for (const path of [gatewayExe, serverEntry, nodeExe]) await regularFile(path);
  if (basename(serverEntry) !== "server.js" || dirname(serverEntry).toLowerCase() !== webRoot.toLowerCase()) throw error("STANDALONE_ENTRY_REQUIRED");
  if (resolve(options.dataDir) === parse(resolve(options.dataDir)).root) throw error("DEDICATED_PRIVATE_DIRECTORY_REQUIRED");
  await mkdir(options.dataDir, { recursive: true }); const dataDir = await realpath(options.dataDir), paths = playablePaths(dataDir);
  let state, gateway, next, actor, actorModelEnabled = false, stopping, stopRequested = false, phase = "starting", queue = Promise.resolve();
  let releaseGateway, releaseWeb, startupDone, resolveStartup, resolveDone;
  const done = new Promise(resolveDonePromise => { resolveDone = resolveDonePromise; });
  startupDone = new Promise(resolveStartupPromise => { resolveStartup = resolveStartupPromise; });
  const streams = [];
  const status = () => ({ type: "r16PlayableStatus", phase, servicePid: process.pid, gatewayPid: alive(gateway) ? gateway.pid : null,
    webPid: alive(next) ? next.pid : null, actorPid: alive(actor) ? actor.pid : null, actorModelEnabled,
    origin: "http://127.0.0.1:19041", gatewayUrl: "ws://127.0.0.1:19040/ws", dataDir });
  const requireRunning = () => { if (stopRequested) throw error("STOP_REQUESTED"); };
  const loggedChild = (executable, args, environment, cwd, name) => {
    const output = createWriteStream(resolve(paths.logs, name + ".log"), { flags: "a", mode: 0o600 }); streams.push(output); output.on("error", () => {});
    const child = spawn(executable, args, { cwd, windowsHide: true, stdio: ["pipe", "pipe", "pipe"], env: environment });
    child.on("error", () => {}); child.stdout.pipe(output, { end: false }); child.stderr.pipe(output, { end: false }); return child;
  };
  const stopActor = async () => {
    const child = actor; actor = null; actorModelEnabled = false;
    if (!child) return true;
    if (!alive(child)) { if (child.r16Saved !== true) throw error("ACTOR_SAVE_UNCONFIRMED"); return true; }
    let saved = false;
    child.on("message", message => { if (message?.type === "actorStopped") saved = message.saved === true; });
    child.send({ type: "stop" }, () => {});
    if (!await waitExit(child, 45_000)) { child.kill(); await waitExit(child, 2000); throw error("ACTOR_SAVE_UNCONFIRMED"); }
    if (!saved) throw error("ACTOR_SAVE_UNCONFIRMED"); return true;
  };
  const startActor = async () => {
    if (!options.withAlo) throw error("ACTOR_NOT_ENABLED"); requireRunning();
    await verifyActorStores(state, dataDir);
    const child = spawn(nodeExe, [self, "--actor-worker"], { cwd: projectRoot, windowsHide: true, stdio: ["ignore", "ignore", "ignore", "ipc"], env: playableActorWorkerEnvironment() });
    actor = child;
    child.on("message", message => { if (message?.type === "actorStopped") child.r16Saved = message.saved === true; });
    child.once("exit", () => { if (actor === child && !stopRequested && phase === "running") { process.exitCode = 1; signal(); } });
    await new Promise((ready, reject) => {
      const timer = setTimeout(() => reject(error("ACTOR_STARTUP_TIMEOUT")), 90_000);
      const finish = fn => value => { clearTimeout(timer); child.removeListener("exit", exited); child.removeListener("message", message); fn(value); };
      const exited = () => finish(reject)(error("ACTOR_STARTUP_FAILED"));
      const message = value => { if (value?.type === "actorReady") { actorModelEnabled = value.modelEnabled === true; finish(ready)(); }
        else if (value?.type === "actorFailed") finish(reject)(error("ACTOR_STARTUP_FAILED")); };
      child.once("error", () => finish(reject)(error("ACTOR_STARTUP_FAILED"))); child.once("exit", exited); child.on("message", message);
      child.send({ type: "start", profile: playableActorProfile(state, dataDir), identity: { accountId: state.actor.accountId, password: state.actor.password },
        archiveKey: state.keys.archiveKey, modelSettingsBundle: options.modelSettingsBundle }, cause => { if (cause) finish(reject)(error("ACTOR_STARTUP_FAILED")); });
    });
    await verifyActorStores({ ...state, actorStoreInitialized: true }, dataDir);
    if (!state.actorStoreInitialized) state = await updateState(dataDir, state, { ...state, actorStoreInitialized: true });
  };
  const stop = () => stopping ??= (async () => {
    stopRequested = true; await startupDone; phase = "stopping"; let saved = true;
    try { await stopActor(); } catch { saved = false; }
    try { await stopOwnedChild(next); } catch { saved = false; } next = null;
    try { await stopOwnedChild(gateway, true); } catch { saved = false; } gateway = null;
    await releaseGateway?.(); releaseGateway = null; await releaseWeb?.(); releaseWeb = null;
    phase = saved ? "stopped" : "stop-unconfirmed"; if (!saved) process.exitCode = 1;
    print({ type: "r16PlayableStopped", saved, dataPreserved: true }); resolveDone(); return status();
  })();
  const enqueue = operation => { const result = queue.then(operation); queue = result.catch(() => {}); return result; };
  const control = createServer(socket => {
    let input = "", answered = false; socket.setTimeout(65_000, () => socket.destroy());
    socket.on("error", () => {});
    socket.on("data", chunk => {
      if (answered) return; input += chunk;
      if (input.length > 1024) { answered = true; socket.destroy(); return; }
      if (!input.includes("\n")) return; answered = true;
      (async () => {
        const request = JSON.parse(input.trim());
        if (!authenticatedKey(request.key, state?.keys.controlKey) || !["status", "stop", "restartActor"].includes(request.command)) throw error("CONTROL_UNAVAILABLE");
        const response = request.command === "status" ? status() : request.command === "stop" ? await enqueue(stop)
          : await enqueue(async () => { if (phase !== "running") throw error("CONTROL_UNAVAILABLE"); await stopActor(); await startActor(); return status(); });
        socket.end(JSON.stringify({ ok: true, status: response }) + "\n");
      })().catch(() => socket.end(JSON.stringify({ ok: false }) + "\n"));
    });
  });
  let lines;
  const signal = () => { stopRequested = true; enqueue(stop).catch(() => { process.exitCode = 1; }); };
  try {
    await new Promise((ready, reject) => { control.once("error", () => reject(error("LOCAL_INSTANCE_ALREADY_RUNNING"))); control.listen(playableControlAddress(dataDir), ready); });
    for (const name of ["SIGINT", "SIGTERM"]) process.once(name, signal);
    releaseGateway = await reservePort(SERVICE_PORTS.gateway); releaseWeb = await reservePort(SERVICE_PORTS.web);
    const prepared = await preparePlayableState(dataDir); state = prepared.state;
    if (prepared.firstRun) for (const path of [paths.recovery, paths.supply, paths.memory, paths.logs]) await mkdir(path);
    else { await privateDirectory(paths.logs); await privateDirectory(paths.memory); }
    const { localSupplyServerEnvironment } = await import("../../ai-player/src/local-supply-server-config.mjs");
    const { GatewayClient } = await import("../../ai-player/src/gateway-client.mjs");
    const connect = async () => {
      const client = new GatewayClient({ url: "ws://127.0.0.1:19040/ws", realmId: state.world.realmId, timeoutMs: 25_000 });
      try { const realm = await client.connect(); if (realm.profileId !== "platinum_fiveclass" || realm.profileVersion !== 1) throw error("R16_CONTENT_PROFILE_REQUIRED"); return client; }
      catch (cause) { await client.close(); throw cause; }
    };
    if (prepared.firstRun) {
      phase = "initializing"; requireRunning(); await releaseGateway(); releaseGateway = null;
      gateway = loggedChild(gatewayExe, [], playableGatewayEnvironment(state, dataDir), projectRoot, "gateway-bootstrap");
      await waitHttp(gateway, "http://127.0.0.1:19040/health", 60_000);
      for (const role of ["owner", "actor"]) {
        requireRunning(); const identity = state[role], client = await connect();
        try { await client.createAccount(identity.accountId, identity.password); await client.login(identity.accountId, identity.password);
          const character = await client.createCharacter(identity.characterName); identity.characterIndex = character.index;
          await client.startGame(character.index); if (!await client.logout()) throw error("BOOTSTRAP_SAVE_UNCONFIRMED");
        } finally { await client.close(); }
      }
      await stopOwnedChild(gateway, true); gateway = null;
      const completed = { ...state, phase: "ready" }; validatePlayableState(completed);
      await writeExclusiveJson(paths.policy, playablePolicy(completed));
      await localSupplyServerEnvironment({ directory: paths.supply, allowedRoot: dataDir, realmId: state.world.realmId, worldEpoch: state.world.worldEpoch });
      const previous = await privateJson(paths.state); state = await updateState(dataDir, previous, completed);
      releaseGateway = await reservePort(SERVICE_PORTS.gateway);
    }
    requireRunning();
    const supply = await localSupplyServerEnvironment({ directory: paths.supply, allowedRoot: dataDir, realmId: state.world.realmId, worldEpoch: state.world.worldEpoch });
    await releaseGateway(); releaseGateway = null;
    gateway = loggedChild(gatewayExe, [], playableGatewayEnvironment(state, dataDir, { enrolled: true, supply }), projectRoot, "gateway");
    await waitHttp(gateway, "http://127.0.0.1:19040/health", 60_000);
    const probe = await connect();
    try { if (!probe.features.companionControl || !probe.features.sharedParty || probe.worldEpoch !== state.world.worldEpoch) throw error("COMPANION_ENROLLMENT_REQUIRED"); }
    finally { await probe.close(); }
    requireRunning();
    if (options.withAlo) await startActor();
    requireRunning(); await releaseWeb(); releaseWeb = null;
    const nextNetworkBootstrap = fileURLToPath(new URL("./r16-next-network.mjs", import.meta.url));
    await regularFile(nextNetworkBootstrap);
    next = loggedChild(nodeExe, ["--import", pathToFileURL(nextNetworkBootstrap).href, serverEntry], playableNextEnvironment(state), webRoot, "next"); next.stdin.end();
    await waitHttp(next, "http://127.0.0.1:19041/health", 90_000); requireRunning(); phase = "running";
    // This machine-local login handoff is outside Next's public tree. Never
    // print credentials or copy this file into the distributable allowlist.
    try {
      const guide = await open(resolve(dataDir, "试玩账号.txt"), "wx", 0o600);
      try {
        await guide.writeFile(["Numeron R16 Web 本地试玩账号", "", "地址：http://127.0.0.1:19041/",
          "账号：" + state.owner.accountId, "初始密码：" + state.owner.password, "角色：" + state.owner.characterName, "",
          "请使用这名角色体验阿洛；自行新建的账号尚未绑定这名伙伴。", "修改密码后请使用你设置的新密码；此处保留初始信息。",
          "阿洛入口：进城后右侧「伙伴 Companion」→「邀请组队」。在城中心（288,616）进行教学更安全。",
          "固定教学成功后核对培养版本；正常 Stop 再 Start，角色和记忆继续保留。",
          "默认启动使用规则；本机已有模型设置时，用 Start-With-Local-Model.cmd 启动真实模型。",
          "补给授权最多50游戏金币、1瓶血药；阿洛须有自己的金币。可通过原组队和交易界面协作。", "",
          "本文件含本地游戏登录信息，请留在 private/，不要分享或随试玩包发布。", ""].join("\r\n"), "utf8");
        await guide.sync();
      } finally { await guide.close(); }
    } catch (cause) { if (cause.code !== "EEXIST") throw cause; }
    for (const child of [gateway, next]) child.once("exit", () => { if (!stopRequested && phase === "running") { process.exitCode = 1; signal(); } });
    resolveStartup(); print({ ...status(), type: "r16PlayableReady", firstRun: prepared.firstRun, commands: ["status", "restartActor", "stop"] });
    lines = createInterface({ input: process.stdin, terminal: false });
    lines.on("line", command => {
      const value = command.trim();
      if (value === "status") print(status());
      else if (value === "stop") signal();
      else if (value === "restartActor") enqueue(async () => { await stopActor(); await startActor(); print(status()); }).catch(() => print({ type: "r16PlayableCommandUnavailable" }));
    });
    // EOF is normal for a background local service. Only explicit stop/signals
    // or the private authenticated pipe close the owned children.
    lines.on("close", () => {});
    await done;
  } catch (cause) {
    process.exitCode = 1; print({ type: "r16PlayableFailed", phase, code: /^[A-Z_]+$/.test(cause?.code ?? "") ? cause.code : "STARTUP_FAILED", dataPreserved: true });
    resolveStartup(); await stop();
  } finally {
    lines?.close(); for (const name of ["SIGINT", "SIGTERM"]) process.removeListener(name, signal);
    for (const stream of streams) stream.end();
    await new Promise(doneClose => control.close(() => doneClose()));
  }
}

if (process.argv[1] && resolve(process.argv[1]) === self) {
  (async () => {
    if (process.argv.length === 3 && process.argv[2] === "--actor-worker") return runActorWorker();
    const options = parsePlayableArguments(process.argv.slice(2));
    if (options.command !== "run") return print(await sendPlayableControl(options));
    await runPlayableService(options);
  })().catch(() => { print({ type: "r16PlayableFailed", code: "CONFIGURATION_UNAVAILABLE", dataPreserved: true }); process.exitCode = 1; });
}
