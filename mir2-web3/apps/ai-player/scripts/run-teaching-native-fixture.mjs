// Private Windows teaching fixture. Startup makes no model request; only the
// native owner's teaching/commission controls can invoke the configured model.
// All characters are synthetic and this is not a real-human retention trial.
import { createInterface } from "node:readline";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { appendFileSync, writeFileSync } from "node:fs";
import { mkdir, realpath, stat, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, parse, relative, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { assembleNativeFixture, verifyR16ResourceInput } from "./native-resource-input.mjs";
import { isolatedEnvironment, resolveGatewayExecutable, sha256 } from "./local-gateway.mjs";
import { loadLocalModelSettings } from "../src/local-model-settings.mjs";
import { loadLocalModelNetwork, modelProcessEnvironment } from "../src/model-runtime-environment.mjs";

const COMMANDS = ["auto", "pause", "tick", "status", "restart", "stop"];
const HABITS = ["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"];
const HELP = "node scripts/run-teaching-native-fixture.mjs --gateway-exe <new-gateway.exe> --native-exe <new-native.exe> --out-root F:/mir2-builds/alo-cultivation-20261103/teaching-native --settings-bundle F:/mir2-builds/alo-r16-20261103";
const safeText = (value, limit = 160) => {
  if (typeof value !== "string") return null;
  if (/\b(?:sk|ghp|github_pat)[-_][A-Za-z0-9_-]{8,}|\bbearer\s+\S+|(?:password|api[ _-]?key|secret)\s*[:=]\s*\S+/iu.test(value)) return "[redacted]";
  return [...value.replace(/[\p{Cc}\p{Cf}\p{Cs}]/gu, " ")].slice(0, limit).join("");
};
const number = value => Number.isSafeInteger(value) ? value : null;
const point = value => value && Number.isInteger(value.x) && Number.isInteger(value.y) ? { x: value.x, y: value.y } : null;
const name = value => safeText(value, 64);
const habits = value => Object.fromEntries(HABITS.map(key => [key, value?.[key] === true]));
const scalarId = value => typeof value === "bigint" ? value.toString() : typeof value === "string" && /^(?:0|[1-9][0-9]{0,19})$/.test(value) ? value : number(value);
const within = (root, path) => { const part = relative(root, path); return part === "" || !part.startsWith("..") && !isAbsolute(part); };

function argumentsFor(args) {
  const result = { outRoot: "F:/mir2-builds/alo-cultivation-20261103/teaching-native", settingsBundle: "F:/mir2-builds/alo-r16-20261103" };
  const flags = new Map([["--gateway-exe", "gatewayExecutable"], ["--native-exe", "nativeExecutable"], ["--out-root", "outRoot"],
    ["--settings-bundle", "settingsBundle"], ["--candidate-root", "candidateRoot"], ["--release-control-root", "releaseControlRoot"]]);
  const seen = new Set();
  if (args.length === 1 && args[0] === "--help") return { help: true };
  for (let index = 0; index < args.length; index++) {
    const key = flags.get(args[index]), value = args[++index];
    if (!key || seen.has(key) || typeof value !== "string" || !value.trim() || value.startsWith("-") || !isAbsolute(value)) throw new Error("Invalid private teaching fixture arguments.");
    seen.add(key); result[key] = resolve(value);
  }
  if (!result.gatewayExecutable || !result.nativeExecutable) throw new Error("Explicit newly built gateway and native inputs are required.");
  return result;
}

async function outputDirectory(requested) {
  const path = resolve(requested);
  if (parse(path).root.toLowerCase() !== "f:\\" || path === parse(path).root) throw new Error("Use a dedicated absolute F output directory.");
  // Check the existing ancestor before mkdir so an F junction cannot silently
  // send the private profiles/logs to the constrained source drive.
  let ancestor = path;
  while (true) {
    try { await stat(ancestor); break; }
    catch (error) { if (error.code !== "ENOENT") throw error; const parent = dirname(ancestor); if (parent === ancestor) throw error; ancestor = parent; }
  }
  if (parse(await realpath(ancestor)).root.toLowerCase() !== "f:\\") throw new Error("The private output ancestor must resolve to F.");
  await mkdir(path, { recursive: true }); const canonical = await realpath(path);
  if (parse(canonical).root.toLowerCase() !== "f:\\") throw new Error("The private output must resolve to F.");
  return canonical;
}

function ownerView(path) {
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    db.exec("BEGIN");
    const profile = JSON.parse(db.prepare("SELECT profile_json FROM companion_state WHERE singleton=1").get().profile_json);
    const archive = JSON.parse(db.prepare("SELECT archive_json FROM character_archive WHERE singleton=1").get().archive_json);
    const rows = db.prepare("SELECT episode_id,recorded_at,policy_version,status,payload_json FROM episodes ORDER BY sequence DESC LIMIT 20").all();
    const feedback = db.prepare("SELECT f.feedback_id,f.status,f.payload_json,e.recorded_at FROM feedback f JOIN episodes e ON e.episode_id=f.episode_id ORDER BY e.sequence DESC LIMIT 20").all();
    const result = {
      archive: Object.fromEntries(["archiveId", "residentId", "displayName", "characterName", "realmId", "worldEpoch"].map(key => [key, safeText(archive[key], 100)])),
      profile: { policyVersion: number(profile.policyVersion), style: safeText(profile.style, 32), party: habits(profile.party) },
      growthRevision: number(db.prepare("SELECT growth_revision FROM character_archive WHERE singleton=1").get().growth_revision),
      feedback: feedback.map(row => ({ feedbackId: safeText(row.feedback_id, 100), status: safeText(row.status, 24), recordedAt: number(row.recorded_at),
        ...(row.status === "deleted" ? {} : { summary: safeText(JSON.parse(row.payload_json).feedbackText, 160) }) })),
      episodes: rows.map(row => { const payload = row.status === "deleted" ? {} : JSON.parse(row.payload_json); return {
        episodeId: safeText(row.episode_id, 100), status: safeText(row.status, 24), recordedAt: number(row.recorded_at), policyVersion: number(row.policy_version),
        kind: safeText(payload.kind, 48), action: safeText(payload.action, 48), outcome: safeText(payload.outcome, 32), summary: safeText(payload.summary, 160),
      }; }),
      durableModelCalls: number(db.prepare("SELECT COUNT(*) AS n FROM episodes WHERE episode_id GLOB 'goal:*:call' OR episode_id GLOB 'teaching:*:call'").get().n),
    };
    db.exec("COMMIT"); return result;
  } finally { db.close(); }
}

