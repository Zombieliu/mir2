import { readFile, writeFile, open } from "node:fs/promises";
import { DatabaseSync } from "node:sqlite";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CompanionStore } from "./memory/companion-store.mjs";
import { acquireArchiveGuard } from "./archive-guard.mjs";
import { archiveKeyFromEnvironment, validateRuntimeService } from "./companion-runtime-service.mjs";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url));
function argsOf(args) {
  const command = args.shift(), result = { command };
  if (!["inspect", "export", "import", "adopt-legacy"].includes(command)) throw new Error("Use inspect, export, import or adopt-legacy.");
  while (args.length) {
    const key = args.shift();
    if (!["--profile", "--input", "--output"].includes(key) || !args.length || Object.hasOwn(result, key.slice(2))) throw new Error("Invalid memory command arguments.");
    result[key.slice(2)] = args.shift();
  }
  if (!result.profile || command === "export" && !result.output || command === "import" && !result.input) throw new Error("Provide the required profile/input/output files.");
  return result;
}
async function boundedArchive(path) {
  const handle = await open(resolve(path), "r");
  try {
    const buffer = Buffer.alloc(2 * 1024 * 1024 + 1);
    let used = 0;
    while (used < buffer.length) { const { bytesRead } = await handle.read(buffer, used, buffer.length - used, null); if (!bytesRead) break; used += bytesRead; }
    if (used === buffer.length) throw new Error("Archive exceeds the portable size limit.");
    return buffer.subarray(0, used).toString("utf8");
  } finally { await handle.close(); }
}
export async function runMemoryCommand(args, { environment = process.env } = {}) {
  const options = argsOf([...args]), profile = JSON.parse(await readFile(resolve(options.profile), "utf8"));
  if (!profile.companion?.memoryPath) throw new Error("Configure a companion memory path.");
  const companion = profile.companion, path = resolve(APP_ROOT, companion.memoryPath);
  if (options.command === "inspect") {
    const db = new DatabaseSync(path, { readOnly: true });
    try {
      const row = db.prepare("SELECT binding_json FROM companion_state WHERE singleton=1").get();
      const version = db.prepare("PRAGMA user_version").get().user_version;
      const archive = [2, 3].includes(version) ? JSON.parse(db.prepare("SELECT archive_json FROM character_archive WHERE singleton=1").get().archive_json) : null;
      return { type: "companionArchiveInspection", schemaVersion: version, binding: JSON.parse(row.binding_json), archive };
    } finally { db.close(); }
  }
  const service = validateRuntimeService(profile);
  if (!service) throw new Error("Configure trusted P0 archive/owner/world identity before portability operations.");
  const release = await acquireArchiveGuard(companion.archiveId);
  let store, key;
  try {
    const binding = { realmId: profile.realmId, residentId: companion.residentId, characterName: profile.characterName, characterIndex: companion.characterIndex,
      ...(companion.operatorRef ? { operatorRef: companion.operatorRef } : {}) };
    if (options.command === "import") {
      key = archiveKeyFromEnvironment(service, environment);
      store = CompanionStore.importArchive({ path, binding, archiveId: companion.archiveId, ownerRef: companion.ownerRef,
        worldEpoch: companion.worldEpoch, key, envelope: await boundedArchive(options.input) });
    } else {
      store = new CompanionStore({ path, binding, archiveId: companion.archiveId, ownerRef: companion.ownerRef, worldEpoch: companion.worldEpoch,
        ...(options.command === "adopt-legacy" ? { adoptLegacy: true } : {}) });
      if (options.command === "export") {
        key = archiveKeyFromEnvironment(service, environment);
        await writeFile(resolve(options.output), store.exportArchive({ key }), { flag: "wx", mode: 0o600 });
      }
    }
    return { type: options.command === "import" ? "companionArchiveImported" : options.command === "export" ? "companionArchiveExported" : "companionLegacyAdopted",
      archiveId: store.archive().archiveId, policyVersion: store.profile().policyVersion, growthRevision: store.growth().growthRevision };
  } finally { store?.close(); key?.fill(0); await release(); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  runMemoryCommand(process.argv.slice(2)).then(value => console.log(JSON.stringify(value))).catch(() => {
    console.error("Memory operation stopped: check trusted profile/binding, signing key, fresh import destination, input and archive lock. Existing files were not overwritten.");
    process.exitCode = 1;
  });
}
