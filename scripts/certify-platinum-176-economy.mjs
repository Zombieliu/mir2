#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const naturalPath = path.resolve(process.argv[2] ?? path.join(repoRoot, "docs/generated/player-qa/platinum-176/latest-natural-1-to-50.json"));
const releasePath = path.resolve(process.argv[3] ?? path.join(repoRoot, "docs/generated/player-qa/platinum-176/latest-release-evidence.json"));
const outputPath = path.resolve(process.argv[4] ?? path.join(repoRoot, "docs/generated/player-qa/platinum-176/latest-economy.json"));

const antiArbitrageCase = "runtime::session::tests::platinum_176_vendor_round_trips_cannot_create_gold_over_72_hours";
const testOutput = execFileSync(
  "cargo",
  ["+1.89.0", "test", "-p", "mir2-simulation", "--lib", antiArbitrageCase, "--", "--exact", "--nocapture"],
  { cwd: repoRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
);
if (!testOutput.includes(`test ${antiArbitrageCase} ... ok`)) {
  throw new Error(`exact anti-arbitrage case did not run: ${antiArbitrageCase}`);
}

const natural = readJson(naturalPath);
const release = readJson(releasePath);
const first = natural.segments?.[0]?.first;
const last = natural.segments?.at(-1)?.last;
const allInventory = (natural.segments ?? []).flatMap((segment) => [segment.first, segment.last])
  .flatMap((snapshot) => snapshot?.inventory ?? []);
const allEquipment = (natural.segments ?? []).flatMap((segment) => [segment.first, segment.last])
  .flatMap((snapshot) => snapshot?.equipment ?? []);
const potionNames = allInventory.filter((name) => /drug|potion/i.test(name));
const durabilitySamples = allEquipment.filter((item) => Number.isFinite(item.durabilityCurrent));
const assertions = {
  naturalCertificatePassed: natural.passed === true,
  releaseEvidencePassed: release.passed === true,
  transactionAndMailPassed: release.categories?.some((entry) => entry.id === "economy.transaction-and-mail" && entry.passed),
  consumableAndRepairPassed: release.categories?.some((entry) => entry.id === "economy.consumables-and-repair" && entry.passed),
  naturalGoldObserved: (last?.gold ?? 0) > (first?.gold ?? 0),
  naturalPotionObserved: potionNames.length > 0,
  naturalDurabilityObserved: durabilitySamples.length > 0,
  naturalInventoryGrowthObserved: (last?.inventory?.length ?? 0) > (first?.inventory?.length ?? 0),
  seventyTwoHourVendorAntiArbitragePassed: true,
};
const report = {
  schema: "mir2-platinum-176-economy/1",
  generatedAt: new Date().toISOString(),
  profileId: "platinum_176",
  passed: Object.values(assertions).every(Boolean),
  assertions,
  telemetry: {
    gold: { first: first?.gold ?? null, last: last?.gold ?? null },
    inventoryCount: { first: first?.inventory?.length ?? null, last: last?.inventory?.length ?? null },
    potionNames: [...new Set(potionNames)].sort(),
    durabilitySamples,
  },
  deterministicEvidence: {
    antiArbitrageCase,
    simulatedHours: 72,
    releaseEvidence: path.relative(repoRoot, releasePath),
  },
  naturalEvidence: path.relative(repoRoot, naturalPath),
  caveats: [
    "The 72-hour gate is a deterministic simulated vendor round-trip test, not a 72-hour wall-clock soak.",
    "The two-hour 50-client PostgreSQL and Redis soak is certified separately.",
    "Launch telemetry should still watch player-to-player price formation; this certificate closes code correctness and observed natural resource flow.",
  ],
};

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ ok: report.passed, outputPath, assertions }, null, 2));
if (!report.passed) process.exitCode = 1;

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}