function publicModel(value) {
  const task = value?.modelTask ?? {}, proposal = value?.teachingProposal ?? {};
  const call = raw => raw ? Object.fromEntries(["model", "providerModel", "status", "requestId", "responseId"].map(key => [key, safeText(raw[key], 160)]).concat(
    ["inputTokens", "outputTokens", "httpStatus"].map(key => [key, number(raw[key])]))) : null;
  const references = rows => Array.isArray(rows) ? rows.slice(0, 5).map(row => ({ episodeId: safeText(row.episodeId, 100), summary: safeText(row.summary, 160) })) : [];
  return { provider: safeText(value?.provider, 24), model: safeText(value?.model, 100), configured: value?.configured === true, modelEnabled: value?.modelEnabled === true, modelCalls: number(value?.modelCalls),
    modelTask: { status: safeText(task.status, 32), step: safeText(task.step, 48), reply: safeText(task.reply, 400), instruction: safeText(task.instruction, 1000),
      completedKills: number(task.completedKills), requestedKills: number(task.requestedKills), references: references(task.references), lastCall: call(task.lastCall) },
    teachingProposal: { proposalId: safeText(proposal.proposalId, 100), status: safeText(proposal.status, 32), sourceVersion: number(proposal.sourceVersion), reply: safeText(proposal.reply, 160),
      changes: Object.fromEntries(HABITS.filter(key => typeof proposal.changes?.[key] === "boolean").map(key => [key, proposal.changes[key]])), references: references(proposal.references), lastCall: call(proposal.lastCall) } };
}

