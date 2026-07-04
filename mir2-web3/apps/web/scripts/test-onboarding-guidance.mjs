// Behavioral tests for lib/onboarding-guidance.ts — the pure helpers behind the
// net-new onboarding clarity fixes (class-aware objective copy + guide-quest
// coordination). Pure logic only: no DOM, no React. The .ts source is transpiled
// in-memory via the `typescript` devDependency (same harness as the other
// test-*.mjs scripts).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

const mod = loadTypeScriptModule(new URL("../lib/onboarding-guidance.ts", import.meta.url));

const {
  GUIDE_QUEST_ID,
  GUIDE_QUEST_TARGET_NAME,
  isMeleeClass,
  rangeHintForClass,
  classAwareObjectiveLine,
  classAwareObjectiveLines,
  activeGuideQuest,
  guideQuestAttackHint,
} = mod;

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
}

// The sim copy that motivated gap C (class-blind melee instruction).
const MELEE_LINE =
  "The wasp is still alive. Stay in melee range before attacking. Return after you obtain the Wasp Stinger.";

check("guide quest id matches the sim contract (1001)", () => {
  assert.equal(GUIDE_QUEST_ID, 1001);
});

check("melee classification: warrior/assassin are melee; others are not", () => {
  assert.equal(isMeleeClass("warrior"), true);
  assert.equal(isMeleeClass("assassin"), true);
  assert.equal(isMeleeClass("Warrior"), true, "case-insensitive");
  assert.equal(isMeleeClass("archer"), false);
  assert.equal(isMeleeClass("wizard"), false);
  assert.equal(isMeleeClass("taoist"), false);
  assert.equal(isMeleeClass(null), false);
  assert.equal(isMeleeClass(undefined), false);
});

check("range hint is class-appropriate", () => {
  assert.match(rangeHintForClass("warrior"), /melee/i);
  assert.match(rangeHintForClass("archer"), /distance|range/i);
  assert.match(rangeHintForClass("wizard"), /range/i);
  assert.doesNotMatch(rangeHintForClass("archer"), /melee/i);
});

check("melee class keeps the original melee instruction verbatim", () => {
  assert.equal(classAwareObjectiveLine(MELEE_LINE, "warrior"), MELEE_LINE);
  assert.equal(classAwareObjectiveLine(MELEE_LINE, "assassin"), MELEE_LINE);
});

check("archer no longer told to stay in melee range", () => {
  const out = classAwareObjectiveLine(MELEE_LINE, "archer");
  assert.doesNotMatch(out, /melee/i, "the word 'melee' must be gone for an archer");
  assert.match(out, /distance|range/i, "a ranged instruction replaces it");
  // The rest of the sentence (target + return hint) survives.
  assert.match(out, /wasp/i);
  assert.match(out, /Wasp Stinger/);
});

check("caster classes also lose the melee instruction", () => {
  for (const cls of ["wizard", "taoist"]) {
    const out = classAwareObjectiveLine(MELEE_LINE, cls);
    assert.doesNotMatch(out, /melee/i, `${cls} should not be told to melee`);
  }
});

check("lines without a melee instruction are returned unchanged", () => {
  const line = "Defeat the Field Wasp east of the road and secure a Wasp Stinger.";
  assert.equal(classAwareObjectiveLine(line, "archer"), line);
  assert.equal(classAwareObjectiveLine(line, "warrior"), line);
});

check("empty / non-string input is safe", () => {
  assert.equal(classAwareObjectiveLine("", "archer"), "");
  assert.equal(classAwareObjectiveLine(null, "archer"), null);
  assert.equal(classAwareObjectiveLine(undefined, "archer"), undefined);
});

check("idempotent: rewriting an already-rewritten archer line is stable", () => {
  const once = classAwareObjectiveLine(MELEE_LINE, "archer");
  const twice = classAwareObjectiveLine(once, "archer");
  assert.equal(twice, once);
});

check("classAwareObjectiveLines maps across a list", () => {
  const out = classAwareObjectiveLines([MELEE_LINE, "Plain line."], "archer");
  assert.equal(out.length, 2);
  assert.doesNotMatch(out[0], /melee/i);
  assert.equal(out[1], "Plain line.");
});

// ---- guide quest coordination -------------------------------------------------

const inProgressLog = [
  { questId: 1001, stage: "inProgress", tracker: "Field Wasp 0/1 defeated." },
];
const availableLog = [{ questId: 1001, stage: "available", tracker: "Guide waiting." }];
const readyLog = [{ questId: 1001, stage: "readyToTurnIn", tracker: "Return to Guide." }];
const otherLog = [{ questId: 42, stage: "inProgress", tracker: "Something else." }];

check("activeGuideQuest finds the in-progress guide quest", () => {
  assert.ok(activeGuideQuest(inProgressLog));
  assert.equal(activeGuideQuest(inProgressLog).questId, 1001);
});

check("activeGuideQuest ignores completed / non-guide / empty logs", () => {
  assert.equal(activeGuideQuest([{ questId: 1001, stage: "completed" }]), null);
  assert.equal(activeGuideQuest(otherLog), null);
  assert.equal(activeGuideQuest([]), null);
  assert.equal(activeGuideQuest(null), null);
  assert.equal(activeGuideQuest(undefined), null);
});

check("guideQuestAttackHint names the wasp for an in-progress archer, never melee", () => {
  const hint = guideQuestAttackHint(inProgressLog, "archer");
  assert.ok(hint);
  assert.match(hint, new RegExp(GUIDE_QUEST_TARGET_NAME));
  assert.doesNotMatch(hint, /melee/i);
  assert.match(hint, /range/i);
});

check("guideQuestAttackHint tells a warrior to close to melee", () => {
  const hint = guideQuestAttackHint(inProgressLog, "warrior");
  assert.match(hint, /melee/i);
});

check("guideQuestAttackHint points an 'available' player at the Village Guide", () => {
  const hint = guideQuestAttackHint(availableLog, "archer");
  assert.match(hint, /Village Guide/i);
});

check("guideQuestAttackHint points a 'readyToTurnIn' player back to the guide", () => {
  const hint = guideQuestAttackHint(readyLog, "archer");
  assert.match(hint, /Village Guide/i);
  assert.match(hint, /Wasp Stinger/i);
});

check("guideQuestAttackHint is null when not on the guide quest", () => {
  assert.equal(guideQuestAttackHint(otherLog, "archer"), null);
  assert.equal(guideQuestAttackHint(null, "archer"), null);
});

console.log(`onboarding-guidance: ${passed} checks passed`);
