// Private manual/Computer-Use fixture. It does not claim a human trial.
import { createInterface } from "node:readline";
import { spawn } from "node:child_process";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { isolatedEnvironment, repoRoot, resolveGatewayExecutable, sha256 } from "./local-gateway.mjs";
import { assembleNativeFixture, nativeFixtureArguments, verifyR16ResourceInput } from "./native-resource-input.mjs";
import { verifyArchive } from "../src/memory/archive-codec.mjs";
import { modelProcessEnvironment } from "../src/model-runtime-environment.mjs";
import { validApiKey, validModelName } from "../src/model/openai-client.mjs";

const options = nativeFixtureArguments(process.argv.slice(2));
let fixture, actor, nativeChild, nativeBundle, exportRoot, phase = "prepare", activeProfile, modelEnvironment = {};
const startedAt = Date.now();
const report = value => console.log(JSON.stringify(value));

async function launchNative() {
  if (!nativeBundle || nativeChild && nativeChild.exitCode === null) throw new Error("The private native window is already active or unavailable.");
  const roaming = resolve(fixture.output, "native-profile/Roaming"), local = resolve(fixture.output, "native-profile/Local");
  await mkdir(roaming, { recursive: true }); await mkdir(local, { recursive: true });
  // Canonical development auto-login still uses the ordinary authenticated
  // login/character flow. Credentials never enter TOML, argv or stdout.
  const current = spawn(nativeBundle.exe, [], { cwd: resolve(nativeBundle.exe, ".."), windowsHide: false,
    stdio: "ignore", env: isolatedEnvironment({ APPDATA: roaming, LOCALAPPDATA: local, MIR2_GATEWAY_WS_URL: fixture.gateway.url,
      MIR2_NATIVE_ACCOUNT: fixture.owner.accountId, MIR2_NATIVE_PASSWORD: fixture.owner.password,
      MIR2_NATIVE_CHARACTER_INDEX: String(fixture.owner.characterIndex), MIR2_COMPANION_EXPORT_DIR: exportRoot }) });
  nativeChild = current;
  current.once("error", () => { if (nativeChild === current) nativeChild = null; report({ type: "nativeFixtureWindowUnavailable" }); });
  report({ type: "nativeFixtureWindowLaunched", pid: nativeChild.pid, ordinaryAuthentication: true });
}
const readMeta = path => {
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    const profile = JSON.parse(db.prepare("SELECT profile_json FROM companion_state WHERE singleton=1").get().profile_json);
    const archive = JSON.parse(db.prepare("SELECT archive_json FROM character_archive WHERE singleton=1").get().archive_json);
    return { archiveId: archive.archiveId, worldEpoch: archive.worldEpoch, characterIndex: archive.characterIndex,
      policyVersion: profile.policyVersion, party: profile.party, growthRevision: db.prepare("SELECT growth_revision FROM character_archive WHERE singleton=1").get().growth_revision,
      feedback: db.prepare("SELECT feedback_id AS feedbackId,status FROM feedback").all(), episodeCount: db.prepare("SELECT COUNT(*) AS n FROM episodes").get().n };
  } finally { db.close(); }
};
async function ownExport() {
  for (const name of await readdir(exportRoot)) {
    if (!name.endsWith(".json")) continue;
    const path = join(exportRoot, name), info = await stat(path);
    if (info.size > 2 * 1024 * 1024 || info.mtimeMs < startedAt) continue;
    const encoded = await readFile(path, "utf8");
    try {
      const value = verifyArchive(encoded, Buffer.from(fixture.archiveKey, "hex"));
      if (value.characterArchive.archiveId === fixture.profileA.companion.archiveId) return { path, encoded, value };
    } catch { /* unrelated exports are not read into evidence or printed */ }
  }
  throw new Error("No completed export for this private test archive is available.");
}
async function command(line) {
  switch (line.trim()) {
    case "launch": await launchNative(); break;
    case "status": report({ type: "nativeFixtureStatus", phase, runtimeId: activeProfile.runtimeService.runtimeId, ...readMeta(activeProfile.companion.memoryPath) }); break;
    case "modelStatus": report({ type: "nativeFixtureModelStatus", ...(await actor.request("modelStatus")) }); break;
    case "restartActor": {
      const before = readMeta(activeProfile.companion.memoryPath), oldPid = actor.pid;
      await fixture.stopActor(); actor = null;
      report({ type: "nativeFixtureActorRestartWaiting", milliseconds: 31_000 });
      await sleep(31_000);
      actor = await fixture.startActor(activeProfile, { run: true, modelEnvironment });
      const after = readMeta(activeProfile.companion.memoryPath);
      if (before.archiveId !== after.archiveId || before.policyVersion !== after.policyVersion || actor.pid === oldPid) throw new Error("Restart did not preserve exact character history.");
      report({ type: "nativeFixtureActorRestarted", oldPid, newPid: actor.pid, differentProcess: actor.pid !== oldPid, ...after }); break;
    }
    case "exports": {
      const value = await ownExport();
      report({ type: "nativeFixtureExport", path: value.path, archiveId: value.value.characterArchive.archiveId, policyVersion: value.value.playerGrowth.policyVersion }); break;
    }
    case "migrate": {
      if (phase === "runner-B") throw new Error("Migration already completed.");
      const exported = await ownExport(), oldPid = actor.pid;
      await fixture.stopActor(); actor = null;
      const imported = await fixture.runCli("memory-cli.mjs", ["import", "--profile", fixture.profileBPath, "--input", exported.path]);
      if (imported.code !== 0) throw new Error("The isolated imported archive was not accepted.");
      report({ type: "nativeFixtureMigrationWaiting", reloginSettleMs: 31_000 });
      await sleep(31_000); // existing logout route lease, real time
      actor = await fixture.startActor(fixture.profileB, { run: true, modelEnvironment });
      activeProfile = fixture.profileB; phase = "runner-B";
      report({ type: "nativeFixtureMigrationPassed", oldPid, newPid: actor.pid, differentProcess: oldPid !== actor.pid, ...readMeta(activeProfile.companion.memoryPath) }); break;
    }
    case "stop": nativeChild?.kill(); await fixture.close(); fixture = null; process.exitCode = 0; process.exit(0); break;
    default: report({ type: "nativeFixtureCommands", commands: ["launch", "status", "exports", "restartActor", "migrate", "stop"] });
  }
}