function publicActor(snapshot, ownerName) {
  const owner = snapshot.entities?.find(entity => entity.kind === "player" && entity.name === ownerName);
  const party = snapshot.party ?? {};
  return { ready: snapshot.ready === true, tick: scalarId(snapshot.tick), mapFileName: safeText(snapshot.mapFileName, 64),
    playerObjectId: number(snapshot.playerObjectId), selfZoneObjectId: number(snapshot.selfZoneObjectId), position: point(snapshot.player),
    hp: number(snapshot.playerHp), maxHp: number(snapshot.playerMaxHp), gold: number(snapshot.gold),
    ownerVisible: Boolean(owner), ownerPosition: point(owner),
    party: { members: (party.members ?? []).slice(0, 15).map(row => name(typeof row === "string" ? row : row?.name)),
      pendingInvite: party.pendingInvite ? { name: name(party.pendingInvite.name), observedAt: number(party.pendingInvite.observedAt) } : null } };
}

function proofEvent(message) {
  const event = message?.event, payload = event?.payload ?? {};
  if (event?.type === "snapshotPosition") return { type: "snapshotPosition", tick: scalarId(payload.tick), player: point(payload.player), selfZoneObjectId: number(payload.selfZoneObjectId) };
  const packet = event?.packet;
  if (!["ObjectStruck", "ObjectAttack", "ObjectDied", "ObjectWalk", "UserLocation", "UseItem", "DeleteItem", "GainedItem", "ObjectItem", "HealthChanged", "GroupMemberInfo", "SendMemberLocation"].includes(packet)) return null;
  const info = payload.info ?? payload, item = payload.item ?? info.item;
  const value = { packet, objectId: number(info.objectId), attackerId: number(info.attackerId), location: point(info.location ?? info),
    direction: safeText(info.direction, 24), hp: number(info.hp), maxHp: number(info.maxHp), success: typeof info.success === "boolean" ? info.success : null };
  if (item) value.item = { itemIndex: number(item.itemIndex), uniqueId: scalarId(item.uniqueId), count: number(item.count) };
  if (payload.uniqueId !== undefined) value.uniqueId = scalarId(payload.uniqueId);
  if (packet === "GroupMemberInfo") value.members = (payload.members ?? []).slice(0, 15).map(row => ({ name: name(row.name), hp: number(row.hp), maxHp: number(row.maxHp), online: row.online === true }));
  return value;
}

let fixture, actor, native, nativeExit, lines, sessionRoot, profile, modelEnvironment = {}, files;
let phase = "arguments", automatic = false, automaticTimer, tickQueued = false, closing = false, stopRequested = false, restartAbort, closePromise, controlQueue = Promise.resolve();
function emit(channel, payload, stdout = channel !== "proof") {
  const event = { at: Date.now(), ...payload }, encoded = JSON.stringify(event);
  if (files) appendFileSync(files[channel], encoded + "\n", "utf8");
  if (stdout) process.stdout.write(encoded + "\n");
}
function onEvent(value) {
  if (value?.type === "proof") { const event = proofEvent(value); if (event) emit("proof", { type: "teachingNativeProof", event }, false); return; }
  if (value?.type === "command" && typeof value.command === "string" && /^[A-Za-z][A-Za-z0-9]{0,48}$/.test(value.command)) { emit("proof", { type: "ordinaryCommand", command: value.command }, false); return; }
  if (value?.type === "fixtureStage") { emit("report", { type: "fixtureStage", stage: safeText(value.stage, 64), ordinal: number(value.ordinal) }); return; }
  if (value?.type === "report") {
    const event = value.event ?? {};
    if (["companionDecision", "companionStep", "companionReady", "companionControlReady", "residentReady", "residentStopped", "fixtureRunStopped", "supplyMemoryWriteUnavailable"].includes(event.type)) {
      emit("report", { type: event.type, kind: safeText(event.kind, 32), reason: safeText(event.reason, 160), policyVersion: number(event.policyVersion),
        direction: safeText(event.direction, 24), from: point(event.from), target: point(event.target), saved: event.saved === true }, false);
    }
  }
}

