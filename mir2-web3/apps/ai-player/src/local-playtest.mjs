// Local developer playtest launcher. It never contacts the public TEST world.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { createWriteStream } from "node:fs";
import { access, lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { acquireArchiveGuard } from "./archive-guard.mjs";
import { loadLocalModelSettings } from "./local-model-settings.mjs";
import { loadLocalModelNetwork, modelProcessEnvironment } from "./model-runtime-environment.mjs";
import { canonicalPrivateDirectory } from "./local-supply-server-config.mjs";
import { R16_GAMEPLAY_BASELINE } from "./generated-gameplay-baseline.mjs";
import { createTrialFixture, launchTrialActor } from "../scripts/companion-trial-fixture.mjs";
import { appRoot, repoRoot, freePort, isolatedEnvironment, startLocalGateway, resolveLocalOutputRoot } from "../scripts/local-gateway.mjs";

const STATE_SCHEMA = "mir2-alo-r16-local-playtest/1";
let startupStage = "starting";
const startupStages = new Set(["starting", "reading-local-state", "initializing-local-characters", "bootstrap-gateway-ready", "ordinary-character-saved", "enrolled-gateway-ready", "opening-saved-world", "connecting-companion", "opening-game", "running", "stopped"]);
function recordStartupStage(stage) {
  if (!startupStages.has(stage)) return;
  startupStage = stage;
  console.log(JSON.stringify({ type: "localPlaytestStage", stage, at: new Date().toISOString() }));
}
const inside = (parent, path) => {
  const difference = relative(parent, path);
  return Boolean(difference) && difference !== ".." && !difference.startsWith(`..\\`) && !difference.startsWith("../") && !isAbsolute(difference);
};

export function validateLocalPlaytestState(state, dataRoot) {
  const invalid = () => { throw new Error("The existing local trial state is invalid; it was not replaced."); };
  if (!state || state.schema !== STATE_SCHEMA || !state.profile || !state.companionPolicy || !Array.isArray(state.companionPolicy.enrollments)) invalid();
  const { companion, runtimeService } = state.profile;
  const enrollment = state.companionPolicy.enrollments[0];
  if (!companion || !runtimeService || state.companionPolicy.enrollments.length !== 1 || !enrollment || !state.owner || !state.actor) invalid();
  if (state.profile.realmId !== state.companionPolicy.realmId || companion.worldEpoch !== state.companionPolicy.worldEpoch || companion.archiveId !== enrollment.archiveId || companion.residentId !== enrollment.residentId) invalid();
  for (const [role, identity] of [["owner", state.owner], ["actor", state.actor]]) {
    if (typeof identity.accountId !== "string" || !/^ait[0-9a-f]{10}$/.test(identity.accountId) || typeof identity.password !== "string" || identity.password.length < 16 || !Number.isSafeInteger(identity.characterIndex) || identity.characterIndex < 0 || typeof identity.characterName !== "string") invalid();
    if (enrollment[role]?.accountId !== identity.accountId || enrollment[role]?.characterIndex !== identity.characterIndex) invalid();
  }
  if (companion.characterIndex !== state.actor.characterIndex || state.profile.characterName !== state.actor.characterName || companion.leaderName !== state.owner.characterName || !/^[0-9a-f]{64}$/.test(state.archiveKey ?? "")) invalid();
  if (companion.ownerRef !== `owner:${createHash("sha256").update(state.owner.accountId).digest("hex")}`) invalid();
  if (!inside(dataRoot, resolve(state.bootstrapOutput ?? "")) || !inside(dataRoot, resolve(companion.memoryPath ?? "")) || !inside(dataRoot, resolve(runtimeService.experiment?.ledgerPath ?? ""))) invalid();
  if (runtimeService.controlEnabled !== true || runtimeService.provider !== "rules" || runtimeService.model !== "rules" || runtimeService.credentialRef !== null || runtimeService.archiveKeyEnv !== "MIR2_COMPANION_ARCHIVE_KEY" || runtimeService.experiment?.environment !== "test" || runtimeService.experiment?.offer !== null) invalid();
  return state;
}

export async function prepareLocalPlaytest({ gatewayExecutable, stateFile, outputRoot, onStage = () => {} } = {}) {
  const dataRoot = await resolveLocalOutputRoot(outputRoot);
  stateFile ??= resolve(dataRoot, "local-playtest/state.local.json");
  if (!inside(dataRoot, resolve(stateFile))) throw new Error("Keep local trial state in this application's private data directory.");
  stateFile = resolve(await canonicalPrivateDirectory(dirname(resolve(stateFile)), dataRoot), basename(stateFile));
  onStage("reading-local-state");
  let state;
  try {
    if ((await lstat(stateFile)).isSymbolicLink()) throw new Error("Invalid linked trial state.");
    const encoded = await readFile(stateFile, "utf8");
    if (Buffer.byteLength(encoded) > 64 * 1024) throw new Error("Oversized local state.");
    state = validateLocalPlaytestState(JSON.parse(encoded), dataRoot);
  } catch (error) {
    if (error.code !== "ENOENT") throw new Error("Existing local trial state could not be read; no new character was substituted.");
  }
  if (!state) {
    onStage("initializing-local-characters");
    const fixture = await createTrialFixture({ gatewayExecutable, outputRoot: dataRoot, supplyConfigDirectory: dirname(stateFile), onEvent: event => {
      if (event?.type === "fixtureStage" && startupStages.has(event.stage)) onStage(event.stage);
    } });
    try {
      state = validateLocalPlaytestState({ schema: STATE_SCHEMA, owner: fixture.owner, actor: fixture.actor,
        bootstrapOutput: fixture.bootstrapOutput, archiveKey: fixture.archiveKey, profile: fixture.profileA,
        companionPolicy: fixture.companionPolicy }, dataRoot);
      await writeFile(stateFile, JSON.stringify(state, null, 2), { flag: "wx", mode: 0o600 });
      return { ...fixture, profile: fixture.profileA, firstRun: true };
    } catch (error) { await fixture.close(); throw error; }
  }
  onStage("opening-saved-world");
  const gateway = await startLocalGateway({ executable: gatewayExecutable, prefix: "local-playtest",
    reuse: { output: state.bootstrapOutput, realmId: state.profile.realmId }, companionPolicy: state.companionPolicy,
    outputRoot: dataRoot, supplyConfigDirectory: dirname(stateFile) });
  let activeActor;
  const profile = structuredClone(state.profile);
  profile.url = gateway.url;
  profile.guardPort = await freePort();
  try {
    return { gateway, output: gateway.output, owner: state.owner, actor: state.actor, profile,
      archiveKey: state.archiveKey, firstRun: false,
      async startActor(activeProfile = profile, { run = true, modelEnvironment = {} } = {}) {
        activeActor = launchTrialActor({ profile: activeProfile, identity: state.actor, archiveKey: state.archiveKey, run, modelEnvironment });
        await activeActor.started;
        return activeActor;
      },
      async close() { try { await activeActor?.close(); } finally { await gateway.close(); } },
    };
  } catch (error) { await gateway.close(); throw error; }
}

export async function runLocalPlaytest({ gatewayExecutable = resolve(repoRoot, "gateway/mir2-gateway.exe"), nativeExecutable = resolve(repoRoot, "game/mir2-platform-windows.exe") } = {}) {
  recordStartupStage("starting");
  await Promise.all([access(gatewayExecutable), access(nativeExecutable)]);
  // The launcher lock is distinct from the resident archive lock. A second
  // double-click cannot start a competing account-store writer, even pre-login.
  const release = await acquireArchiveGuard("alo-r16-local-playtest-launcher");
  let trial, native;
  try {
    const localModel = await loadLocalModelSettings({ bundleDir: repoRoot });
    trial = await prepareLocalPlaytest({ gatewayExecutable, onStage: recordStartupStage });
    recordStartupStage("connecting-companion");
    const activeProfile = structuredClone(trial.profile);
    if (localModel.provider === "openai") Object.assign(activeProfile.runtimeService, {
      provider: "openai", model: localModel.model, credentialRef: "OPENAI_API_KEY", promptVersion: "alo-objective-v2", maxCallsPerHour: localModel.maxCallsPerHour,
    });
    const modelEnvironment = localModel.provider === "openai" ? modelProcessEnvironment(localModel.apiKey, await loadLocalModelNetwork({ bundleDir: repoRoot })) : {};
    await trial.startActor(activeProfile, { run: true, modelEnvironment });
    localModel.apiKey = null;
    delete modelEnvironment.OPENAI_API_KEY;
    const nativeProfile = resolve(appRoot, ".data/local-playtest/native-profile");
    await mkdir(nativeProfile, { recursive: true });
    let diagnosticEnvironment = {};
    try {
      await access(resolve(nativeProfile, "enable-companion-diagnostics"));
      diagnosticEnvironment = { MIR2_COMPANION_DIAGNOSTICS_PATH: resolve(nativeProfile, "companion-diagnostics.jsonl") };
    } catch (error) { if (error.code !== "ENOENT") throw error; }
    await writeFile(resolve(dirname(nativeExecutable), "mir2-client.toml"), `[server]\ngateway_ws_url = "${trial.gateway.url}"\nallowed_classes = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]\n[display]\nwidth = 1024\nheight = 768\nforce_daylight = true\n[gameplay]\nquest_guidance = "${R16_GAMEPLAY_BASELINE.questGuidance}"\n`);
    const diagnosticsEnabled = Object.keys(diagnosticEnvironment).length !== 0;
    recordStartupStage("opening-game");
    native = spawn(nativeExecutable, [], { cwd: dirname(nativeExecutable), windowsHide: false, stdio: diagnosticsEnabled ? ["ignore", "pipe", "pipe"] : "ignore",
      env: isolatedEnvironment({ APPDATA: nativeProfile, LOCALAPPDATA: nativeProfile,
        MIR2_GATEWAY_WS_URL: trial.gateway.url, MIR2_NATIVE_ACCOUNT: trial.owner.accountId,
        MIR2_NATIVE_PASSWORD: trial.owner.password, MIR2_NATIVE_CHARACTER_INDEX: String(trial.owner.characterIndex),
        MIR2_COMPANION_EXPORT_DIR: resolve(nativeProfile, "exports"), ...diagnosticEnvironment }) });
    if (diagnosticsEnabled) {
      const output = createWriteStream(resolve(nativeProfile, "native-diagnostics.log"), { flags: "a", mode: 0o600 });
      let captured = 0;
      output.on("error", () => {});
      const capture = chunk => {
        const remaining = 1024 * 1024 - captured;
        if (remaining > 0) { const part = chunk.subarray(0, remaining); captured += part.length; output.write(part); }
      };
      native.stdout.on("data", capture); native.stderr.on("data", capture);
      native.once("exit", () => output.end());
    }
    await once(native, "spawn");
    recordStartupStage("running");
    console.log(JSON.stringify({ type: "localPlaytestReady", environment: "test", firstRun: trial.firstRun,
      archiveId: trial.profile.companion.archiveId, worldEpoch: trial.profile.companion.worldEpoch, modelEnabled: activeProfile.runtimeService.provider === "openai", paymentEnabled: false }));
    const [exitCode] = await once(native, "exit");
    if (exitCode !== 0) throw new Error("The local game client exited unsuccessfully.");
    recordStartupStage("stopped");
  } finally {
    try { await trial?.close(); } finally { await release(); }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runLocalPlaytest().catch(error => {
    if (error?.code === "ARCHIVE_ALREADY_RUNNING") {
      console.log(JSON.stringify({ type: "localPlaytestAlreadyRunning", at: new Date().toISOString() }));
      process.exitCode = 20;
      return;
    }
    console.error(JSON.stringify({ type: "localPlaytestFailed", stage: startupStage, code: error?.code === "ARCHIVE_LOCK_UNAVAILABLE" ? "archive-lock-unavailable" : error?.code === "LOCAL_MODEL_SETTINGS_UNAVAILABLE" ? "local-model-settings-unavailable" : error?.code === "LOCAL_MODEL_NETWORK_UNAVAILABLE" ? "local-model-network-unavailable" : "startup-failed", at: new Date().toISOString() }));
    if (error?.code === "LOCAL_MODEL_SETTINGS_UNAVAILABLE") console.error("Open Numeron-Alo-Settings.exe in this trial directory and save the local model settings again; existing characters and memories were not reset.");
    console.error("Local Alo trial stopped. Check the private local configuration and companion log; existing characters and memories were not reset.");
    process.exitCode = 1;
  });
}
