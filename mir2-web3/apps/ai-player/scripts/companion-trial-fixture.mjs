import { fork, spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { GatewayClient } from "../src/gateway-client.mjs";
import { appRoot, repoRoot, freePort, isolatedEnvironment, startLocalGateway } from "./local-gateway.mjs";
import { modelProcessEnvironment } from "../src/model-runtime-environment.mjs";

/** Child event callback receives only allowlisted ordinary game evidence. */
export function launchTrialActor({ profile, identity, archiveKey, run = false, onEvent = () => {}, modelEnvironment = {} }) {
  const child = fork(resolve(appRoot, "scripts/trial-actor-fixture.mjs"), [], {
    cwd: repoRoot, windowsHide: true, execArgv: [], serialization: "advanced", stdio: ["ignore", "pipe", "pipe", "ipc"],
    env: isolatedEnvironment({ MIR2_AI_PLAYER_ACCOUNT: identity.accountId, MIR2_AI_PLAYER_PASSWORD: identity.password,
      MIR2_COMPANION_ARCHIVE_KEY: archiveKey,
      ...(profile.runtimeService?.provider === "openai" ? modelProcessEnvironment(modelEnvironment.OPENAI_API_KEY, modelEnvironment) : {}) }),
  });
  let sequence = 0, stopped = false;
  const pending = new Map();
  // Node's experimental-SQLite warning is deliberately not printed as evidence.
  child.stdout.on("data", () => {}); child.stderr.on("data", () => {});
  child.on("message", message => {
    if (message?.type !== "result") { onEvent(message); return; }
    const waiter = pending.get(message.id);
    if (!waiter) return;
    clearTimeout(waiter.timer); pending.delete(message.id);
    if (message.error) {
      onEvent({ type: "fixtureFailure", operation: waiter.command, hint: message.errorHint });
      waiter.reject(new Error("The isolated actor operation failed."));
    } else waiter.resolve(message.result);
  });
  const exited = new Promise(resolveExit => child.once("exit", () => {
    stopped = true;
    for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("The isolated actor exited.")); }
    pending.clear(); resolveExit();
  }));
  child.on("error", () => {
    for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("The isolated actor could not start.")); }
    pending.clear();
  });
  function request(command, fields = {}) {
    if (stopped || !child.connected || pending.size >= 8) return Promise.reject(new Error("The isolated actor is unavailable."));
    return new Promise((resolveRequest, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error("The isolated actor operation timed out.")); }, 35_000);
      pending.set(id, { resolve: resolveRequest, reject, timer, command });
      child.send({ id, command, ...fields }, error => {
        if (!error || !pending.has(id)) return;
        clearTimeout(timer); pending.delete(id); reject(new Error("The isolated actor channel failed."));
      });
    });
  }
  async function close() {
    if (stopped) return;
    try { await request("stop"); } catch { /* only terminate this owned child below */ }
    let timer;
    try { await Promise.race([exited, new Promise(resolveWait => { timer = setTimeout(resolveWait, 5000); })]); }
    finally { clearTimeout(timer); if (!stopped) { child.kill(); await exited; } }
  }
  return { pid: child.pid, child, request, close, started: request("start", { profile, run }) };
}

/**
 * Private in-memory fixture. Do not print the returned identities/archiveKey.
 * Bootstrap and trial gateways use ordinary accounts and the same private store.
 */