function pause() { automatic = false; clearInterval(automaticTimer); automaticTimer = null; }
async function tick(manual = false) {
  if (!actor || closing) return;
  const result = await actor.request("tick");
  const value = { type: "teachingNativeTick", manual, kind: safeText(result?.kind, 32), reason: safeText(result?.reason, 160), policyVersion: number(result?.policyVersion),
    performed: result?.performed === true, moved: result?.moved === true, confirmed: result?.confirmed === true, blocked: result?.blocked === true };
  emit("proof", value, manual);
}
function queueAutoTick() {
  if (!automatic || tickQueued || closing || stopRequested) return;
  tickQueued = true;
  controlQueue = controlQueue.then(async () => { if (automatic && !closing && !stopRequested) await tick(); }).catch(() => {
    pause(); emit("report", { type: "teachingNativeAutomaticPaused", phase });
  }).finally(() => { tickQueued = false; });
}
async function status() {
  const snapshot = await actor.request("snapshot"), modelStatus = publicModel(await actor.request("modelStatus"));
  const value = { type: "teachingNativeStatus", phase, actorPid: actor.pid, nativePid: native?.pid ?? null, automatic,
    ownerView: ownerView(profile.companion.memoryPath), actor: publicActor(snapshot, fixture.owner.characterName), modelStatus };
  emit("report", value); return value;
}
async function shutdown() {
  if (closePromise) return closePromise;
  closing = true; stopRequested = true; restartAbort?.abort(); pause();
  closePromise = (async () => {
    if (native && native.exitCode === null && native.signalCode === null) {
      native.kill(); await Promise.race([nativeExit, sleep(5000)]);
    }
    await fixture?.close(); fixture = null; actor = null;
    delete modelEnvironment.OPENAI_API_KEY; modelEnvironment = {};
    emit("report", { type: "teachingNativeStopped", environment: "test" });
    lines?.close(); process.stdin.pause();
  })();
  return closePromise;
}
async function command(value) {
  if (closing || stopRequested && value !== "stop") return;
  emit("controls", { type: "teachingNativeControl", command: value }, false);
  if (value === "auto") {
    if (!automatic) { automatic = true; automaticTimer = setInterval(queueAutoTick, 650); queueAutoTick(); }
    emit("report", { type: "teachingNativeAutomatic", enabled: true });
  } else if (value === "pause") { pause(); emit("report", { type: "teachingNativeAutomatic", enabled: false }); }
  else if (value === "tick") { if (automatic) throw new Error("Pause automatic execution first."); await tick(true); }
  else if (value === "status") await status();
  else if (value === "restart") {
    pause(); const oldPid = actor.pid;
    await fixture.stopActor(); actor = null;
    const before = ownerView(profile.companion.memoryPath);
    emit("report", { type: "teachingNativeRestartWaiting", milliseconds: 31000, oldPid });
    restartAbort = new AbortController();
    try { await sleep(31000, undefined, { signal: restartAbort.signal }); } // Real ordinary logout/admission lease; never accelerated.
    catch (error) { if (error.name === "AbortError") return; throw error; }
    finally { restartAbort = null; }
    if (stopRequested) return;
    actor = await fixture.startActor(profile, { run: false, modelEnvironment });
    const after = ownerView(profile.companion.memoryPath), model = publicModel(await actor.request("modelStatus"));
    if (actor.pid === oldPid || before.archive.archiveId !== after.archive.archiveId || before.profile.policyVersion !== after.profile.policyVersion ||
        JSON.stringify(before.profile.party) !== JSON.stringify(after.profile.party) || before.durableModelCalls !== after.durableModelCalls || model.modelCalls !== before.durableModelCalls) throw new Error("Restart identity/history validation failed.");
    emit("report", { type: "teachingNativeRestarted", oldPid, newPid: actor.pid, differentProcess: true, automatic: false, modelCallsUnchanged: true,
      archiveId: after.archive.archiveId, policyVersion: after.profile.policyVersion, habits: after.profile.party });
    await status();
  } else if (value === "stop") await shutdown();
}