try {
  phase = "validate-local-binary-inputs";
  const gatewayExecutable = await resolveGatewayExecutable(options.gatewayExecutable);
  const nativeExecutable = options.nativeExecutable ?? resolve(repoRoot, "apps/game-client/platform-windows/target/release/mir2-platform-windows.exe");
  if (!(await stat(nativeExecutable)).isFile()) throw new Error("Pass the newly built Windows EXE with --native-exe.");
  phase = "verify-r16-resource-input";
  report({ type: "nativeFixtureResourceVerificationStarted", candidate: "WN-CANDIDATE-20261101-unified-16" });
  const resources = await verifyR16ResourceInput({ candidateRoot: options.candidateRoot, releaseControlRoot: options.releaseControlRoot,
    onProgress: value => report({ type: "nativeFixtureResourceVerification", ...value }) });
  phase = "ordinary-gateway-and-actor";
  fixture = await createTrialFixture({ gatewayExecutable });
  exportRoot = resolve(fixture.output, "native-exports"); await mkdir(exportRoot);
  if (options.liveOpenaiFromEnvironment) {
    const model = process.env.MIR2_AI_MODEL_NAME ?? "gpt-6-luna";
    if (!validApiKey(process.env.OPENAI_API_KEY) || !validModelName(model)) throw new Error("Explicit local API credentials and model are required.");
    for (const profile of [fixture.profileA, fixture.profileB]) Object.assign(profile.runtimeService, { provider: "openai", model, credentialRef: "OPENAI_API_KEY", promptVersion: "alo-objective-v2", maxCallsPerHour: 20 });
    modelEnvironment = modelProcessEnvironment(process.env.OPENAI_API_KEY, process.env);
  }
  actor = await fixture.startActor(fixture.profileA, { run: true, modelEnvironment }); activeProfile = fixture.profileA; phase = "runner-A";
  if (options.nativeOutputRoot && !(await stat(options.nativeOutputRoot)).isDirectory()) throw new Error("The explicit native output root must already be a directory.");
  const nativeDirectory = options.nativeOutputRoot
    ? resolve(options.nativeOutputRoot, `native-client-${fixture.gateway.runId}`) : resolve(fixture.output, "native-client");
  nativeBundle = await assembleNativeFixture({ directory: nativeDirectory, nativeExecutable, resources });
  const { exe, assetRoot } = nativeBundle;
  await writeFile(resolve(nativeDirectory, "mir2-client.toml"), `[server]\ngateway_ws_url = "${fixture.gateway.url}"\nallowed_classes = ["Warrior", "Wizard", "Taoist"]\n[display]\nwidth = 1024\nheight = 768\nforce_daylight = true\n[gameplay]\nquest_guidance = "newcomer-v2"\n`, { flag: "wx" });
  const credentialsFile = resolve(fixture.output, "native-credentials.local.json");
  await writeFile(credentialsFile, JSON.stringify({ accountId: fixture.owner.accountId, password: fixture.owner.password, characterName: fixture.owner.characterName, characterIndex: fixture.owner.characterIndex }), { flag: "wx", mode: 0o600 });
  report({ type: "nativeFixtureReady", output: fixture.output, exe, credentialsFile, assetRoot, exportRoot,
    contentProfile: fixture.gateway.contentProfile, observedContentProfile: fixture.gateway.observedContentProfile ?? null, gameplayBaseline: fixture.gateway.gameplayBaseline,
    resourceInput: nativeBundle.provenance.resourceInput, nativeSha256: await sha256(exe), gatewaySha256: await sha256(fixture.gateway.executable),
    archiveId: fixture.profileA.companion.archiveId, realmId: fixture.gateway.realmId, environment: "test", modelEnabled: activeProfile.runtimeService.provider === "openai" });
  if (options.launchNative) await launchNative();
  const lines = createInterface({ input: process.stdin, terminal: false });
  let queue = Promise.resolve();
  lines.on("line", line => { queue = queue.then(() => command(line)).catch(() => report({ type: "nativeFixtureOperationUnavailable", phase })); });
  lines.on("close", () => { nativeChild?.kill(); queue.then(() => fixture?.close()).finally(() => { process.exitCode = 0; }); });
  process.once("SIGTERM", () => { nativeChild?.kill(); fixture?.close().finally(() => process.exit(0)); });
  process.once("SIGINT", () => { nativeChild?.kill(); fixture?.close().finally(() => process.exit(0)); });
} catch {
  nativeChild?.kill();
  await fixture?.close().catch(() => {});
  report({ type: "nativeFixtureFailed", phase }); process.exitCode = 1;
}
