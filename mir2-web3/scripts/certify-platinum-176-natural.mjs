#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPaths = process.argv.slice(2, 6).length === 4
  ? process.argv.slice(2, 6).map((entry) => path.resolve(entry))
  : [
      "docs/stage5-screenshots/platinum-176-natural-1-to-7-manifest.json",
      "docs/stage5-screenshots/platinum-176-natural-8-to-21-manifest.json",
      "docs/stage5-screenshots/platinum-176-natural-22-to-35-manifest.json",
      "docs/stage5-screenshots/platinum-176-natural-36-to-50-manifest.json",
    ].map((entry) => path.join(repoRoot, entry));
const outputPath = path.resolve(
  process.argv[6] ?? path.join(repoRoot, "docs/generated/player-qa/platinum-176/latest-natural-1-to-50.json"),
);

const manifests = manifestPaths.map(readJson);
const expectedAccountId = process.env.MIR2_NATURAL_ACCOUNT_ID ??
  manifests.flatMap((manifest) => manifest.selectFlow ?? [])
    .map((entry) => entry.accountId)
    .find(Boolean) ??
  "NatFinalA21";
const expectedCharacter = manifests[0].selectFlow
  ?.flatMap((entry) => entry.characters ?? [])
  .find((entry) => Number(entry.level) === 1 && entry.classKey === "warrior") ??
  manifests[0].selectFlow?.flatMap((entry) => entry.characters ?? [])[0] ??
  null;
if (!expectedCharacter) {
  throw new Error("Could not derive the persistent natural-progression character from segment 1.");
}
const forbiddenCommandTypes = new Set([
  "moveTo",
  "transferMap",
  "stage5Command",
  "qa",
  "grantExperience",
  "grantGold",
  "grantItem",
]);
const expectedEndpoints = [[1, 7], [7, 21], [21, 35], [35, 50]];

const segments = manifests.map((manifest, index) => {
  const snapshots = manifest.naturalProgressionFlow ?? [];
  const first = snapshots[0];
  const last = snapshots.at(-1);
  const characters = (manifest.selectFlow ?? []).flatMap((entry) => entry.characters ?? []);
  const identity = characters.find((entry) => entry.name === expectedCharacter.name) ?? null;
  const commands = manifest.commandAudit ?? [];
  const forbidden = commands.filter((entry) => forbiddenCommandTypes.has(entry?.type));
  const endpoint = [first?.self?.level ?? null, last?.self?.level ?? null];
  const expected = expectedEndpoints[index];
  const assertions = {
    expectedEndpoint: endpoint[0] === expected[0] && endpoint[1] === expected[1],
    sameAccount: (manifest.selectFlow ?? []).every((entry) => !entry.accountId || entry.accountId === expectedAccountId),
    sameCharacter:
      identity?.index === expectedCharacter.index &&
      identity?.name === expectedCharacter.name &&
      identity?.classKey === expectedCharacter.classKey,
    naturalCommandsOnly: forbidden.length === 0,
    noCriticalConsoleErrors: (manifest.criticalConsoleErrors ?? []).length === 0,
    gatewayStayedOpen: snapshots.every((entry) => entry.wsState === "open"),
  };
  return {
    index: index + 1,
    source: path.relative(repoRoot, manifestPaths[index]),
    sha256: sha256(manifestPaths[index]),
    endpoint,
    generatedAt: manifest.generatedAt,
    commandCount: commands.length,
    commandTypes: [...new Set(commands.map((entry) => entry?.type).filter(Boolean))].sort(),
    forbiddenCommands: forbidden,
    criticalConsoleErrorCount: (manifest.criticalConsoleErrors ?? []).length,
    multipliers: {
      killExperience: manifest.killExperienceMultiplier ?? 1,
      killDamage: manifest.killDamageMultiplier ?? 1,
      killDropSample: manifest.killDropSampleMultiplier ?? 1,
    },
    first: compactSnapshot(first),
    last: compactSnapshot(last),
    assertions,
    passed: Object.values(assertions).every(Boolean),
  };
});

const assertions = {
  fourContinuousSegments: segments.length === 4 && segments.every((entry) => entry.passed),
  continuousLevelEndpoints: segments.every((entry, index) => index === 0 || segments[index - 1].endpoint[1] === entry.endpoint[0]),
  reachedLevel50: segments.at(-1)?.last?.level === 50,
  onePersistentIdentity: segments.every((entry) => entry.assertions.sameAccount && entry.assertions.sameCharacter),
  noForbiddenMutationCommands: segments.every((entry) => entry.assertions.naturalCommandsOnly),
  noCriticalConsoleErrors: segments.every((entry) => entry.assertions.noCriticalConsoleErrors),
};
const report = {
  schema: "mir2-platinum-176-natural-1-to-50/1",
  generatedAt: new Date().toISOString(),
  profileId: "platinum_176",
  accountId: expectedAccountId,
  character: {
    index: expectedCharacter.index,
    name: expectedCharacter.name,
    classKey: expectedCharacter.classKey,
  },
  passed: Object.values(assertions).every(Boolean),
  assertions,
  segments,
  disclosure: [
    "All movement, attacks, pickup, item use and ticks traveled through the browser Gateway command surface.",
    "The listed QA multipliers accelerate wall-clock acquisition but do not grant levels, experience, gold, equipment or map position directly.",
    "Each source manifest is content-addressed so acquisition evidence cannot be silently replaced by a later reconnect-only manifest.",
  ],
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ ok: report.passed, outputPath, assertions, segments: segments.length }, null, 2));
if (!report.passed) process.exitCode = 1;

function compactSnapshot(snapshot) {
  if (!snapshot) return null;
  return {
    label: snapshot.label,
    level: snapshot.self?.level ?? null,
    experience: snapshot.self?.experience ?? null,
    maxExperience: snapshot.self?.maxExperience ?? null,
    mapFileName: snapshot.mapFileName ?? null,
    gold: snapshot.gold ?? 0,
    inventory: (snapshot.inventoryItems ?? []).map((item) => item.name),
    equipment: (snapshot.equipmentItems ?? []).map((item) => ({
      name: item.name,
      slot: item.slot,
      durabilityCurrent: item.durabilityCurrent,
      durabilityMax: item.durabilityMax,
    })),
  };
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function sha256(filePath) {
  return crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}