try {
  const options = argumentsFor(process.argv.slice(2));
  if (options.help) { process.stdout.write(HELP + "\nstdin: " + COMMANDS.join(" / ") + "\n"); }
  else {
    if (process.platform !== "win32") throw new Error("This fixture requires Windows.");
    phase = "private-output";
    const root = await outputDirectory(options.outRoot);
    // The server's atomic initialized-head publication adds a 64-hex temp
    // suffix below trial-bootstrap-<UUID>. A verbose session UUID directory
    // made that real Win32 rename path 269 characters in the failed fixture.
    sessionRoot = resolve(root, `tn-${randomUUID().replace(/-/g, "").slice(0, 16)}`);
    const journalPublicationPath = resolve(sessionRoot, "trial-bootstrap-00000000-0000-0000-0000-000000000000",
      `supply-server.journal.json.initialized.tmp.${"0".repeat(64)}`);
    const pathBudget = { type: "teachingNativePathBudget", sessionRootLength: sessionRoot.length, maximumJournalPublicationPathLength: journalPublicationPath.length, limit: 240 };
    if (journalPublicationPath.length > 240) { emit("report", pathBudget); throw new Error("Choose a shorter private output root."); }
    await mkdir(sessionRoot);
    files = Object.fromEntries(["report", "proof", "controls"].map(channel => [channel, resolve(sessionRoot, `teaching-${channel}.jsonl`)]));
    for (const path of Object.values(files)) writeFileSync(path, "", { flag: "wx", mode: 0o600 });
    emit("report", pathBudget);
    phase = "validate-binaries";
    const gatewayExecutable = await resolveGatewayExecutable(options.gatewayExecutable), nativeExecutable = await realpath(options.nativeExecutable);
    if (!(await stat(nativeExecutable)).isFile()) throw new Error("The native input is unavailable.");
    const settingsBundle = await realpath(resolve(options.settingsBundle));
    if (within(settingsBundle, sessionRoot) || within(sessionRoot, settingsBundle)) throw new Error("Private output cannot overlap model settings.");
    phase = "verify-r16-resources";
    emit("report", { type: "teachingNativeResourceVerificationStarted" });
    const resources = await verifyR16ResourceInput({ candidateRoot: options.candidateRoot, releaseControlRoot: options.releaseControlRoot,
      onProgress: progress => emit("report", { type: "teachingNativeResourceVerification", verifiedResources: number(progress.verifiedResources), totalResources: number(progress.totalResources) }) });
    phase = "ordinary-fixture";
    fixture = await createTrialFixture({ gatewayExecutable, outputRoot: sessionRoot, onEvent });
    phase = "local-model-settings";
    const settings = await loadLocalModelSettings({ bundleDir: settingsBundle, env: isolatedEnvironment({}) });
    if (settings.provider !== "openai" || !settings.apiKey) throw new Error("Configure the local Windows credential first.");
    try { modelEnvironment = modelProcessEnvironment(settings.apiKey, await loadLocalModelNetwork({ bundleDir: settingsBundle })); }
    finally { delete settings.apiKey; }
    profile = fixture.profileA;
    Object.assign(profile.runtimeService, { provider: "openai", model: settings.model, credentialRef: "OPENAI_API_KEY", promptVersion: "alo-cultivation-v1", maxCallsPerHour: 5 });
    await writeFile(fixture.profileAPath, JSON.stringify(profile, null, 2) + "\n", { mode: 0o600 });
    phase = "paused-actor";
    actor = await fixture.startActor(profile, { run: false, modelEnvironment });
    const initial = ownerView(profile.companion.memoryPath), initialModel = publicModel(await actor.request("modelStatus"));
    if (HABITS.some(key => initial.profile.party[key] !== false) || initial.durableModelCalls !== 0 || initialModel.modelCalls !== 0) throw new Error("Fresh collection profile or startup model count differs.");
    phase = "assemble-native";
    const nativeDirectory = resolve(sessionRoot, "native-client"), nativeBundle = await assembleNativeFixture({ directory: nativeDirectory, nativeExecutable, resources });
    const roaming = resolve(sessionRoot, "native-profile/Roaming"), local = resolve(sessionRoot, "native-profile/Local"), exports = resolve(sessionRoot, "native-exports");
    await mkdir(roaming, { recursive: true }); await mkdir(local, { recursive: true }); await mkdir(exports);
    await writeFile(resolve(nativeDirectory, "mir2-client.toml"), `[server]\ngateway_ws_url = "${fixture.gateway.url}"\nallowed_classes = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]\n[display]\nwidth = 1024\nheight = 768\nforce_daylight = true\n[gameplay]\nquest_guidance = "newcomer-v2"\n`, { flag: "wx" });
    phase = "native-ready";
    native = spawn(nativeBundle.exe, [], { cwd: nativeDirectory, windowsHide: false, stdio: "ignore", env: isolatedEnvironment({
      APPDATA: roaming, LOCALAPPDATA: local, MIR2_GATEWAY_WS_URL: fixture.gateway.url,
      MIR2_NATIVE_ACCOUNT: fixture.owner.accountId, MIR2_NATIVE_PASSWORD: fixture.owner.password,
      MIR2_NATIVE_CHARACTER_INDEX: String(fixture.owner.characterIndex), MIR2_COMPANION_EXPORT_DIR: exports,
    }) });
    nativeExit = new Promise(resolveExit => {
      native.once("exit", (code, signal) => { emit("report", { type: "teachingNativeWindowExited", pid: native.pid, code: number(code), signal: safeText(signal, 24) }); resolveExit(); });
      native.once("error", resolveExit);
    });
    await new Promise((resolveSpawn, rejectSpawn) => {
      native.once("spawn", resolveSpawn);
      native.once("error", () => rejectSpawn(new Error("The private native process could not start.")));
    });
    emit("report", { type: "teachingNativeReady", environment: "test", realHumanTrial: false, output: sessionRoot, fixtureOutput: fixture.output, logs: files,
      exe: nativeBundle.exe, nativePid: native.pid, actorPid: actor.pid, gatewaySha256: await sha256(gatewayExecutable), nativeSha256: await sha256(nativeBundle.exe),
      resourceInput: nativeBundle.provenance.resourceInput, contentProfile: fixture.gateway.contentProfile, model: safeText(settings.model, 100), maxCallsPerHour: 5,
      initialHabits: initial.profile.party, initialModelCalls: 0, automatic: false, commands: COMMANDS });
    lines = createInterface({ input: process.stdin, terminal: false });
    lines.on("line", line => {
      const value = line.trim();
      if (!COMMANDS.includes(value)) { emit("report", { type: "teachingNativeCommands", commands: COMMANDS }); return; }
      if (["pause", "restart", "stop"].includes(value)) pause();
      if (value === "stop") { stopRequested = true; restartAbort?.abort(); }
      controlQueue = controlQueue.then(() => command(value)).catch(() => { pause(); emit("report", { type: "teachingNativeOperationUnavailable", command: value, phase }); });
    });
    lines.on("close", () => { stopRequested = true; restartAbort?.abort(); pause(); controlQueue.then(shutdown).catch(() => { process.exitCode = 1; }); });
    for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { stopRequested = true; restartAbort?.abort(); pause(); controlQueue.then(shutdown).catch(() => { process.exitCode = 1; }); });
  }
} catch {
  try { emit("report", { type: "teachingNativeFailed", phase }); } catch { /* fixed failure only; never serialize the original exception */ }
  await shutdown().catch(() => {}); process.exitCode = 1;
}