export async function createTrialFixture({ gatewayExecutable, contentProfile, onEvent = () => {}, outputRoot, supplyConfigDirectory } = {}) {
  let bootstrap, gateway, activeActor;
  const clients = new Set();
  const identities = ["Owner", "Alo", "Guest"].map(prefix => ({
    accountId: `ait${randomBytes(5).toString("hex")}`, password: `Ab${randomBytes(8).toString("hex")}7`,
    characterName: `${prefix}${randomBytes(3).toString("hex")}`, characterIndex: null,
  }));
  const [owner, actor, stranger] = identities;
  try {
    bootstrap = await startLocalGateway({ executable: gatewayExecutable, contentProfile, prefix: "trial-bootstrap", outputRoot });
    onEvent({ type: "fixtureStage", stage: "bootstrap-gateway-ready" });
    for (const identity of identities) {
      const client = new GatewayClient({ url: bootstrap.url, realmId: bootstrap.realmId, timeoutMs: 25_000 });
      clients.add(client);
      const realm = await client.connect();
      if (realm.profileId !== bootstrap.contentProfile) throw new Error("The bootstrap gateway did not activate its declared content profile.");
      await client.createAccount(identity.accountId, identity.password);
      await client.login(identity.accountId, identity.password);
      const character = await client.createCharacter(identity.characterName);
      identity.characterIndex = character.index;
      await client.startGame(character.index);
      if (!await client.logout()) throw new Error("Ordinary bootstrap save was not acknowledged.");
      await client.close(); clients.delete(client);
      onEvent({ type: "fixtureStage", stage: "ordinary-character-saved", ordinal: identities.indexOf(identity) + 1 });
    }
    await bootstrap.close();
    const worldEpoch = randomUUID(), archiveId = randomUUID(), residentId = "alo-trial";
    const companionPolicy = { schemaVersion: 1, realmId: bootstrap.realmId, worldEpoch,
      enrollments: [{ residentId, archiveId, displayName: "阿洛", owner: { accountId: owner.accountId, characterIndex: owner.characterIndex },
        actor: { accountId: actor.accountId, characterIndex: actor.characterIndex } }] };
    gateway = await startLocalGateway({ executable: gatewayExecutable, contentProfile: bootstrap.contentProfile,
      prefix: "experience", reuse: { output: bootstrap.output, realmId: bootstrap.realmId }, companionPolicy,
      outputRoot, supplyConfigDirectory: supplyConfigDirectory ?? bootstrap.output });
    onEvent({ type: "fixtureStage", stage: "enrolled-gateway-ready" });
    const output = gateway.output, archiveKey = randomBytes(32).toString("hex");
    const profileBase = { url: gateway.url, realmId: gateway.realmId, characterName: actor.characterName, timeoutMs: 25_000,
      allowedMaps: ["0", "0141"], companion: { residentId, archiveId, ownerRef: `owner:${createHash("sha256").update(owner.accountId).digest("hex")}`,
        worldEpoch, characterIndex: actor.characterIndex, displayName: "阿洛", baseSetting: "比奇的合成人冒险者", leaderName: owner.characterName,
        homePosition: { x: 288, y: 616, mapFileName: "0" } },
      runtimeService: { controlEnabled: true, runtimeId: "runner-A", provider: "rules", model: "rules", promptVersion: "rules-v1", credentialRef: null,
        archiveKeyEnv: "MIR2_COMPANION_ARCHIVE_KEY", experiment: { experimentId: `test-${gateway.runId}`, participantRef: "synthetic-P01",
          ledgerPath: resolve(output, "experience.sqlite"), currency: "CNY", environment: "test", offer: null } } };
    const profileA = structuredClone(profileBase), profileB = structuredClone(profileBase);
    profileA.guardPort = await freePort(); profileA.companion.memoryPath = resolve(output, "runner-A", "alo.sqlite");
    do { profileB.guardPort = await freePort(); } while (profileB.guardPort === profileA.guardPort);
    profileB.companion.memoryPath = resolve(output, "runner-B", "alo.sqlite"); profileB.runtimeService.runtimeId = "runner-B";
    const profileAPath = resolve(output, "runner-A", "profile.json"), profileBPath = resolve(output, "runner-B", "profile.json");
    for (const [profile, path] of [[profileA, profileAPath], [profileB, profileBPath]]) {
      await mkdir(resolve(path, ".."), { recursive: true });
      await writeFile(path, JSON.stringify(profile, null, 2), { flag: "wx", mode: 0o600 });
    }
    async function newPlayerClient(identity) {
      const client = new GatewayClient({ url: gateway.url, realmId: gateway.realmId, timeoutMs: 25_000 }); clients.add(client);
      try {
        const realm = await client.connect();
        gateway.observedContentProfile = { profileId: realm.profileId, profileVersion: realm.profileVersion };
        if (realm.profileId !== gateway.contentProfile) throw new Error("The gateway did not activate its declared content profile.");
        await client.login(identity.accountId, identity.password); await client.startGame(identity.characterIndex); return client;
      }
      catch { await client.close(); clients.delete(client); throw new Error("Ordinary trial character admission failed."); }
    }
    async function stopActor() { const current = activeActor; activeActor = null; await current?.close(); }
    async function startActor(profile = profileA, { run = false, modelEnvironment = {} } = {}) {
      if (activeActor) throw new Error("Stop the current fixture actor first.");
      const current = launchTrialActor({ profile, identity: actor, archiveKey, run, onEvent, modelEnvironment }); activeActor = current;
      try { current.state = await current.started; return current; }
      catch { await stopActor(); throw new Error("The enrolled trial actor could not start."); }
    }
    function runCli(script, args, { timeoutMs = 20_000 } = {}) {
      if (!["memory-cli.mjs", "experience-cli.mjs"].includes(script)) throw new Error("Unsupported fixture CLI.");
      return new Promise(resolveResult => {
        const child = spawn(process.execPath, [resolve(appRoot, "src", script), ...args], { cwd: repoRoot, windowsHide: true,
          stdio: ["ignore", "pipe", "pipe"], env: isolatedEnvironment({ MIR2_COMPANION_ARCHIVE_KEY: archiveKey }) });
        let stdout = "", stderr = "", timedOut = false;
        const timer = setTimeout(() => { timedOut = true; child.kill(); }, timeoutMs);
        child.stdout.on("data", data => { stdout = (stdout + data).slice(-65_536); });
        child.stderr.on("data", data => { stderr = (stderr + data).slice(-65_536); });
        child.once("error", () => { clearTimeout(timer); resolveResult({ code: -1, timedOut, stdout: "", stderr: "CLI launch failed." }); });
        child.once("exit", code => { clearTimeout(timer); resolveResult({ code, timedOut, stdout, stderr }); });
      });
    }
    return { gateway, output, bootstrapOutput: bootstrap.output, owner, actor, stranger, profileA, profileB, profileAPath, profileBPath,
      archiveKey, companionPolicy, startActor, stopActor, runCli, newOwnerClient: () => newPlayerClient(owner),
      newStrangerClient: () => newPlayerClient(stranger), newActorClient: () => newPlayerClient(actor),
      async close() {
        await stopActor();
        for (const client of clients) { try { if (client.snapshot().ready) await client.logout(); } catch {} await client.close().catch(() => {}); }
        clients.clear(); await gateway.close();
      } };
  } catch {
    await activeActor?.close().catch(() => {});
    for (const client of clients) await client.close().catch(() => {});
    await gateway?.close().catch(() => {}); await bootstrap?.close().catch(() => {});
    throw new Error("The isolated trial fixture could not be prepared.");
  }
}
