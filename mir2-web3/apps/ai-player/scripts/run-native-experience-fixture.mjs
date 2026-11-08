// Private manual/Computer-Use fixture. It does not claim a human trial.
import { createInterface } from "node:readline";
import { copyFile, mkdir, readFile, readdir, stat, symlink, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { appRoot, repoRoot, sha256 } from "./local-gateway.mjs";
import { verifyArchive } from "../src/memory/archive-codec.mjs";

let fixture, actor, phase = "prepare", activeProfile;
const startedAt = Date.now();
const initialExports = new Set();
const exportRoot = resolve(process.env.APPDATA ?? process.env.LOCALAPPDATA, "mir2-web3", "companion-exports");
const report = value => console.log(JSON.stringify(value));
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
    if (initialExports.has(name) || !name.endsWith(".json")) continue;
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
    case "status": report({ type: "nativeFixtureStatus", phase, runtimeId: activeProfile.runtimeService.runtimeId, ...readMeta(activeProfile.companion.memoryPath) }); break;
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
      actor = await fixture.startActor(fixture.profileB, { run: true });
      activeProfile = fixture.profileB; phase = "runner-B";
      report({ type: "nativeFixtureMigrationPassed", oldPid, newPid: actor.pid, differentProcess: oldPid !== actor.pid, ...readMeta(activeProfile.companion.memoryPath) }); break;
    }
    case "stop": await fixture.close(); fixture = null; process.exitCode = 0; process.exit(0); break;
    default: report({ type: "nativeFixtureCommands", commands: ["status", "exports", "migrate", "stop"] });
  }
}

try {
  try { for (const name of await readdir(exportRoot)) initialExports.add(name); } catch { /* no previous exports */ }
  fixture = await createTrialFixture();
  actor = await fixture.startActor(fixture.profileA, { run: true }); activeProfile = fixture.profileA; phase = "runner-A";
  const nativeDirectory = resolve(fixture.output, "native-client"); await mkdir(nativeDirectory);
  const sourceExe = resolve(repoRoot, "apps/game-client/platform-windows/target/release/mir2-platform-windows.exe"), exe = resolve(nativeDirectory, "mir2-platform-windows.exe");
  await copyFile(sourceExe, exe);
  // Reuse current art plus the existing complete generated map pack. All links
  // and copies are new files in this fixture; source assets are never edited.
  const currentAssets = resolve(repoRoot, "apps/web/public"), candidateAssets = resolve(repoRoot, "dist/mir2-windows-candidate/WN-CANDIDATE-03-20260826/mir2-assets");
  const assetRoot = resolve(nativeDirectory, "mir2-assets"); await mkdir(assetRoot);
  const currentEntries = await readdir(currentAssets, { withFileTypes: true });
  for (const item of currentEntries) {
    if (item.name === "generated") continue;
    if (item.isDirectory()) await symlink(resolve(currentAssets, item.name), resolve(assetRoot, item.name), "junction");
    else if (item.isFile()) await copyFile(resolve(currentAssets, item.name), resolve(assetRoot, item.name));
  }
  // The repository serves map bytes through its web API; the installed native
  // bundle also needs the actual crystal-map-pack files beside its manifests.
  const currentNames = new Set(currentEntries.map(item => item.name));
  for (const item of await readdir(candidateAssets, { withFileTypes: true })) {
    if (item.name === "generated" || currentNames.has(item.name)) continue;
    if (item.isDirectory()) await symlink(resolve(candidateAssets, item.name), resolve(assetRoot, item.name), "junction");
    else if (item.isFile()) await copyFile(resolve(candidateAssets, item.name), resolve(assetRoot, item.name));
  }
  await mkdir(resolve(assetRoot, "generated"));
  for (const item of await readdir(resolve(currentAssets, "generated"), { withFileTypes: true })) {
    if (["map-atlas", "native-map-keyed"].includes(item.name)) continue;
    if (item.isDirectory()) await symlink(resolve(currentAssets, "generated", item.name), resolve(assetRoot, "generated", item.name), "junction");
    else if (item.isFile()) await copyFile(resolve(currentAssets, "generated", item.name), resolve(assetRoot, "generated", item.name));
  }
  for (const name of ["map-atlas", "native-map-keyed"]) await symlink(resolve(candidateAssets, "generated", name), resolve(assetRoot, "generated", name), "junction");
  for (const name of ["bevy-entity-atlases/manifest.json", "generated/map-atlas/manifest.json", "generated/native-map-keyed/manifest.json", "crystal-map-pack/0.map.gz", "original-effects/effects.generated.json"]) await stat(resolve(assetRoot, name));
  await writeFile(resolve(nativeDirectory, "mir2-client.toml"), `[server]\ngateway_ws_url = "${fixture.gateway.url}"\n[display]\nwidth = 1024\nheight = 768\n`, { flag: "wx" });
  const credentialsFile = resolve(fixture.output, "native-credentials.local.json");
  await writeFile(credentialsFile, JSON.stringify({ accountId: fixture.owner.accountId, password: fixture.owner.password, characterName: fixture.owner.characterName, characterIndex: fixture.owner.characterIndex }), { flag: "wx", mode: 0o600 });
  report({ type: "nativeFixtureReady", output: fixture.output, exe, credentialsFile, assetRoot, nativeSha256: await sha256(exe), gatewaySha256: await sha256(fixture.gateway.executable),
    archiveId: fixture.profileA.companion.archiveId, realmId: fixture.gateway.realmId, environment: "test", modelEnabled: false });
  const lines = createInterface({ input: process.stdin, terminal: false });
  let queue = Promise.resolve();
  lines.on("line", line => { queue = queue.then(() => command(line)).catch(() => report({ type: "nativeFixtureOperationUnavailable", phase })); });
  lines.on("close", () => { queue.then(() => fixture?.close()).finally(() => { process.exitCode = 0; }); });
  process.once("SIGTERM", () => fixture?.close().finally(() => process.exit(0)));
  process.once("SIGINT", () => fixture?.close().finally(() => process.exit(0)));
} catch {
  await fixture?.close().catch(() => {});
  report({ type: "nativeFixtureFailed", phase }); process.exitCode = 1;
}
