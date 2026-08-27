#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ledgerPath = resolve(
  repoRoot,
  "docs/generated/player-qa/windows-visual-parity/phase-a-denominator.json",
);
const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
const contract = readFileSync(
  resolve(repoRoot, "docs/parity/CRYSTAL-WINDOWS-VISUAL-PARITY-CONTRACT.md"),
  "utf8",
);

assert.equal(ledger.schemaVersion, 1);
assert.equal(ledger.sourceRevision, "484983404e3d6afa584e93801f8006ae3429bea9");
assert.equal(ledger.sourceRootClean, false);
assert.equal(
  ledger.implementationBaseRevision,
  "67a55b37900ced07d66bd788cbe06ef429ede8aa",
);
assert.equal(ledger.branch, "codex/windows-visual-parity");
assert.equal(ledger.claims.semanticLeafInventoryComplete, false);
assert.equal(ledger.claims.inventoryComplete, false);
assert.equal(ledger.claims.globalParityPercent, null);
assert.equal(ledger.claims.accepted, false);
assert.equal(ledger.claims.visualAccepted, false);
assert.equal(ledger.countingRule.buttonStatesAreRequiredGatesNotLeaves, true);
assert.deepEqual(ledger.countingRule.requiredButtonStates, [
  "normal",
  "hover",
  "pressed",
  "disabled",
]);
assert.equal(ledger.countingRule.unknownBlockedFailCountAsPass, false);
assert.equal(ledger.countingRule.requiredGateMayBeNotApplicable, false);

const uiFamilies = ledger.uiPhaseA.families;
assert.ok(Array.isArray(uiFamilies) && uiFamilies.length > 0);
assert.equal(new Set(uiFamilies.map(({ id }) => id)).size, uiFamilies.length);
assert.equal(
  uiFamilies.reduce((sum, { leaves }) => sum + leaves, 0),
  ledger.uiPhaseA.knownFixedTemplateLeaves,
);
assert.equal(ledger.uiPhaseA.knownFixedTemplateLeaves, 410);
assert.equal(
  uiFamilies.find(({ id }) => id === "character")?.equipmentCells,
  14,
);

assert.deepEqual(ledger.actors, {
  inventoryComplete: false,
  playerPixelLibraries: 477,
  playerPixelFrames: 541010,
  currentPlayerAtlasRoots: 7,
  currentPlayerAtlasFrames: 7360,
  monsterPixelLibraries: 546,
  monsterPixelFrames: 219607,
  currentMonsterAtlasLibraries: 8,
  currentMonsterAtlasFrames: 1742,
  playerActionRecords: 33,
  playerBodyDirectionPhaseLeaves: 1384,
  playerEffectWingDirectionPhaseLeaves: 1240,
  monsterExplicitLibraryContracts: 455,
  monsterExplicitActionRecords: 3332,
  monsterExplicitDirectionPhaseLeaves: 153416,
  monsterLibrariesWithoutExplicitContract: 91,
  visualRuleLeaves: 32,
});
assert.deepEqual(ledger.effects, {
  inventoryComplete: false,
  nonNoneSpells: 129,
  nonNoneSpellEffects: 34,
  currentObjectEffectManifestEntries: 11,
  uniqueSpellObjectBranches: 29,
  groundEffectManifestEntries: 13,
  spellObjectBackedGroundEffectEntries: 7,
  mapEventSpells: 19,
  mapEventVisualEntries: 0,
  spellEffectMapManifestEntries: 2,
  nonNonePoisonTypes: 11,
  buffTypes: 59,
  worldObservableBuffBranches: 17,
  mirActions: 45,
  weatherFlags: 10,
  lightSettings: 5,
});

assert.deepEqual(ledger.firstSlices.combatEffects, [
  "FlamingSword",
  "FireBall",
  "Lightning",
  "SoulFireBall",
  "FireWall",
]);
assert.deepEqual(ledger.firstSlices.uiStates, [
  "hud-normal",
  "inventory-hover",
  "inventory-pressed",
  "bigmap-teleport-disabled",
]);
assert.deepEqual(ledger.externalGates, [
  "clean-crystal-source-binding",
  "same-exe-ui-live-wss",
  "real-dpi-100-125-150",
  "native-30-minute-soak",
  "human-visual-animation-audio-feel",
  "complete-legal-asset-pack",
  "formal-publisher-signing",
]);
assert.match(contract, /current fixed\/template UI scope contains 410 leaves/);
assert.match(contract, /7 corresponding branches among 13 ground-manifest entries/);
assert.match(contract, /Map event spells \| 19 \| 0;/);
assert.match(contract, /globalParityPercent: null/);
assert.match(contract, /visualAccepted: false/);

console.log(
  JSON.stringify({
    ok: true,
    ledgerId: ledger.ledgerId,
    uiLeaves: ledger.uiPhaseA.knownFixedTemplateLeaves,
    globalParityPercent: ledger.claims.globalParityPercent,
    inventoryComplete: ledger.claims.inventoryComplete,
    note: "ledger integrity only; complete source extraction remains open",
  }),
);
